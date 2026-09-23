//! Offline verification of a hash-chained attestation log.

use crate::envelope::{AttestationEnvelope, ExecutionPathKind, GENESIS_HASH};
use crate::signer::Signer;
use serde::Serialize;
use std::collections::HashSet;
use uuid::Uuid;

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct LineResult {
    pub action_id: Uuid,
    pub ok: bool,
    pub detail: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct VerifyReport {
    pub results: Vec<LineResult>,
}

impl VerifyReport {
    pub fn all_ok(&self) -> bool {
        self.results.iter().all(|r| r.ok)
    }

    pub fn passed(&self) -> usize {
        self.results.iter().filter(|r| r.ok).count()
    }
}

/// Verify every envelope in `log` against `verifying_key_hex`.
///
/// Also fails when `agent_action_id` repeats, or — when any row has a
/// `prev_hash` — when the hash chain is broken by a delete or reorder.
pub fn verify_log(log: &[AttestationEnvelope], verifying_key_hex: &str) -> VerifyReport {
    let chained = log.iter().any(|e| !e.prev_hash.is_empty());
    let mut seen: HashSet<Uuid> = HashSet::new();
    let mut expected_prev = GENESIS_HASH.to_string();
    let mut results = Vec::with_capacity(log.len());

    for env in log {
        if !seen.insert(env.agent_action_id) {
            results.push(LineResult {
                action_id: env.agent_action_id,
                ok: false,
                detail: "duplicate agent_action_id (replay / confused deputy)".into(),
            });
            continue;
        }

        let sig = match Signer::verify(env, verifying_key_hex) {
            Ok(true) => None,
            Ok(false) => Some("signature mismatch".into()),
            Err(e) => Some(format!("invalid signature material: {e}")),
        };
        if let Some(detail) = sig {
            results.push(LineResult {
                action_id: env.agent_action_id,
                ok: false,
                detail,
            });
            if chained {
                expected_prev = env.chain_hash();
            }
            continue;
        }

        if chained && env.prev_hash != expected_prev {
            results.push(LineResult {
                action_id: env.agent_action_id,
                ok: false,
                detail: format!(
                    "chain break: prev_hash={} expected={}",
                    env.prev_hash, expected_prev
                ),
            });
            expected_prev = env.chain_hash();
            continue;
        }

        results.push(LineResult {
            action_id: env.agent_action_id,
            ok: true,
            detail: format!("pass ({})", path_label(&env.execution_path)),
        });
        if chained {
            expected_prev = env.chain_hash();
        }
    }

    VerifyReport { results }
}

fn path_label(path: &ExecutionPathKind) -> String {
    match path {
        ExecutionPathKind::Classifier => "classifier".into(),
        ExecutionPathKind::Llm => "llm".into(),
        ExecutionPathKind::Hybrid => "hybrid".into(),
        ExecutionPathKind::HumanOverride => "human_override".into(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::envelope::*;
    use crate::signer::Signer;
    use chrono::Utc;
    use std::collections::HashMap;

    fn signed(signer: &Signer, prev: &str) -> AttestationEnvelope {
        let now = Utc::now();
        let mut env = AttestationEnvelope {
            envelope_version: "1.1".into(),
            agent_action_id: Uuid::new_v4(),
            case_id: Uuid::new_v4(),
            tenant_id: "demo".into(),
            agent_id: "triager-hybrid-v1".into(),
            execution_path: ExecutionPathKind::Classifier,
            verdict: Verdict::Benign,
            evidence: EvidenceBlock::Classifier(ClassifierEvidence {
                model_artifact_hash: "aa".into(),
                feature_extractor_hash: "bb".into(),
                input_features: HashMap::new(),
                shap_values: HashMap::new(),
                raw_prediction: 0.1,
                calibrated_confidence: 0.1,
                novelty_score: 0.0,
            }),
            timing: TimingBlock {
                started_at: now,
                finished_at: now,
                total_ms: 1,
            },
            prev_hash: prev.to_string(),
            signature: String::new(),
            signed_at: now,
        };
        signer.sign(&mut env);
        env
    }

    #[test]
    fn verifies_chain_and_catches_delete() {
        let signer = Signer::generate();
        let a = signed(&signer, GENESIS_HASH);
        let b = signed(&signer, &a.chain_hash());
        let key = signer.verifying_key_hex();
        assert!(verify_log(&[a.clone(), b.clone()], &key).all_ok());
        let report = verify_log(&[b], &key);
        assert!(!report.all_ok());
        assert!(report.results[0].detail.contains("chain break"));
    }
}
