//! Attestation crate — signed, replayable evidence envelopes for every agent action.
//!
//! Three envelope variants share a common signed outer wrapper:
//! - **Classifier** — SHAP feature attribution, XGBoost prediction, novelty score
//! - **Llm** — reasoning trace, tool calls, intermediate beliefs, evidence citations
//! - **Hybrid** — both classifier evidence + LLM evidence + escalation reason
//!
//! Ed25519 signing ensures tamper-evidence. The append-only `AttestationLog` writes
//! newline-delimited JSON to `ATTEST_LOG_PATH` (default `./attestations.ndjson`).

pub mod envelope;
pub mod log;
pub mod signer;

pub use envelope::{
    AttestationEnvelope, ClassifierEvidence, EscalationReason, EvidenceBlock, ExecutionPathKind,
    HybridEvidence, IntermediateBelief, LlmEvidence, TimingBlock, ToolCallRecord, Verdict,
};
pub use log::AttestationLog;
pub use signer::Signer;
