//! Axum HTTP server for the orchestrator.
//!
//! Routes:
//!   GET  /healthz           — liveness probe
//!   POST /triage            — run the Hybrid triage loop
//!   GET  /agent             — return the loaded agent definition

use crate::triage::{TriageEngine, TriageRequest};
use axum::{
    extract::State,
    http::StatusCode,
    response::{IntoResponse, Json},
    routing::{get, post},
    Router,
};
use std::sync::Arc;

#[derive(Clone)]
pub struct OrchestratorState {
    pub engine: Arc<TriageEngine>,
}

pub fn build_router(engine: TriageEngine) -> Router {
    let state = OrchestratorState { engine: Arc::new(engine) };
    Router::new()
        .route("/healthz", get(healthz))
        .route("/triage", post(handle_triage))
        .route("/agent", get(handle_agent_info))
        .with_state(state)
}

async fn healthz() -> &'static str {
    "ok"
}

async fn handle_triage(
    State(state): State<OrchestratorState>,
    Json(req): Json<TriageRequest>,
) -> impl IntoResponse {
    match state.engine.run_triage(req).await {
        Ok(verdict) => (StatusCode::OK, Json(serde_json::to_value(verdict).unwrap())),
        Err(e) => {
            tracing::error!(error = %e, "triage failed");
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(serde_json::json!({ "error": e.to_string() })),
            )
        }
    }
}

async fn handle_agent_info(State(state): State<OrchestratorState>) -> Json<serde_json::Value> {
    Json(serde_json::json!({
        "agent_id": state.engine.agent.id,
        "role": state.engine.agent.role,
        "version_hash": state.engine.agent.version_hash,
        // Ed25519 verifying key — use to verify attestation envelope signatures offline
        "attestation_verifying_key": state.engine.verifying_key,
    }))
}
