//! HTTP ingest endpoint for the collector.

use axum::{
    Json,
    extract::State,
    http::StatusCode,
    response::IntoResponse,
};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use utoipa::ToSchema;
use uuid::Uuid;

use crate::{error::CollectorError, normalizer::normalize_cloudtrail, producer::EventProducer};

#[derive(Clone)]
pub struct AppState {
    pub producer: Arc<EventProducer>,
    pub tenant_id: String,
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
/// Accepts either a bare record object or the `{"Records": [...]}` envelope.
/// Normalises every record to OCSF `FlatEvent` and produces to the
/// `cloudtrail` Kafka topic.
#[utoipa::path(
    post,
    path = "/ingest",
    request_body(
        content = serde_json::Value,
        description = "CloudTrail record or {\"Records\":[...]} envelope",
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
    Json(body): Json<serde_json::Value>,
) -> impl IntoResponse {
    match handle_ingest(state, body).await {
        Ok(resp) => (StatusCode::OK, Json(serde_json::to_value(resp).unwrap())),
        Err(e) => (
            StatusCode::BAD_REQUEST,
            Json(serde_json::json!({"error": e.to_string()})),
        ),
    }
}

async fn handle_ingest(
    state: AppState,
    body: serde_json::Value,
) -> Result<IngestResponse, CollectorError> {
    let events = normalize_cloudtrail(&body, &state.tenant_id)?;
    let mut ids = Vec::with_capacity(events.len());

    for event in &events {
        state.producer.produce(event).await?;
        ids.push(event.event_id());
    }

    tracing::info!(count = ids.len(), "ingested events");
    Ok(IngestResponse { event_ids: ids })
}

#[derive(Deserialize)]
#[allow(dead_code)]
pub struct FileIngestRequest {
    pub path: String,
}
