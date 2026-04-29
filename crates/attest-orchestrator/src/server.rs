//! Axum HTTP server for the orchestrator.
//!
//! Routes:
//!   GET  /healthz           — liveness probe
//!   POST /triage            — run the Hybrid triage loop
//!   GET  /agent             — return the loaded agent definition
//!   GET  /metrics           — triage latency p50/p95/p99 over last 200 calls

use crate::triage::{TriageEngine, TriageRequest};
use axum::{
    extract::State,
    http::StatusCode,
    response::{IntoResponse, Json},
    routing::{get, post},
    Router,
};
use std::collections::VecDeque;
use std::sync::{Arc, Mutex};

const LATENCY_WINDOW: usize = 200;

#[derive(Clone)]
pub struct OrchestratorState {
    pub engine: Arc<TriageEngine>,
    /// Sliding window of the last LATENCY_WINDOW triage latencies (ms).
    pub latencies: Arc<Mutex<VecDeque<u64>>>,
}

pub fn build_router(engine: TriageEngine) -> Router {
    let state = OrchestratorState {
        engine: Arc::new(engine),
        latencies: Arc::new(Mutex::new(VecDeque::with_capacity(LATENCY_WINDOW))),
    };
    Router::new()
        .route("/healthz",  get(healthz))
        .route("/triage",   post(handle_triage))
        .route("/agent",    get(handle_agent_info))
        .route("/metrics",  get(handle_metrics))
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
        Ok(verdict) => {
            // Record latency in the sliding window.
            if let Ok(mut w) = state.latencies.lock() {
                if w.len() >= LATENCY_WINDOW {
                    w.pop_front();
                }
                w.push_back(verdict.latency_ms);
            }
            (StatusCode::OK, Json(serde_json::to_value(&verdict).unwrap()))
        }
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
        "attestation_verifying_key": state.engine.verifying_key,
    }))
}

/// Returns latency percentiles (ms) derived from the sliding window.
async fn handle_metrics(State(state): State<OrchestratorState>) -> Json<serde_json::Value> {
    let window: Vec<u64> = state
        .latencies
        .lock()
        .map(|w| w.iter().copied().collect())
        .unwrap_or_default();

    let (p50, p95, p99, count) = percentiles(&window);
    Json(serde_json::json!({
        "triage_count": count,
        "p50_ms": p50,
        "p95_ms": p95,
        "p99_ms": p99,
    }))
}

/// Compute p50, p95, p99 from an unsorted sample.  Returns (p50, p95, p99, n).
fn percentiles(samples: &[u64]) -> (f64, f64, f64, usize) {
    let n = samples.len();
    if n == 0 {
        return (0.0, 0.0, 0.0, 0);
    }
    let mut sorted = samples.to_vec();
    sorted.sort_unstable();
    let pct = |p: f64| -> f64 {
        let idx = ((p / 100.0) * (n - 1) as f64).round() as usize;
        sorted[idx.min(n - 1)] as f64
    };
    (pct(50.0), pct(95.0), pct(99.0), n)
}
