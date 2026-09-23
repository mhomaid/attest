//! Attestation crate — signed, replayable evidence envelopes for every agent action.
//!
//! Three envelope variants share a common signed outer wrapper:
//! - **Classifier** — SHAP feature attribution, XGBoost prediction, novelty score
//! - **Llm** — reasoning trace, tool calls, intermediate beliefs, evidence citations
//! - **Hybrid** — both classifier evidence + LLM evidence + escalation reason
//!
//! Ed25519 signing ensures tamper-evidence. The append-only `AttestationLog` writes
//! newline-delimited JSON to `ATTEST_LOG_PATH` (default `./attestations.ndjson`) and,
//! when `ATTEST_LOG_S3_BUCKET` is set, a durable object-store replica.

pub mod envelope;
pub mod log;
pub mod pin;
pub mod signer;
pub mod trace_step;
pub mod verify;

pub use envelope::{
    AttestationEnvelope, AutoCloseEvidence, CaseState, ClassifierEvidence, CrossReviewBlock,
    EscalationReason, EvidenceBlock, ExecutionPathKind, HybridEvidence, IntermediateBelief,
    LlmEvidence, OverrideEvidence, ShadowCheckDecision, TimingBlock, ToolCallRecord, Verdict,
    GENESIS_HASH,
};
pub use log::AttestationLog;
pub use pin::{check_pin, AgentPin};
pub use signer::Signer;
pub use trace_step::{execution_path_snake, TraceStep, TRACE_STEPS_TOPIC};
pub use verify::{verify_log, LineResult, VerifyReport};
