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

/// Case disposition after the shadow-check gate (Phase 6).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum CaseState {
    /// Closed automatically — shadow check approved, signed envelope emitted.
    AutoClosed,
    /// Routed to the human review queue.
    #[default]
    PendingHumanReview,
}

/// The shadow-check gate decision embedded in auto-close envelopes.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ShadowCheckDecision {
    pub allowed: bool,
    /// Human-readable explanation (always populated).
    pub reason: String,
    /// Ordered list of policy names that were evaluated.
    pub policies_evaluated: Vec<String>,
}

/// Evidence embedded in an `EvidenceBlock::AutoClose` envelope.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AutoCloseEvidence {
    /// The classifier or hybrid evidence that produced the verdict.
    pub classifier_evidence: ClassifierEvidence,
    /// The shadow-check gate result.
    pub shadow_check: ShadowCheckDecision,
    /// The threshold that was configured at auto-close time.
    pub auto_close_threshold: f32,
    /// Principal extracted from the alert.
    pub principal: String,
    /// Action class extracted from the alert.
    pub action_class: String,
}

/// Which execution path produced this envelope.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ExecutionPathKind {
    Classifier,
    Llm,
    Hybrid,
    /// Analyst human override (Phase 8).
    HumanOverride,
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
    /// Number of times the verdict was rejected by guardrails and re-prompted.
    #[serde(default)]
    pub validation_retries: u8,
    /// Cross-agent review result for high-impact verdicts (None if not triggered).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cross_review: Option<CrossReviewBlock>,
}

/// Result of an independent reviewer LLM pass on a high-impact verdict.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CrossReviewBlock {
    /// SHA-256 of the reviewer system prompt.
    pub reviewer_prompt_hash: String,
    /// Whether the reviewer agreed with the primary agent's verdict.
    pub agrees: bool,
    /// Reviewer's explanation when disagreeing (or brief confirmation when agreeing).
    pub disagreement_reason: Option<String>,
    /// Reasoning steps recorded during the reviewer pass.
    pub reviewer_intermediate_beliefs: Vec<IntermediateBelief>,
}

// ── Hybrid evidence ───────────────────────────────────────────────────────────

/// Evidence for a hybrid execution that started with the classifier and escalated to LLM.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HybridEvidence {
    pub classifier_draft: ClassifierEvidence,
    pub llm_final: LlmEvidence,
    pub escalation_reason: EscalationReason,
}

/// Analyst override of a prior agent verdict (Phase 8 — immutable original preserved in log).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OverrideEvidence {
    /// Triager / investigator action that is being superseded.
    pub original_agent_action_id: Uuid,
    pub corrected_verdict: Verdict,
    pub reason: String,
    pub actor_id: String,
    pub actor_email: String,
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
    /// Phase 6: Triager auto-closed the case after shadow-check approval.
    AutoClose(AutoCloseEvidence),
    /// Phase 8: Human analyst override — new signed envelope; prior rows preserved.
    Override(OverrideEvidence),
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
        let value = serde_json::to_value(&body).expect("envelope serialisation is infallible");
        serde_json::to_vec(&sort_keys(value)).expect("envelope serialisation is infallible")
    }
}

/// Recursively rebuild every object with keys in sorted order. `HashMap` fields
/// (`input_features`, `shap_values`) iterate in a per-process random order, so without this
/// an envelope signed in one process would not verify after being read back in another.
fn sort_keys(value: serde_json::Value) -> serde_json::Value {
    use serde_json::{Map, Value};
    match value {
        Value::Object(map) => {
            let mut entries: Vec<(String, Value)> = map.into_iter().collect();
            entries.sort_by(|a, b| a.0.cmp(&b.0));
            Value::Object(
                entries
                    .into_iter()
                    .map(|(k, v)| (k, sort_keys(v)))
                    .collect::<Map<_, _>>(),
            )
        }
        Value::Array(items) => Value::Array(items.into_iter().map(sort_keys).collect()),
        other => other,
    }
}
