//! Axum HTTP server for the orchestrator.
//!
//! Routes:
//!   GET  /healthz           — liveness probe
//!   POST /triage            — run the Hybrid triage loop
//!   GET  /agent             — return the loaded agent definition
//!   GET  /metrics           — triage latency p50/p95/p99 over last 200 calls
//!   GET  /v1/attestations           — envelope summaries
//!   GET  /v1/attestations/export    — NDJSON
//!   GET  /v1/attestations/verify    — walk the hash chain
//!   GET  /v1/attestations/{id}      — one envelope
//!   GET  /docs              — Scalar interactive API docs

use crate::coordinator::{route, CoordinateRequest};
use crate::specialists::{run_hunt, run_respond, HuntRequest, RespondRequest};
use crate::triage::{TriageEngine, TriageRequest};
use attest_attestation::{verify_log, Verdict};
use axum::{
    extract::{Path, State},
    http::{HeaderMap, StatusCode},
    middleware::from_fn,
    response::{IntoResponse, Json},
    routing::{get, post},
    Router,
};
use chrono::{DateTime, Utc};
use std::collections::VecDeque;
use std::sync::{Arc, Mutex};
use utoipa::OpenApi;
use utoipa_scalar::{Scalar, Servable as _};
use uuid::Uuid;

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

#[derive(serde::Deserialize, utoipa::ToSchema)]
pub struct CaseOverrideRequest {
    /// Target `Verdict` label (`snake_case`, e.g. `false_positive`).
    pub label: String,
    pub reason: String,
}

#[derive(serde::Serialize, utoipa::ToSchema)]
pub struct CaseOverrideResponse {
    pub agent_action_id: Uuid,
    pub latency_ms: u64,
    pub signed_at: DateTime<Utc>,
}

// ── OpenAPI spec ─────────────────────────────────────────────────────────────

#[derive(OpenApi)]
#[openapi(
    paths(
        healthz,
        handle_triage,
        handle_coordinate,
        handle_hunt,
        handle_respond,
        handle_agent_info,
        handle_metrics,
        handle_case_override,
        handle_list_attestations,
        handle_export_attestations,
        handle_verify_attestations,
        handle_get_attestation,
    ),
    components(schemas(
        HealthOk,
        AgentInfo,
        OrchestratorMetrics,
        CaseOverrideRequest,
        CaseOverrideResponse,
        crate::triage::TriageRequest,
        crate::triage::TriageVerdict,
        crate::triage::InvestigationSummary,
        crate::coordinator::CoordinateRequest,
        crate::coordinator::CoordinateDecision,
        crate::specialists::HuntRequest,
        crate::specialists::HuntResult,
        crate::specialists::RespondRequest,
        crate::specialists::RespondResult,
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
        .route("/healthz", get(healthz))
        .route("/triage", post(handle_triage))
        .route("/v1/coordinate", post(handle_coordinate))
        .route("/v1/hunt", post(handle_hunt))
        .route("/v1/respond", post(handle_respond))
        .route("/agent", get(handle_agent_info))
        .route("/metrics", get(handle_metrics))
        .route("/v1/cases/{case_id}/override", post(handle_case_override))
        .route("/v1/attestations", get(handle_list_attestations))
        .route("/v1/attestations/export", get(handle_export_attestations))
        .route("/v1/attestations/verify", get(handle_verify_attestations))
        .route("/v1/attestations/{action_id}", get(handle_get_attestation))
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
            (
                StatusCode::OK,
                Json(serde_json::to_value(&verdict).unwrap()),
            )
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

fn load_prompt(env_key: &str, default_path: &str) -> (String, String) {
    let path = std::env::var(env_key).unwrap_or_else(|_| default_path.into());
    let text = std::fs::read_to_string(&path).unwrap_or_default();
    let hash = {
        use sha2::{Digest, Sha256};
        hex::encode(Sha256::digest(text.as_bytes()))
    };
    (text, hash)
}

#[utoipa::path(
    post,
    path = "/v1/coordinate",
    request_body = CoordinateRequest,
    responses((status = 200, description = "Routing decision", body = crate::coordinator::CoordinateDecision)),
    tag = "agents"
)]
async fn handle_coordinate(Json(req): Json<CoordinateRequest>) -> impl IntoResponse {
    (StatusCode::OK, Json(route(req)))
}

