//! Orchestrator binary.
//!
//! Configuration via environment variables:
//!   ORCHESTRATOR_PORT       HTTP port (default 4300)
//!   ARTIFACTS_DIR           Directory containing ONNX artifacts
//!                           (default ./ml/triager/artifacts)
//!   ESCALATION_THRESHOLD    Calibrated confidence below which Triager escalates (default 0.60)
//!   NOVELTY_THRESHOLD       Novelty score above which Triager escalates (default 0.70)
//!   ATTEST_SIGNING_KEY      Hex-encoded 32-byte Ed25519 seed (generated fresh if absent)
//!   CALIBRATION_URL         Calibration sidecar URL (default http://localhost:5001)
//!   ATTEST_LOG_PATH         Attestation log file path (default ./attestations.ndjson)
//!   MCP_GATEWAY_URL         MCP gateway URL (default http://localhost:4500)
//!   SYSTEM_PROMPT_PATH      Path to the triager system prompt (default ./agents/triager/system_prompt_v1.md)
//!   REVIEWER_PROMPT_PATH    Path to the reviewer system prompt (default ./agents/triager/reviewer_prompt_v1.md)
//!
//! Guardrails (Phase 5):
//!   ATTEST_GUARDRAILS             "on" (default) | "off"
//!   GUARDRAIL_MAX_RETRIES         Max re-prompts per check (default 3)
//!   CROSS_REVIEW_SEVERITY_THRESHOLD  Calibrated confidence above which cross-review fires (default 0.85)
//!
//! Auto-close (Phase 6):
//!   AUTO_CLOSE_THRESHOLD        Confidence floor for auto-close (default 0.90)
//!   DO_NOT_TOUCH_LIST           Comma-separated principals that are never auto-closed
//!   TENANT_ALLOWS_AUTOMATION    "true" (default) | "false"
//!   AUTO_CLOSE_ACTION_CLASSES   Comma-separated action classes eligible for auto-close
//!                               (default login,api_call,file_access)
//!
//! LLM inference (Phase 4b):
//!   ATTEST_LLM_PROVIDER     "local" (default) | "anthropic"
//!   ATTEST_LLM_BASE_URL     OpenAI-compat base URL for local provider
//!                           (default http://127.0.0.1:8888/v1)
//!   ATTEST_LLM_MODEL        Model ID (default unsloth/Qwen3.6-35B-A3B-GGUF)
//!   ANTHROPIC_API_KEY       Required when ATTEST_LLM_PROVIDER=anthropic
//!
//! Investigator (Phase 7):
//!   INVESTIGATOR_PROMPT_PATH     Path to investigator system prompt
//!                                (default ./agents/investigator/system_prompt_v1.md)
//!   INVESTIGATOR_AGENT_ID        Agent id on investigator envelopes (default investigator-v1)
//!   INVESTIGATOR_MAX_ITERATIONS  Max LLM tool rounds for Investigator (default 10)
//!
//! OpenTelemetry / Grafana Tempo (optional):
//!   OTEL_EXPORTER_OTLP_ENDPOINT  OTLP gRPC endpoint (e.g. http://127.0.0.1:4317)
//!   OTEL_SERVICE_NAME            Override default service name for traces

