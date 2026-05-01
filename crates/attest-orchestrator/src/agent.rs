//! Agent definition types — versioned, signed at build time, immutable at runtime.

use serde::{Deserialize, Serialize};

pub use attest_policy_engine::AgentRole;

/// References to ONNX model artifacts loaded by the orchestrator.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ClassifierArtifact {
    /// Absolute path to `model.onnx`.
    pub model_path: String,
    /// Absolute path to `shap_background.npy` (optional).
    pub shap_background_path: Option<String>,
    /// Absolute path to `novelty_mean.npy`.
    pub novelty_mean_path: String,
    /// Absolute path to `novelty_inv_cov.npy`.
    pub novelty_inv_cov_path: String,
    /// Absolute path to `novelty_threshold.txt`.
    pub novelty_threshold_path: String,
    /// SHA-256 of `model.onnx` at load time (for attestation).
    pub model_artifact_hash: String,
    /// SHA-256 of the feature column order string.
    pub feature_extractor_hash: String,
    /// Calibrated confidence below which the Triager escalates.
    pub escalation_threshold: f32,
    /// Novelty score above which the Triager escalates.
    pub novelty_threshold: f32,
}

/// Which execution path this agent uses.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case", tag = "kind")]
pub enum ExecutionPath {
    Classifier {
        artifact: ClassifierArtifact,
    },
    Llm {
        provider: String,
        model_id: String,
        system_prompt_hash: String,
        max_iterations: u8,
    },
    Hybrid {
        primary: Box<ExecutionPath>,
        escalation: Box<ExecutionPath>,
    },
}

/// A versioned agent definition. Signed at build time; immutable at runtime.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentDefinition {
    /// Unique identifier e.g. `triager-hybrid-v1`.
    pub id: String,
    pub role: AgentRole,
    pub execution: ExecutionPath,
    /// Version hash: SHA-256 of the serialised agent definition JSON.
    pub version_hash: String,
}
