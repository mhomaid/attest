//! Shared helpers for the live end-to-end suites under `tests/`.

use std::time::{SystemTime, UNIX_EPOCH};

/// Dev defaults match `docker-compose.yml`. Override with the env vars when the stack uses another secret.
pub fn service_secret() -> String {
    std::env::var(attest_service_auth::SERVICE_AUTH_ENV)
        .unwrap_or_else(|_| "dev-only-service-auth-secret".into())
}

pub fn ingest_secret() -> String {
    std::env::var(attest_service_auth::INGEST_HMAC_ENV)
        .unwrap_or_else(|_| "dev-only-ingest-hmac-secret".into())
}

pub fn authed(request: reqwest::RequestBuilder, audience: &str) -> reqwest::RequestBuilder {
    request.header(
        "authorization",
        attest_service_auth::authorization_header(&service_secret(), audience),
    )
}

/// Raw service JWT for clients that cannot set headers, such as a WebSocket URL.
pub fn service_access_token(audience: &str) -> String {
    attest_service_auth::issue_token(
        &service_secret(),
        audience,
        attest_service_auth::SERVICE_TOKEN_TTL_SECS,
    )
}

/// HMAC the JSON body the same way the collector verifies it.
pub fn signed_ingest(
    request: reqwest::RequestBuilder,
    body: &serde_json::Value,
) -> reqwest::RequestBuilder {
    let bytes = serde_json::to_vec(body).expect("ingest body is json");
    let timestamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0);
    let signature = attest_service_auth::sign_ingest(&ingest_secret(), timestamp, &bytes);
    request
        .header("content-type", "application/json")
        .header("x-attest-timestamp", timestamp.to_string())
        .header("x-attest-signature", signature)
        .body(bytes)
}

pub use attest_service_auth::{
    AUDIENCE_CONTROL_PLANE, AUDIENCE_MCP_GATEWAY, AUDIENCE_ORCHESTRATOR, AUDIENCE_WS_GATEWAY,
};

/// Live suites need a running stack (`make dev-up`), so they only run with `ATTEST_E2E=1`.
pub fn e2e_enabled() -> bool {
    std::env::var("ATTEST_E2E").as_deref() == Ok("1")
}

/// Return early from a test unless live end-to-end tests are enabled.
#[macro_export]
macro_rules! require_e2e {
    () => {
        if !$crate::e2e_enabled() {
            eprintln!("SKIP: set ATTEST_E2E=1 with a running stack");
            return;
        }
    };
}
