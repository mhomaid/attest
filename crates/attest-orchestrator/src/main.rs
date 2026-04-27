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

use attest_attestation::Signer;
use attest_orchestrator::{
    agent::{AgentDefinition, ClassifierArtifact, ExecutionPath},
    build_router,
    triage::TriageEngine,
    AgentRole,
};
use sha2::{Digest, Sha256};
use std::sync::Arc;
use tokio::net::TcpListener;
use tracing_subscriber::{fmt, EnvFilter};

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    fmt().with_env_filter(EnvFilter::from_default_env()).init();

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

    // Load model SHA-256 from artifacts (produced by train.py)
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

    let agent_def = AgentDefinition {
        id: "triager-hybrid-v1".into(),
        role: AgentRole::Triager,
        execution: ExecutionPath::Hybrid {
            primary: Box::new(ExecutionPath::Classifier { artifact }),
            escalation: Box::new(ExecutionPath::Llm {
                provider: "anthropic".into(),
                model_id: "claude-haiku-3-5".into(),
                system_prompt_hash: "stub".into(),
                max_iterations: 5,
            }),
        },
        version_hash: {
            let s = serde_json::to_string(&serde_json::json!({
                "id": "triager-hybrid-v1",
                "escalation_threshold": escalation_threshold,
                "novelty_threshold": novelty_threshold,
            }))
            .unwrap();
            hex::encode(Sha256::digest(s.as_bytes()))
        },
    };

    let signer = if let Ok(seed) = std::env::var("ATTEST_SIGNING_KEY") {
        Signer::from_hex_seed(&seed)?
    } else {
        tracing::warn!("ATTEST_SIGNING_KEY not set — generating ephemeral signing key");
        Signer::generate()
    };

    tracing::info!(verifying_key = %signer.verifying_key_hex(), "Ed25519 signing key ready");

    let engine = TriageEngine::load(agent_def, Arc::new(signer))?;
    let router = build_router(engine);

    let listener = TcpListener::bind(("0.0.0.0", port)).await?;
    tracing::info!(port, artifacts_dir, "Orchestrator listening");

    axum::serve(listener, router).await?;
    Ok(())
}
