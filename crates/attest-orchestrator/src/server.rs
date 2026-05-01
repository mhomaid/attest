//! Axum HTTP server for the orchestrator.
//!
//! Routes:
//!   GET  /healthz           — liveness probe
//!   POST /triage            — run the Hybrid triage loop
//!   GET  /agent             — return the loaded agent definition
//!   GET  /metrics           — triage latency p50/p95/p99 over last 200 calls
//!   GET  /docs              — Scalar interactive API docs

use crate::triage::{TriageEngine, TriageRequest};
use axum::{
    extract::State,
    http::StatusCode,
    middleware::from_fn,
    response::{IntoResponse, Json},
    routing::{get, post},
    Router,
};
use std::collections::VecDeque;
use std::sync::{Arc, Mutex};
use utoipa::OpenApi;
use utoipa_scalar::{Scalar, Servable as _};

const LATENCY_WINDOW: usize = 200;

#[derive(Clone)]
pub struct OrchestratorState {
    pub engine: Arc<TriageEngine>,
    /// Sliding window of the last LATENCY_WINDOW triage latencies (ms).
    pub latencies: Arc<Mutex<VecDeque<u64>>>,
}

// ── OpenAPI schema types ──────────────────────────────────────────────────────

#[derive(serde::Serialize, utoipa::ToSchema)]
pub struct HealthOk {
    pub status: &'static str,
}

#[derive(serde::Serialize, utoipa::ToSchema)]
pub struct AgentInfo {
    pub agent_id: String,
    pub role: String,
    pub version_hash: String,
    pub attestation_verifying_key: String,
}

#[derive(serde::Serialize, utoipa::ToSchema)]
pub struct OrchestratorMetrics {
    /// Number of triage calls in the current sliding window.
    pub triage_count: usize,
    pub p50_ms: f64,
    pub p95_ms: f64,
    pub p99_ms: f64,
}

// ── OpenAPI spec ─────────────────────────────────────────────────────────────

#[derive(OpenApi)]
#[openapi(
    paths(healthz, handle_triage, handle_agent_info, handle_metrics),
    components(schemas(
        HealthOk,
        AgentInfo,
        OrchestratorMetrics,
        crate::triage::TriageRequest,
        crate::triage::TriageVerdict,
        crate::triage::InvestigationSummary,
    )),
    info(
        title = "Attest Orchestrator",
        version = "0.1.0",
        description = "Hybrid ML + LLM triage engine. \
            POST /triage to classify an event; GET /metrics for sliding-window latency stats."
    )
)]
struct ApiDoc;

// ── Router ────────────────────────────────────────────────────────────────────

pub fn build_router(engine: TriageEngine) -> Router {
    let state = OrchestratorState {
        engine: Arc::new(engine),
        latencies: Arc::new(Mutex::new(VecDeque::with_capacity(LATENCY_WINDOW))),
    };
    let api = Router::new()
        .route("/healthz",  get(healthz))
        .route("/triage",   post(handle_triage))
        .route("/agent",    get(handle_agent_info))
        .route("/metrics",  get(handle_metrics))
        .with_state(state);

    Router::new()
        .merge(api)
        .merge(Scalar::with_url("/docs", ApiDoc::openapi()))
        .layer(from_fn(attest_telemetry::axum_trace_propagation))
}

// ── Handlers ─────────────────────────────────────────────────────────────────

/// Liveness probe.
#[utoipa::path(
    get,
    path = "/healthz",
    responses(
        (status = 200, description = "Service healthy", body = HealthOk),
    ),
    tag = "ops"
)]
async fn healthz() -> &'static str {
    "ok"
}

/// Run the Hybrid triage loop on a single event.
///
/// The engine first attempts the ONNX classifier. If calibrated confidence
/// falls below the escalation threshold, it escalates to the LLM path.
#[utoipa::path(
    post,
    path = "/triage",
    request_body(content = crate::triage::TriageRequest, description = "Event to triage"),
    responses(
        (status = 200, description = "Triage verdict", body = crate::triage::TriageVerdict),
        (status = 500, description = "Triage failed"),
    ),
    tag = "triage"
)]
async fn handle_triage(
    State(state): State<OrchestratorState>,
    Json(req): Json<TriageRequest>,
) -> impl IntoResponse {
    match state.engine.run_triage(req).await {
        Ok(verdict) => {
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

/// Return the loaded agent definition and Ed25519 verifying key.
#[utoipa::path(
    get,
    path = "/agent",
    responses(
        (status = 200, description = "Agent metadata", body = AgentInfo),
    ),
    tag = "ops"
)]
async fn handle_agent_info(State(state): State<OrchestratorState>) -> Json<serde_json::Value> {
    Json(serde_json::json!({
        "agent_id": state.engine.agent.id,
        "role": state.engine.agent.role,
        "version_hash": state.engine.agent.version_hash,
        "attestation_verifying_key": state.engine.verifying_key,
    }))
}

/// Return latency percentiles (ms) from the last 200 triage calls.
#[utoipa::path(
    get,
    path = "/metrics",
    responses(
        (status = 200, description = "Latency percentiles", body = OrchestratorMetrics),
    ),
    tag = "ops"
)]
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
