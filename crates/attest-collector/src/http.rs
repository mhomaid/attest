//! HTTP ingest endpoint for the collector.

use axum::{extract::State, http::HeaderMap, http::StatusCode, response::IntoResponse, Json};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use utoipa::ToSchema;
use uuid::Uuid;

use crate::{
    edge::{EdgeAuditSource, EdgeFirewallSource},
    error::CollectorError,
    graylog::{heartbeat_missed, DefinitionHeartbeat, GraylogSource},
    mapping::FieldMapping,
    normalizer::normalize_cloudtrail,
    producer::EventProducer,
    s3::{decode_cloudtrail_bytes, get_object, is_s3_notification, pointers_from_notification},
    source::{NormalizeCtx, Source},
};

#[derive(Clone)]
pub struct AppState {
    pub producer: Arc<EventProducer>,
    pub tenant_id: String,
    pub mapping: FieldMapping,
}

#[derive(Serialize, ToSchema)]
pub struct IngestResponse {
    /// UUIDs of every event produced to Kafka.
    pub event_ids: Vec<Uuid>,
}

#[derive(Serialize, ToSchema)]
#[allow(dead_code)]
pub struct ErrorResponse {
    pub error: String,
}

#[derive(Serialize, ToSchema)]
pub struct HealthResponse {
    pub status: String,
}

/// Liveness probe — returns `{"status":"ok"}` when the process is up.
#[utoipa::path(
    get,
    path = "/healthz",
    responses(
        (status = 200, description = "Service healthy", body = HealthResponse),
    ),
    tag = "ops"
)]
pub async fn healthz() -> impl IntoResponse {
    (StatusCode::OK, Json(serde_json::json!({"status": "ok"})))
}

/// Ingest one or more CloudTrail records.
///
/// Accepts a bare record, the `{"Records": [...]}` envelope, or an S3
/// object-created notification. Tenant comes from `X-Tenant-Id`, then `TENANT_ID`.
#[utoipa::path(
    post,
    path = "/ingest",
    request_body(
        content = serde_json::Value,
        description = "CloudTrail record, {\"Records\":[...]} envelope, or S3 notification",
        content_type = "application/json"
    ),
    responses(
        (status = 200, description = "Events accepted", body = IngestResponse),
        (status = 400, description = "Malformed payload", body = ErrorResponse),
    ),
    tag = "ingest"
)]
pub async fn ingest(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(body): Json<serde_json::Value>,
) -> impl IntoResponse {
    let tenant = tenant_from_headers(&headers, &state.tenant_id);
    match handle_ingest(state, &tenant, body).await {
        Ok(resp) => (StatusCode::OK, Json(serde_json::to_value(resp).unwrap())),
        Err(e) => (
            StatusCode::BAD_REQUEST,
            Json(serde_json::json!({"error": e.to_string()})),
        ),
    }
}

/// Fetch a CloudTrail object from S3/MinIO and ingest it.
///
/// Body is `{ "bucket", "key" }` or the same S3 notification shape as `/ingest`.
#[utoipa::path(
    post,
    path = "/ingest/s3",
    request_body(
        content = serde_json::Value,
        description = "{\"bucket\":\"…\",\"key\":\"…\"} or an S3 object-created event",
        content_type = "application/json"
    ),
    responses(
        (status = 200, description = "Events accepted", body = IngestResponse),
        (status = 400, description = "Fetch or parse failed", body = ErrorResponse),
    ),
    tag = "ingest"
)]
pub async fn ingest_s3(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(body): Json<serde_json::Value>,
) -> impl IntoResponse {
    let tenant = tenant_from_headers(&headers, &state.tenant_id);
    match handle_s3_ingest(state, &tenant, body).await {
        Ok(resp) => (StatusCode::OK, Json(serde_json::to_value(resp).unwrap())),
        Err(e) => (
            StatusCode::BAD_REQUEST,
            Json(serde_json::json!({"error": e.to_string()})),
        ),
    }
}

pub(crate) async fn handle_ingest(
    state: AppState,
    tenant_id: &str,
    body: serde_json::Value,
) -> Result<IngestResponse, CollectorError> {
    if is_s3_notification(&body) {
        return handle_s3_ingest(state, tenant_id, body).await;
    }
    produce_cloudtrail(state, tenant_id, &body).await
}

async fn handle_s3_ingest(
    state: AppState,
    tenant_id: &str,
    body: serde_json::Value,
) -> Result<IngestResponse, CollectorError> {
    let pointers = pointers_from_notification(&body);
    if pointers.is_empty() {
        return Err(CollectorError::S3(
            "body is not {bucket,key} and has no S3 Records".into(),
        ));
    }

    let mut ids = Vec::new();
    for pointer in pointers {
        let bytes = get_object(&pointer.bucket, &pointer.key)
            .await
            .map_err(|e| CollectorError::S3(e.to_string()))?;
        let payload = decode_cloudtrail_bytes(&pointer.key, bytes)
            .map_err(|e| CollectorError::S3(e.to_string()))?;
        let mut part = produce_cloudtrail(state.clone(), tenant_id, &payload).await?;
        ids.append(&mut part.event_ids);
    }
    Ok(IngestResponse { event_ids: ids })
}

async fn produce_cloudtrail(
    state: AppState,
    tenant_id: &str,
    body: &serde_json::Value,
) -> Result<IngestResponse, CollectorError> {
    let events = normalize_cloudtrail(body, tenant_id)?;
    let mut ids = Vec::with_capacity(events.len());

    for event in &events {
        state.producer.produce(event).await?;
        ids.push(event.event_id());
    }

    tracing::info!(count = ids.len(), tenant = tenant_id, "ingested events");
    Ok(IngestResponse { event_ids: ids })
}

