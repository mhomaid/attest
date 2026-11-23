//! Service-to-service authentication.
//!
//! HTTP APIs require a short-lived HS256 JWT whose `aud` is the service name.
//! Collector ingest instead requires an HMAC over the raw body and a timestamp.
//! `/healthz` is the only route that stays open. The secret is never written to logs.

use axum::body::Body;
use axum::extract::Request;
use axum::http::{HeaderMap, StatusCode};
use axum::middleware::Next;
use axum::response::{IntoResponse, Response};
use axum::Router;
use hmac::{Hmac, Mac};
use http_body_util::BodyExt;
use jsonwebtoken::{decode, encode, Algorithm, DecodingKey, EncodingKey, Header, Validation};
use serde::{Deserialize, Serialize};
use sha2::Sha256;
use std::time::{SystemTime, UNIX_EPOCH};

pub const SERVICE_AUTH_ENV: &str = "ATTEST_SERVICE_AUTH_SECRET";
pub const INGEST_HMAC_ENV: &str = "ATTEST_INGEST_HMAC_SECRET";
/// Tokens last two minutes. Callers mint a fresh one per request.
pub const SERVICE_TOKEN_TTL_SECS: i64 = 120;
/// Ingest signatures older than this are rejected as replays.
pub const INGEST_MAX_SKEW_SECS: i64 = 300;
const MAX_INGEST_BODY: usize = 8 * 1024 * 1024;

pub const AUDIENCE_CONTROL_PLANE: &str = "control-plane";
pub const AUDIENCE_ORCHESTRATOR: &str = "orchestrator";
pub const AUDIENCE_MCP_GATEWAY: &str = "mcp-gateway";
pub const AUDIENCE_WS_GATEWAY: &str = "ws-gateway";

#[derive(Debug)]
pub enum AuthError {
    Rejected,
}

#[derive(Debug, Serialize, Deserialize)]
struct Claims {
    aud: String,
    iat: i64,
    exp: i64,
}

/// Mint an HS256 service token. `ttl_secs` may be negative to build an already-expired token in tests.
pub fn issue_token(secret: &str, audience: &str, ttl_secs: i64) -> String {
    let now = unix_now();
    issue_token_at(secret, audience, now, now.saturating_add(ttl_secs))
}

pub fn issue_token_at(secret: &str, audience: &str, iat: i64, exp: i64) -> String {
    let claims = Claims {
        aud: audience.to_string(),
        iat,
        exp,
    };
    encode(
        &Header::new(Algorithm::HS256),
        &claims,
        &EncodingKey::from_secret(secret.as_bytes()),
    )
    .expect("HS256 encoding of a service token does not fail")
}

pub fn verify_token(secret: &str, audience: &str, token: &str) -> Result<(), AuthError> {
    if secret.is_empty() || token.is_empty() {
        return Err(AuthError::Rejected);
    }
    let mut validation = Validation::new(Algorithm::HS256);
    validation.set_audience(&[audience]);
    validation.validate_exp = true;
    validation.leeway = 0;
    // `aud` is checked via set_audience. Require exp so a token cannot omit it.
    validation.set_required_spec_claims(&["exp"]);
    decode::<Claims>(
        token,
        &DecodingKey::from_secret(secret.as_bytes()),
        &validation,
    )
    .map(|_| ())
    .map_err(|_| AuthError::Rejected)
}

pub fn authorization_header(secret: &str, audience: &str) -> String {
    format!(
        "Bearer {}",
        issue_token(secret, audience, SERVICE_TOKEN_TTL_SECS)
    )
}

/// `Authorization` value for an outgoing call, when a secret is configured.
pub fn authorization_from_env(audience: &str) -> Option<String> {
    let secret = std::env::var(SERVICE_AUTH_ENV).unwrap_or_default();
    if secret.is_empty() {
        None
    } else {
        Some(authorization_header(&secret, audience))
    }
}

/// Wrap a router so every route except `/healthz` requires a service token.
pub fn protect(router: Router, secret: impl Into<String>, audience: impl Into<String>) -> Router {
    let secret = secret.into();
    let audience = audience.into();
    router.layer(axum::middleware::from_fn(
        move |req: Request, next: Next| {
            let secret = secret.clone();
            let audience = audience.clone();
            async move { check_service(secret, audience, req, next).await }
        },
    ))
}