#[utoipa::path(
    post,
    path = "/v1/hunt",
    request_body = HuntRequest,
    responses((status = 200, description = "Hunt result", body = crate::specialists::HuntResult)),
    tag = "agents"
)]
async fn handle_hunt(
    State(state): State<OrchestratorState>,
    Json(req): Json<HuntRequest>,
) -> impl IntoResponse {
    let (prompt, hash) = load_prompt("HUNTER_PROMPT_PATH", "./agents/hunter/system_prompt_v1.md");
    let agent_id = std::env::var("HUNTER_AGENT_ID").unwrap_or_else(|_| "hunter-v1".into());
    match run_hunt(
        req,
        state.engine.llm(),
        state.engine.mcp(),
        state.engine.signer(),
        state.engine.attestation_log(),
        &prompt,
        &hash,
        &agent_id,
    )
    .await
    {
        Ok(out) => (StatusCode::OK, Json(serde_json::to_value(&out).unwrap())).into_response(),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({ "error": e.to_string() })),
        )
            .into_response(),
    }
}

#[utoipa::path(
    post,
    path = "/v1/respond",
    request_body = RespondRequest,
    responses((status = 200, description = "Responder result", body = crate::specialists::RespondResult)),
    tag = "agents"
)]
async fn handle_respond(
    State(state): State<OrchestratorState>,
    Json(req): Json<RespondRequest>,
) -> impl IntoResponse {
    let (prompt, hash) = load_prompt(
        "RESPONDER_PROMPT_PATH",
        "./agents/responder/system_prompt_v1.md",
    );
    let agent_id = std::env::var("RESPONDER_AGENT_ID").unwrap_or_else(|_| "responder-v1".into());
    match run_respond(
        req,
        state.engine.llm(),
        state.engine.mcp(),
        state.engine.signer(),
        state.engine.attestation_log(),
        state.engine.shadow(),
        &prompt,
        &hash,
        &agent_id,
    )
    .await
    {
        Ok(out) => (StatusCode::OK, Json(serde_json::to_value(&out).unwrap())).into_response(),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({ "error": e.to_string() })),
        )
            .into_response(),
    }
}

/// Analyst override — append a signed `EvidenceBlock::Override` envelope and emit a trace step.
#[utoipa::path(
    post,
    path = "/v1/cases/{case_id}/override",
    params(
        ("case_id" = Uuid, Path, description = "Case UUID"),
    ),
    request_body = CaseOverrideRequest,
    responses(
        (status = 200, description = "Signed override envelope appended", body = CaseOverrideResponse),
        (status = 400, description = "Bad request (missing headers or invalid verdict label)"),
        (status = 404, description = "No prior envelope for case"),
        (status = 500, description = "Internal error"),
    ),
    tag = "cases"
)]
async fn handle_case_override(
    State(state): State<OrchestratorState>,
    Path(case_id): Path<Uuid>,
    headers: HeaderMap,
    Json(body): Json<CaseOverrideRequest>,
) -> impl IntoResponse {
    let actor_id = headers
        .get("x-actor-id")
        .and_then(|v| v.to_str().ok())
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty());
    let actor_email = headers
        .get("x-actor-email")
        .and_then(|v| v.to_str().ok())
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty());

    let (Some(actor_id), Some(actor_email)) = (actor_id, actor_email) else {
        return (
            StatusCode::BAD_REQUEST,
            Json(serde_json::json!({
                "error": "missing X-Actor-Id or X-Actor-Email",
            })),
        )
            .into_response();
    };

    let corrected: Verdict = match serde_json::from_value(serde_json::Value::String(body.label)) {
        Ok(v) => v,
        Err(e) => {
            return (
                StatusCode::BAD_REQUEST,
                Json(serde_json::json!({ "error": format!("invalid label: {e}") })),
            )
                .into_response();
        }
    };

    match state
        .engine
        .append_human_override(case_id, corrected, body.reason, actor_id, actor_email)
        .await
    {
        Ok((agent_action_id, latency_ms, signed_at)) => (
            StatusCode::OK,
            Json(CaseOverrideResponse {
                agent_action_id,
                latency_ms,
                signed_at,
            }),
        )
            .into_response(),
        Err(e) if e.to_string().contains("no attestation envelope") => (
            StatusCode::NOT_FOUND,
            Json(serde_json::json!({ "error": e.to_string() })),
        )
            .into_response(),
        Err(e) => {
            tracing::error!(error = %e, "case override failed");
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(serde_json::json!({ "error": e.to_string() })),
            )
                .into_response()
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

/// Summaries of every signed envelope currently on the log.
#[utoipa::path(
    get,
    path = "/v1/attestations",
    responses((status = 200, description = "Envelope summaries")),
    tag = "attestations"
)]
async fn handle_list_attestations(State(state): State<OrchestratorState>) -> impl IntoResponse {
    match state.engine.attestation_log().read_all().await {
        Ok(envs) => {
            let rows: Vec<serde_json::Value> = envs
                .iter()
                .map(|e| {
                    serde_json::json!({
                        "agent_action_id": e.agent_action_id,
                        "case_id": e.case_id,
                        "tenant_id": e.tenant_id,
                        "verdict": e.verdict,
                        "execution_path": e.execution_path,
                        "prev_hash": e.prev_hash,
                        "signed_at": e.signed_at,
                    })
                })
                .collect();
            (
                StatusCode::OK,
                Json(serde_json::json!({ "envelopes": rows })),
            )
        }
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({ "error": e.to_string() })),
        ),
    }
}