use attest_attestation::Signer;
use attest_inference_router::from_env as llm_from_env;
use attest_orchestrator::{
    agent::{AgentDefinition, ClassifierArtifact, ExecutionPath},
    build_router,
    shadow_check::ShadowChecker,
    triage::TriageEngine,
    AgentRole,
};
use sha2::{Digest, Sha256};
use std::sync::Arc;
use tokio::net::TcpListener;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let service_name =
        std::env::var("OTEL_SERVICE_NAME").unwrap_or_else(|_| "attest-orchestrator".into());
    let _otel = attest_telemetry::init_subscriber_with_otel(&service_name)?;

    let port: u16 = std::env::var("ORCHESTRATOR_PORT")
        .unwrap_or_else(|_| "4300".into())
        .parse()?;

    let artifacts_dir = std::env::var("ARTIFACTS_DIR")
        .unwrap_or_else(|_| "./ml/triager/artifacts".into());

    let escalation_threshold: f32 = std::env::var("ESCALATION_THRESHOLD")
        .unwrap_or_else(|_| "0.60".into())
        .parse()?;

    let novelty_threshold: f32 = std::env::var("NOVELTY_THRESHOLD")
        .unwrap_or_else(|_| "0.70".into())
        .parse()?;

    // ── Classifier artifacts ──────────────────────────────────────────────────
    let model_hash_path = format!("{artifacts_dir}/model_hash.txt");
    let model_artifact_hash = std::fs::read_to_string(&model_hash_path)
        .unwrap_or_else(|_| "unknown".into())
        .trim()
        .to_string();

    let feature_hash_path = format!("{artifacts_dir}/feature_hash.txt");
    let feature_extractor_hash = std::fs::read_to_string(&feature_hash_path)
        .unwrap_or_else(|_| "unknown".into())
        .trim()
        .to_string();

    let artifact = ClassifierArtifact {
        model_path: format!("{artifacts_dir}/model.onnx"),
        shap_background_path: Some(format!("{artifacts_dir}/shap_background.npy")),
        novelty_mean_path: format!("{artifacts_dir}/novelty_mean.npy"),
        novelty_inv_cov_path: format!("{artifacts_dir}/novelty_inv_cov.npy"),
        novelty_threshold_path: format!("{artifacts_dir}/novelty_threshold.txt"),
        model_artifact_hash,
        feature_extractor_hash,
        escalation_threshold,
        novelty_threshold,
    };

    // ── System prompt ─────────────────────────────────────────────────────────
    let system_prompt_path = std::env::var("SYSTEM_PROMPT_PATH")
        .unwrap_or_else(|_| "./agents/triager/system_prompt_v1.md".into());

    let system_prompt = std::fs::read_to_string(&system_prompt_path)
        .unwrap_or_else(|e| {
            tracing::warn!(path = %system_prompt_path, error = %e, "system prompt file not found — using empty prompt");
            String::new()
        });

    let system_prompt_hash = hex::encode(Sha256::digest(system_prompt.as_bytes()));
    tracing::info!(
        path = %system_prompt_path,
        hash = %&system_prompt_hash[..16],
        bytes = system_prompt.len(),
        "System prompt loaded"
    );

    // ── Reviewer prompt ───────────────────────────────────────────────────────
    let reviewer_prompt_path = std::env::var("REVIEWER_PROMPT_PATH")
        .unwrap_or_else(|_| "./agents/triager/reviewer_prompt_v1.md".into());

    let reviewer_prompt = std::fs::read_to_string(&reviewer_prompt_path)
        .unwrap_or_else(|e| {
            tracing::warn!(path = %reviewer_prompt_path, error = %e, "reviewer prompt file not found — cross-review will be a no-op");
            String::new()
        });

    let reviewer_prompt_hash = hex::encode(Sha256::digest(reviewer_prompt.as_bytes()));
    tracing::info!(
        path = %reviewer_prompt_path,
        hash = %&reviewer_prompt_hash[..16],
        bytes = reviewer_prompt.len(),
        "Reviewer prompt loaded"
    );

    // ── Investigator prompt (Phase 7) ────────────────────────────────────────
    let investigator_prompt_path = std::env::var("INVESTIGATOR_PROMPT_PATH").unwrap_or_else(|_| {
        "./agents/investigator/system_prompt_v1.md".into()
    });
    let investigator_prompt = std::fs::read_to_string(&investigator_prompt_path)
        .unwrap_or_else(|e| {
            tracing::warn!(
                path = %investigator_prompt_path,
                error = %e,
                "investigator prompt not found — NeedsInvestigation cases will get empty instructions"
            );
            String::new()
        });
    let investigator_prompt_hash = hex::encode(Sha256::digest(investigator_prompt.as_bytes()));
    let investigator_agent_id =
        std::env::var("INVESTIGATOR_AGENT_ID").unwrap_or_else(|_| "investigator-v1".into());
    tracing::info!(
        path = %investigator_prompt_path,
        agent_id = %investigator_agent_id,
        hash = %&investigator_prompt_hash[..16],
        "Investigator prompt loaded"
    );

    // ── LLM client ────────────────────────────────────────────────────────────
    let llm_provider = std::env::var("ATTEST_LLM_PROVIDER").unwrap_or_else(|_| "local".into());
    let llm_model = std::env::var("ATTEST_LLM_MODEL")
        .unwrap_or_else(|_| "unsloth/Qwen3.6-35B-A3B-GGUF".into());
    let mcp_url = std::env::var("MCP_GATEWAY_URL")
        .unwrap_or_else(|_| "http://localhost:4242".into());

    let llm_client = match llm_from_env() {
        Ok(c) => {
            tracing::info!(
                provider = c.provider(),
                model = c.model_id(),
                mcp_url = %mcp_url,
                "LLM escalation path ready"
            );
            Some(c)
        }
        Err(e) => {
            tracing::warn!(
                error = %e,
                provider = %llm_provider,
                model = %llm_model,
                "LLM client unavailable — escalated alerts will fall back to EscalatedStub"
            );
            None
        }
    };

    // ── Agent definition ──────────────────────────────────────────────────────
    let agent_def = AgentDefinition {
        id: "triager-hybrid-v1".into(),
        role: AgentRole::Triager,
        execution: ExecutionPath::Hybrid {
            primary: Box::new(ExecutionPath::Classifier { artifact }),
            escalation: Box::new(ExecutionPath::Llm {
                provider: llm_provider,
                model_id: llm_model,
                system_prompt_hash: system_prompt_hash.clone(),
                max_iterations: std::env::var("LLM_MAX_ITERATIONS")
                    .unwrap_or_else(|_| "8".into())
                    .parse()
                    .unwrap_or(8),
            }),
        },
        version_hash: {
            let s = serde_json::to_string(&serde_json::json!({
                "id": "triager-hybrid-v1",
                "escalation_threshold": escalation_threshold,
                "novelty_threshold": novelty_threshold,
                "system_prompt_hash": system_prompt_hash,
            }))
            .unwrap();
            hex::encode(Sha256::digest(s.as_bytes()))
        },
    };

    // ── Signing key ───────────────────────────────────────────────────────────
    let signer = if let Ok(seed) = std::env::var("ATTEST_SIGNING_KEY") {
        Signer::from_hex_seed(&seed)?
    } else {
        tracing::warn!("ATTEST_SIGNING_KEY not set — generating ephemeral signing key");
        Signer::generate()
    };

    tracing::info!(verifying_key = %signer.verifying_key_hex(), "Ed25519 signing key ready");

    // ── Shadow checker (Phase 6) ──────────────────────────────────────────────
    let shadow_checker = ShadowChecker::from_env();
    tracing::info!(
        auto_close_threshold = std::env::var("AUTO_CLOSE_THRESHOLD").unwrap_or_else(|_| "0.90".into()),
        tenant_allows_automation = std::env::var("TENANT_ALLOWS_AUTOMATION").unwrap_or_else(|_| "true".into()),
        "Shadow checker ready"
    );

    // ── Build engine and start server ─────────────────────────────────────────
    let engine = TriageEngine::load(
        agent_def,
        Arc::new(signer),
        system_prompt,
        system_prompt_hash,
        reviewer_prompt,
        reviewer_prompt_hash,
        investigator_prompt,
        investigator_prompt_hash,
        investigator_agent_id,
        llm_client,
        shadow_checker,
    )?;

    let router = build_router(engine);
    let listener = TcpListener::bind(("0.0.0.0", port)).await?;
    tracing::info!(port, artifacts_dir, mcp_url, "Orchestrator listening");

    axum::serve(listener, router).await?;
    Ok(())
}