async fn check_service(secret: String, audience: String, req: Request, next: Next) -> Response {
    if req.uri().path() == "/healthz" {
        return next.run(req).await;
    }
    let token = bearer_token(req.headers()).or_else(|| query_token(req.uri().query()));
    let Some(token) = token else {
        tracing::warn!(path = %req.uri().path(), "service auth rejected");
        return unauthorized();
    };
    if verify_token(&secret, &audience, &token).is_err() {
        tracing::warn!(path = %req.uri().path(), "service auth rejected");
        return unauthorized();
    }
    next.run(req).await
}

fn bearer_token(headers: &HeaderMap) -> Option<String> {
    let value = headers
        .get(axum::http::header::AUTHORIZATION)?
        .to_str()
        .ok()?;
    let (scheme, token) = value.split_once(' ')?;
    if !scheme.eq_ignore_ascii_case("bearer") {
        return None;
    }
    let token = token.trim();
    if token.is_empty() {
        None
    } else {
        Some(token.to_string())
    }
}

/// WebSocket clients cannot set headers. They pass the same service token as `access_token`.
fn query_token(query: Option<&str>) -> Option<String> {
    let query = query?;
    for pair in query.split('&') {
        let (key, value) = pair.split_once('=')?;
        if key == "access_token" && !value.is_empty() {
            return Some(value.to_string());
        }
    }
    None
}

fn unauthorized() -> Response {
    (
        StatusCode::UNAUTHORIZED,
        [("content-type", "application/json")],
        r#"{"error":"unauthorized"}"#,
    )
        .into_response()
}

// ── Collector ingest HMAC ────────────────────────────────────────────────────

type HmacSha256 = Hmac<Sha256>;

pub fn sign_ingest(secret: &str, timestamp: i64, body: &[u8]) -> String {
    let mut mac =
        HmacSha256::new_from_slice(secret.as_bytes()).expect("HMAC accepts any key length");
    mac.update(timestamp.to_string().as_bytes());
    mac.update(b".");
    mac.update(body);
    hex::encode(mac.finalize().into_bytes())
}

pub fn verify_ingest(
    secret: &str,
    timestamp: i64,
    body: &[u8],
    signature_hex: &str,
    now: i64,
) -> Result<(), AuthError> {
    if secret.is_empty() {
        return Err(AuthError::Rejected);
    }
    if (now - timestamp).abs() > INGEST_MAX_SKEW_SECS {
        return Err(AuthError::Rejected);
    }
    let Ok(presented) = hex::decode(signature_hex) else {
        return Err(AuthError::Rejected);
    };
    let mut mac =
        HmacSha256::new_from_slice(secret.as_bytes()).expect("HMAC accepts any key length");
    mac.update(timestamp.to_string().as_bytes());
    mac.update(b".");
    mac.update(body);
    mac.verify_slice(&presented)
        .map_err(|_| AuthError::Rejected)
}

/// Require a valid ingest HMAC on `/ingest` and `/ingest/s3`. Other routes pass through.
pub fn protect_ingest(router: Router, secret: impl Into<String>) -> Router {
    let secret = secret.into();
    router.layer(axum::middleware::from_fn(
        move |req: Request, next: Next| {
            let secret = secret.clone();
            async move { check_ingest(secret, req, next).await }
        },
    ))
}

async fn check_ingest(secret: String, req: Request, next: Next) -> Response {
    let path = req.uri().path().to_string();
    if !ingest_path_requires_hmac(&path) {
        return next.run(req).await;
    }
    let (parts, body) = req.into_parts();
    let bytes = match body.collect().await {
        Ok(collected) => collected.to_bytes(),
        Err(_) => return unauthorized(),
    };
    if bytes.len() > MAX_INGEST_BODY {
        return unauthorized();
    }
    let timestamp = parts
        .headers
        .get("x-attest-timestamp")
        .and_then(|v| v.to_str().ok())
        .and_then(|s| s.parse::<i64>().ok());
    let signature = parts
        .headers
        .get("x-attest-signature")
        .and_then(|v| v.to_str().ok())
        .map(str::to_string);
    let (Some(timestamp), Some(signature)) = (timestamp, signature) else {
        tracing::warn!(path = %path, "ingest auth rejected");
        return unauthorized();
    };
    if verify_ingest(&secret, timestamp, &bytes, &signature, unix_now()).is_err() {
        tracing::warn!(path = %path, "ingest auth rejected");
        return unauthorized();
    }
    let req = Request::from_parts(parts, Body::from(bytes));
    next.run(req).await
}

