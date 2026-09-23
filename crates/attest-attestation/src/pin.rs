//! Pinned agent identity — refuse to treat an envelope as authentic if the
//! recorded model or prompt hash does not match the published agent definition.

use crate::envelope::{AttestationEnvelope, EvidenceBlock, LlmEvidence};
use anyhow::{bail, Result};

/// Hashes published in `agents/*.json` that a live envelope must match.
#[derive(Debug, Clone)]
pub struct AgentPin {
    pub agent_id: String,
    pub model_artifact_hash: Option<String>,
    pub system_prompt_hash: Option<String>,
}

/// Deny a signed envelope whose artifact hashes do not match `pin`.
///
/// A prompt or model swap is exactly this failure: the signature may be valid
/// for a different key/prompt, but it is not the pinned agent.
pub fn check_pin(envelope: &AttestationEnvelope, pin: &AgentPin) -> Result<()> {
    if envelope.agent_id != pin.agent_id {
        bail!(
            "agent_id mismatch: envelope={} pin={}",
            envelope.agent_id,
            pin.agent_id
        );
    }

    if let (Some(expected), Some(recorded)) = (
        pin.model_artifact_hash.as_deref(),
        classifier_model_hash(&envelope.evidence),
    ) {
        if recorded != expected {
            bail!("model_artifact_hash swap: recorded={recorded} pinned={expected}");
        }
    }

    if let (Some(expected), Some(recorded)) = (
        pin.system_prompt_hash.as_deref(),
        llm_prompt_hash(&envelope.evidence),
    ) {
        if recorded != expected {
            bail!("system_prompt_hash swap: recorded={recorded} pinned={expected}");
        }
    }

    Ok(())
}

fn classifier_model_hash(block: &EvidenceBlock) -> Option<&str> {
    match block {
        EvidenceBlock::Classifier(ev)
        | EvidenceBlock::EscalatedStub {
            classifier_draft: ev,
            ..
        } => Some(&ev.model_artifact_hash),
        EvidenceBlock::AutoClose(ac) => Some(&ac.classifier_evidence.model_artifact_hash),
        EvidenceBlock::Hybrid(h) => Some(&h.classifier_draft.model_artifact_hash),
        EvidenceBlock::Llm(_) | EvidenceBlock::Override(_) => None,
    }
}

fn llm_prompt_hash(block: &EvidenceBlock) -> Option<&str> {
    Some(match block {
        EvidenceBlock::Llm(LlmEvidence {
            system_prompt_hash, ..
        }) => system_prompt_hash,
        EvidenceBlock::Hybrid(h) => &h.llm_final.system_prompt_hash,
        _ => return None,
    })
}
