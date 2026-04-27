//! Attestation envelope types — three variants sharing a common signed wrapper.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use uuid::Uuid;

/// Final triage/investigation verdict label.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Verdict {
    TruePositive,
    FalsePositive,
    Benign,
    NeedsInvestigation,
    /// Classifier was not confident enough; will escalate (Phase 4a stub).
    EscalatedStub,
}

/// Which execution path produced this envelope.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ExecutionPathKind {
    Classifier,
    Llm,
    Hybrid,
}

/// Why the classifier escalated to the LLM path.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EscalationReason {
    LowCalibratedConfidence { score: f32, threshold: f32 },
    HighNoveltyScore { score: f32, threshold: f32 },
    BothLowConfidenceAndHighNovelty,
}

// ── Classifier evidence ───────────────────────────────────────────────────────

/// Evidence produced by the XGBoost/ONNX classifier path.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ClassifierEvidence {
    /// SHA-256 of the ONNX model file.
    pub model_artifact_hash: String,
    /// SHA-256 of the feature extractor code version.
    pub feature_extractor_hash: String,
    /// Named input features fed to the classifier.
    pub input_features: HashMap<String, f64>,
    /// SHAP value per feature (signed contribution to the prediction).
    pub shap_values: HashMap<String, f64>,
    /// Raw XGBoost prediction score (0–1).
    pub raw_prediction: f32,
    /// Calibrated probability after isotonic regression.
    pub calibrated_confidence: f32,
    /// Mahalanobis-distance-based OOD score (0–1; higher = more novel).
    pub novelty_score: f32,
}

// ── LLM evidence ─────────────────────────────────────────────────────────────

/// A single tool call made by the LLM agent.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolCallRecord {
    pub tool_id: String,
    /// SHA-256 of the serialized arguments (full args stored separately).
    pub args_hash: String,
    /// SHA-256 of the serialized result.
    pub result_hash: String,
    pub latency_ms: u64,
    pub policy_decision: String,
    pub timestamp: DateTime<Utc>,
}

/// An intermediate reasoning step recorded during the LLM loop.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IntermediateBelief {
    pub iteration: u8,
    pub content_summary: String,
    /// Self-reported confidence from the LLM (0–1), if available.
    pub self_reported_confidence: Option<f32>,
    pub timestamp: DateTime<Utc>,
}

/// Evidence produced by the LLM execution path.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LlmEvidence {
    pub model_provider: String,
    pub model_id: String,
    /// SHA-256 of the model version identifier.
    pub model_version_hash: String,
    /// SHA-256 of the system prompt text.
    pub system_prompt_hash: String,
    pub tool_calls: Vec<ToolCallRecord>,
    pub intermediate_beliefs: Vec<IntermediateBelief>,
    /// Structured `[evidence:ocsf_event_id]` citations from the final verdict.
    pub evidence_citations: Vec<String>,
    pub total_iterations: u8,
}

// ── Hybrid evidence ───────────────────────────────────────────────────────────

/// Evidence for a hybrid execution that started with the classifier and escalated to LLM.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HybridEvidence {
    pub classifier_draft: ClassifierEvidence,
    pub llm_final: LlmEvidence,
    pub escalation_reason: EscalationReason,
}

// ── Envelope variants ─────────────────────────────────────────────────────────

/// The execution-path-specific evidence block.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum EvidenceBlock {
    Classifier(ClassifierEvidence),
    Llm(LlmEvidence),
    Hybrid(HybridEvidence),
    /// Phase 4a stub: classifier escalated but LLM not yet wired.
    EscalatedStub {
        classifier_draft: ClassifierEvidence,
        escalation_reason: EscalationReason,
    },
}

/// Common signed wrapper shared by all three envelope variants.
///
/// The `signature` field covers a canonical JSON serialization of all fields
/// except `signature` itself (Ed25519 over `sha256(canonical_json)`).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AttestationEnvelope {
    pub envelope_version: String,
    /// Unique ID for this specific agent action/decision.
    pub agent_action_id: Uuid,
    pub case_id: Uuid,
    pub tenant_id: String,
    pub agent_id: String,
    pub execution_path: ExecutionPathKind,
    pub verdict: Verdict,
    pub evidence: EvidenceBlock,
    pub timing: TimingBlock,
    /// Hex-encoded Ed25519 signature over the canonical envelope body.
    pub signature: String,
    pub signed_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TimingBlock {
    pub started_at: DateTime<Utc>,
    pub finished_at: DateTime<Utc>,
    pub total_ms: u64,
}

impl AttestationEnvelope {
    /// Produce the canonical bytes that are signed/verified.
    /// All fields except `signature` are included in sorted-key JSON.
    pub fn canonical_bytes(&self) -> Vec<u8> {
        // Build a copy without the signature field for canonical serialisation.
        #[derive(Serialize)]
        struct Body<'a> {
            envelope_version: &'a str,
            agent_action_id: Uuid,
            case_id: Uuid,
            tenant_id: &'a str,
            agent_id: &'a str,
            execution_path: &'a ExecutionPathKind,
            verdict: &'a Verdict,
            evidence: &'a EvidenceBlock,
            timing: &'a TimingBlock,
            signed_at: &'a DateTime<Utc>,
        }
        let body = Body {
            envelope_version: &self.envelope_version,
            agent_action_id: self.agent_action_id,
            case_id: self.case_id,
            tenant_id: &self.tenant_id,
            agent_id: &self.agent_id,
            execution_path: &self.execution_path,
            verdict: &self.verdict,
            evidence: &self.evidence,
            timing: &self.timing,
            signed_at: &self.signed_at,
        };
        serde_json::to_vec(&body).expect("envelope serialisation is infallible")
    }
}