/// `/ingest` and every path under it require the body HMAC. `/healthz` does not.
pub fn ingest_path_requires_hmac(path: &str) -> bool {
    path == "/ingest" || path.starts_with("/ingest/")
}

fn unix_now() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0)
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::routing::{get, post};
    use axum::Router;
    use std::io::Write;
    use std::sync::{Arc, Mutex};
    use tower::ServiceExt;

    const SECRET: &str = "test-service-secret";

    fn app(audience: &str) -> Router {
        protect(
            Router::new()
                .route("/v1/ok", get(|| async { "ok" }))
                .route("/healthz", get(|| async { "ok" })),
            SECRET,
            audience,
        )
    }

    async fn status_of(app: Router, builder: axum::http::Request<Body>) -> StatusCode {
        app.oneshot(builder).await.unwrap().status()
    }

    async fn assert_service(audience: &str) {
        let missing = status_of(
            app(audience),
            axum::http::Request::builder()
                .uri("/v1/ok")
                .body(Body::empty())
                .unwrap(),
        )
        .await;
        assert_eq!(missing, StatusCode::UNAUTHORIZED);

        let wrong = issue_token(SECRET, "other-service", SERVICE_TOKEN_TTL_SECS);
        let wrong_status = status_of(
            app(audience),
            axum::http::Request::builder()
                .uri("/v1/ok")
                .header("authorization", format!("Bearer {wrong}"))
                .body(Body::empty())
                .unwrap(),
        )
        .await;
        assert_eq!(wrong_status, StatusCode::UNAUTHORIZED);

        let ok = issue_token(SECRET, audience, SERVICE_TOKEN_TTL_SECS);
        let ok_status = status_of(
            app(audience),
            axum::http::Request::builder()
                .uri("/v1/ok")
                .header("authorization", format!("Bearer {ok}"))
                .body(Body::empty())
                .unwrap(),
        )
        .await;
        assert_eq!(ok_status, StatusCode::OK);

        let health = status_of(
            app(audience),
            axum::http::Request::builder()
                .uri("/healthz")
                .body(Body::empty())
                .unwrap(),
        )
        .await;
        assert_eq!(health, StatusCode::OK);
    }

    #[tokio::test]
    async fn control_plane_requires_its_own_audience() {
        assert_service(AUDIENCE_CONTROL_PLANE).await;
    }

    #[tokio::test]
    async fn orchestrator_requires_its_own_audience() {
        assert_service(AUDIENCE_ORCHESTRATOR).await;
    }

    #[tokio::test]
    async fn mcp_gateway_requires_its_own_audience() {
        assert_service(AUDIENCE_MCP_GATEWAY).await;
    }

    #[tokio::test]
    async fn ws_gateway_requires_its_own_audience() {
        assert_service(AUDIENCE_WS_GATEWAY).await;
    }

    #[tokio::test]
    async fn expired_token_is_rejected() {
        let token = issue_token_at(
            SECRET,
            AUDIENCE_ORCHESTRATOR,
            unix_now() - 120,
            unix_now() - 60,
        );
        let status = status_of(
            app(AUDIENCE_ORCHESTRATOR),
            axum::http::Request::builder()
                .uri("/v1/ok")
                .header("authorization", format!("Bearer {token}"))
                .body(Body::empty())
                .unwrap(),
        )
        .await;
        assert_eq!(status, StatusCode::UNAUTHORIZED);
    }

    // Other tests reject a token before this one installs a subscriber. The
    // callsite then caches "never", and a later subscriber records nothing.
    // Rebuild interest after installing the subscriber, on this thread.
    #[tokio::test(flavor = "current_thread")]
    async fn rejection_log_does_not_contain_the_secret_or_token() {
        let secret = "s3cr3t-do-not-log";
        let app = protect(
            Router::new().route("/v1/ok", get(|| async { "ok" })),
            secret,
            AUDIENCE_ORCHESTRATOR,
        );
        // Prime the callsite the way the rest of the suite does.
        let _ = status_of(
            app.clone(),
            axum::http::Request::builder()
                .uri("/v1/ok")
                .body(Body::empty())
                .unwrap(),
        )
        .await;

        let buf = Arc::new(Mutex::new(Vec::<u8>::new()));
        let capture = buf.clone();
        let subscriber = tracing_subscriber::fmt()
            .with_max_level(tracing::Level::WARN)
            .with_ansi(false)
            .with_writer(move || Shared(capture.clone()))
            .finish();
        let guard = tracing::subscriber::set_default(subscriber);
        tracing::callsite::rebuild_interest_cache();

        let status = status_of(
            app,
            axum::http::Request::builder()
                .uri(format!("/v1/ok?access_token={secret}"))
                .header("authorization", format!("Bearer {secret}"))
                .body(Body::empty())
                .unwrap(),
        )
        .await;
        assert_eq!(status, StatusCode::UNAUTHORIZED);
        drop(guard);

        let logged = String::from_utf8(buf.lock().unwrap().clone()).unwrap();
        assert!(
            logged.contains("service auth rejected"),
            "rejection was not logged: {logged:?}"
        );
        assert!(
            !logged.contains(secret),
            "rejection log contains the secret: {logged:?}"
        );
    }

    fn ingest_app(secret: &str) -> Router {
        protect_ingest(
            Router::new()
                .route("/ingest", post(|| async { StatusCode::OK }))
                .route("/ingest/graylog", post(|| async { StatusCode::OK }))
                .route("/healthz", get(|| async { StatusCode::OK })),
            secret,
        )
    }

    #[tokio::test]
    async fn graylog_ingest_without_hmac_is_rejected() {
        let status = status_of(
            ingest_app(SECRET),
            axum::http::Request::builder()
                .method("POST")
                .uri("/ingest/graylog")
                .body(Body::from("{}"))
                .unwrap(),
        )
        .await;
        assert_eq!(status, StatusCode::UNAUTHORIZED);
        let health = status_of(
            ingest_app(SECRET),
            axum::http::Request::builder()
                .uri("/healthz")
                .body(Body::empty())
                .unwrap(),
        )
        .await;
        assert_eq!(health, StatusCode::OK);
    }

    #[tokio::test]
    async fn ingest_rejects_unsigned() {
        let status = status_of(
            ingest_app(SECRET),
            axum::http::Request::builder()
                .method("POST")
                .uri("/ingest")
                .body(Body::from("{}"))
                .unwrap(),
        )
        .await;
        assert_eq!(status, StatusCode::UNAUTHORIZED);
    }

    #[tokio::test]
    async fn ingest_rejects_replay() {
        let body = br#"{"event":"x"}"#;
        let old = unix_now() - INGEST_MAX_SKEW_SECS - 5;
        let sig = sign_ingest(SECRET, old, body);
        let status = status_of(
            ingest_app(SECRET),
            axum::http::Request::builder()
                .method("POST")
                .uri("/ingest")
                .header("x-attest-timestamp", old.to_string())
                .header("x-attest-signature", sig)
                .body(Body::from(body.to_vec()))
                .unwrap(),
        )
        .await;
        assert_eq!(status, StatusCode::UNAUTHORIZED);
    }

    #[tokio::test]
    async fn ingest_accepts_a_fresh_signature() {
        let body = br#"{"event":"x"}"#;
        let now = unix_now();
        let sig = sign_ingest(SECRET, now, body);
        let status = status_of(
            ingest_app(SECRET),
            axum::http::Request::builder()
                .method("POST")
                .uri("/ingest")
                .header("x-attest-timestamp", now.to_string())
                .header("x-attest-signature", sig)
                .body(Body::from(body.to_vec()))
                .unwrap(),
        )
        .await;
        assert_eq!(status, StatusCode::OK);
    }

    struct Shared(Arc<Mutex<Vec<u8>>>);
    impl Write for Shared {
        fn write(&mut self, data: &[u8]) -> std::io::Result<usize> {
            self.0.lock().unwrap().extend_from_slice(data);
            Ok(data.len())
        }
        fn flush(&mut self) -> std::io::Result<()> {
            Ok(())
        }
    }
}
