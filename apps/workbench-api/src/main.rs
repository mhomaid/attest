//! Workbench API — read-only case trace from the attestation NDJSON log (Phase 7 MVP).

use attest_attestation::AttestationEnvelope;
use axum::{
    extract::Path,
    http::StatusCode,
    middleware,
    response::{IntoResponse, Json},
    routing::get,
    Router,
};
use serde::Serialize;
use std::path::PathBuf;
use uuid::Uuid;

#[derive(Serialize)]
struct TraceStep {
    /// triage | investigator | auto_close | other
    kind: String,
    pub agent_action_id: Uuid,
    pub agent_id: String,
    pub execution_path: String,
    pub verdict: String,
    pub tool_call_count: usize,
    pub belief_count: usize,
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let service_name =
        std::env::var("OTEL_SERVICE_NAME").unwrap_or_else(|_| "attest-workbench-api".into());
    let _otel = attest_telemetry::init_subscriber_with_otel(&service_name)?;

    let port: u16 = std::env::var("WORKBENCH_API_PORT")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(4400);

    let log_path = PathBuf::from(
        std::env::var("ATTEST_LOG_PATH").unwrap_or_else(|_| "./attestations.ndjson".into()),
    );

    let app = Router::new()
        .route("/healthz", get(|| async { "ok" }))
        .route("/v1/cases/{case_id}/trace", get(case_trace))
        .with_state(log_path)
        .layer(middleware::from_fn(
            attest_telemetry::axum_trace_propagation,
        ));

    let addr = format!("0.0.0.0:{port}");
    tracing::info!("workbench-api listening on {addr}");
    let listener = tokio::net::TcpListener::bind(&addr).await?;
    axum::serve(listener, app).await?;
    Ok(())
}

async fn case_trace(
    Path(case_id): Path<Uuid>,
    axum::extract::State(log_path): axum::extract::State<PathBuf>,
) -> impl IntoResponse {
    let envelopes = match read_log(&log_path).await {
        Ok(e) => e,
        Err(e) => {
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(serde_json::json!({ "error": e.to_string() })),
            )
                .into_response();
        }
    };

    let mut steps: Vec<TraceStep> = Vec::new();
    for env in envelopes.into_iter().filter(|e| e.case_id == case_id) {
        let (tool_n, belief_n, kind) = summarize_envelope(&env);
        steps.push(TraceStep {
            kind,
            agent_action_id: env.agent_action_id,
            agent_id: env.agent_id.clone(),
            execution_path: format!("{:?}", env.execution_path).to_ascii_lowercase(),
            verdict: format!("{:?}", env.verdict).to_ascii_lowercase(),
            tool_call_count: tool_n,
            belief_count: belief_n,
        });
    }

    Json(serde_json::json!({
        "case_id": case_id,
        "steps": steps,
    }))
    .into_response()
}

async fn read_log(path: &std::path::Path) -> anyhow::Result<Vec<AttestationEnvelope>> {
    if !path.exists() {
        return Ok(vec![]);
    }
    let content = tokio::fs::read_to_string(path).await?;
    let mut out = Vec::new();
    for line in content.lines() {
        if line.is_empty() {
            continue;
        }
        if let Ok(env) = serde_json::from_str::<AttestationEnvelope>(line) {
            out.push(env);
        }
    }
    Ok(out)
}

fn summarize_envelope(env: &AttestationEnvelope) -> (usize, usize, String) {
    use attest_attestation::EvidenceBlock;
    let kind = if env.agent_id.contains("investigator") {
        "investigator"
    } else if matches!(env.evidence, EvidenceBlock::AutoClose(_)) {
        "auto_close"
    } else {
        "triage"
    }
    .to_string();

    match &env.evidence {
        EvidenceBlock::Classifier(_) => (0, 0, kind),
        EvidenceBlock::Llm(ev) => (ev.tool_calls.len(), ev.intermediate_beliefs.len(), kind),
        EvidenceBlock::Hybrid(h) => (
            h.llm_final.tool_calls.len(),
            h.llm_final.intermediate_beliefs.len(),
            kind,
        ),
        EvidenceBlock::EscalatedStub { .. } => (0, 0, kind),
        EvidenceBlock::AutoClose(_) => (0, 0, kind),
        EvidenceBlock::Override(_) => (0, 0, "human_override".into()),
    }
}
