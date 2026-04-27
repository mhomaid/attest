//! HTTP ingest endpoint for the collector.
//!
//! `POST /ingest` — accepts a CloudTrail JSON payload (either a bare record or
//! the `{"Records": [...]}` envelope), normalizes it, and produces to Redpanda.
//! Returns `{"event_ids": ["<uuid>", ...]}` for all events produced.

use axum::{
    Json,
    extract::State,
    http::StatusCode,
    response::IntoResponse,
};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use uuid::Uuid;

use crate::{error::CollectorError, normalizer::normalize_cloudtrail, producer::EventProducer};

#[derive(Clone)]
pub struct AppState {
    pub producer: Arc<EventProducer>,
    pub tenant_id: String,
}

#[derive(Serialize)]
pub struct IngestResponse {
    pub event_ids: Vec<Uuid>,
}

#[derive(Serialize)]
#[allow(dead_code)]
pub struct ErrorResponse {
    pub error: String,
}

pub async fn healthz() -> impl IntoResponse {
    (StatusCode::OK, Json(serde_json::json!({"status": "ok"})))
}

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