/// Download the log as NDJSON (same bytes `attest verify` consumes).
#[utoipa::path(
    get,
    path = "/v1/attestations/export",
    responses((status = 200, description = "NDJSON attestation log")),
    tag = "attestations"
)]
async fn handle_export_attestations(State(state): State<OrchestratorState>) -> impl IntoResponse {
    match state.engine.attestation_log().export_ndjson().await {
        Ok(body) => (
            StatusCode::OK,
            [("content-type", "application/x-ndjson")],
            body,
        )
            .into_response(),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({ "error": e.to_string() })),
        )
            .into_response(),
    }
}

/// Walk the hash chain with the process verifying key.
#[utoipa::path(
    get,
    path = "/v1/attestations/verify",
    responses((status = 200, description = "Verify report")),
    tag = "attestations"
)]
async fn handle_verify_attestations(State(state): State<OrchestratorState>) -> impl IntoResponse {
    match state.engine.attestation_log().read_all().await {
        Ok(envs) => {
            let report = verify_log(&envs, &state.engine.verifying_key);
            (
                StatusCode::OK,
                Json(serde_json::json!({
                    "ok": report.all_ok(),
                    "passed": report.passed(),
                    "total": report.results.len(),
                    "verifying_key": state.engine.verifying_key,
                    "durable": state.engine.attestation_log().has_object_store(),
                    "results": report.results,
                })),
            )
        }
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({ "error": e.to_string() })),
        ),
    }
}

/// One signed envelope by `agent_action_id`.
#[utoipa::path(
    get,
    path = "/v1/attestations/{action_id}",
    params(("action_id" = Uuid, Path, description = "agent_action_id")),
    responses(
        (status = 200, description = "Signed envelope"),
        (status = 404, description = "Not found"),
    ),
    tag = "attestations"
)]
async fn handle_get_attestation(
    State(state): State<OrchestratorState>,
    Path(action_id): Path<Uuid>,
) -> impl IntoResponse {
    match state.engine.attestation_log().read_all().await {
        Ok(envs) => match envs.into_iter().find(|e| e.agent_action_id == action_id) {
            Some(env) => {
                (StatusCode::OK, Json(serde_json::to_value(&env).unwrap())).into_response()
            }
            None => (
                StatusCode::NOT_FOUND,
                Json(serde_json::json!({ "error": "envelope not found" })),
            )
                .into_response(),
        },
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({ "error": e.to_string() })),
        )
            .into_response(),
    }
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