pub fn source_flag_on(value: Option<&str>) -> bool {
    matches!(value, Some("on") | Some("1") | Some("true"))
}

/// Graylog HTTP notification. Off unless `ATTEST_SOURCE_GRAYLOG=on`.
pub async fn ingest_graylog(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(body): Json<serde_json::Value>,
) -> impl IntoResponse {
    if !source_flag_on(std::env::var("ATTEST_SOURCE_GRAYLOG").ok().as_deref()) {
        return (
            StatusCode::NOT_FOUND,
            Json(serde_json::json!({"error": "graylog source is off"})),
        );
    }
    let bytes = serde_json::to_vec(&body).unwrap_or_default();
    let ctx = NormalizeCtx {
        tenant_id: tenant_from_headers(&headers, &state.tenant_id),
        extension_prefix: state.mapping.prefix.clone(),
    };
    let records = match GraylogSource.decode(&bytes, Some("application/json")) {
        Ok(v) => v,
        Err(e) => {
            return (
                StatusCode::BAD_REQUEST,
                Json(serde_json::json!({"error": e.to_string()})),
            )
        }
    };
    if let Some(beats) = body.get("heartbeats").and_then(|v| v.as_array()) {
        let now = chrono::Utc::now().timestamp();
        for beat in beats {
            let def = DefinitionHeartbeat {
                id: beat
                    .get("id")
                    .and_then(|v| v.as_str())
                    .unwrap_or("unknown")
                    .to_string(),
                interval_secs: beat
                    .get("interval_secs")
                    .and_then(|v| v.as_i64())
                    .unwrap_or(300),
                last_run_unix: beat.get("last_run_unix").and_then(|v| v.as_i64()),
            };
            if heartbeat_missed(&def, now) {
                tracing::warn!(id = %def.id, "graylog definition heartbeat missed");
            }
        }
    }
    match produce_source(&state, &ctx, &GraylogSource, &records).await {
        Ok(resp) => (StatusCode::OK, Json(serde_json::to_value(resp).unwrap())),
        Err(e) => (
            StatusCode::BAD_REQUEST,
            Json(serde_json::json!({"error": e.to_string()})),
        ),
    }
}

/// Edge firewall JSON, or an audit object when `kind` is `audit`.
/// Off unless `ATTEST_SOURCE_EDGE=on`.
pub async fn ingest_edge(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(body): Json<serde_json::Value>,
) -> impl IntoResponse {
    if !source_flag_on(std::env::var("ATTEST_SOURCE_EDGE").ok().as_deref()) {
        return (
            StatusCode::NOT_FOUND,
            Json(serde_json::json!({"error": "edge source is off"})),
        );
    }
    let bytes = serde_json::to_vec(&body).unwrap_or_default();
    let ctx = NormalizeCtx {
        tenant_id: tenant_from_headers(&headers, &state.tenant_id),
        extension_prefix: state.mapping.prefix.clone(),
    };
    let audit = body.get("kind").and_then(|v| v.as_str()) == Some("audit");
    let produced = if audit {
        decode_and_produce(&state, &ctx, &EdgeAuditSource, &bytes).await
    } else {
        decode_and_produce(&state, &ctx, &EdgeFirewallSource, &bytes).await
    };
    match produced {
        Ok(resp) => (StatusCode::OK, Json(serde_json::to_value(resp).unwrap())),
        Err(e) => (
            StatusCode::BAD_REQUEST,
            Json(serde_json::json!({"error": e.to_string()})),
        ),
    }
}

async fn decode_and_produce(
    state: &AppState,
    ctx: &NormalizeCtx,
    source: &impl Source,
    bytes: &[u8],
) -> Result<IngestResponse, CollectorError> {
    let records = source.decode(bytes, Some("application/json"))?;
    produce_source(state, ctx, source, &records).await
}

async fn produce_source(
    state: &AppState,
    ctx: &NormalizeCtx,
    source: &impl Source,
    records: &[serde_json::Value],
) -> Result<IngestResponse, CollectorError> {
    let mut ids = Vec::new();
    for record in records {
        let event = source.normalize(record, ctx)?;
        state
            .producer
            .produce_source(&event, source.id(), source.is_alert())
            .await?;
        ids.push(event.event_id());
    }
    Ok(IngestResponse { event_ids: ids })
}

/// `X-Tenant-Id` wins when it is a short ASCII token; otherwise the process default.
pub fn tenant_from_headers(headers: &HeaderMap, fallback: &str) -> String {
    headers
        .get("x-tenant-id")
        .and_then(|v| v.to_str().ok())
        .map(str::trim)
        .filter(|s| {
            !s.is_empty()
                && s.len() <= 128
                && s.chars()
                    .all(|c| c.is_ascii_alphanumeric() || matches!(c, '-' | '_' | '.'))
        })
        .map(ToOwned::to_owned)
        .unwrap_or_else(|| fallback.to_string())
}

#[derive(Deserialize)]
#[allow(dead_code)]
pub struct FileIngestRequest {
    pub path: String,
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::http::HeaderValue;

    #[test]
    fn header_tenant_wins() {
        let mut headers = HeaderMap::new();
        headers.insert("x-tenant-id", HeaderValue::from_static("acme"));
        assert_eq!(tenant_from_headers(&headers, "default"), "acme");
    }

    #[test]
    fn rejects_injection_in_tenant_header() {
        let mut headers = HeaderMap::new();
        headers.insert("x-tenant-id", HeaderValue::from_static("acme; drop table"));
        assert_eq!(tenant_from_headers(&headers, "default"), "default");
    }

    #[test]
    fn empty_header_falls_back() {
        let headers = HeaderMap::new();
        assert_eq!(tenant_from_headers(&headers, "hosted"), "hosted");
    }
}
