//! Signed chain-tip anchors. An anchor records how long the log was and the
//! hash of its last envelope. A later verify that sees a shorter log fails,
//! which is how tail truncation is detected. The object prefix is meant to be
//! an Object Lock bucket in compliance mode; this crate only writes the object.

use crate::envelope::{AttestationEnvelope, GENESIS_HASH};
use crate::signer::{LocalKey, Signer as SignerTrait};
use anyhow::{Context, Result};
use chrono::{DateTime, Utc};
use object_store::path::Path as ObjPath;
use object_store::{ObjectStore, ObjectStoreExt, PutPayload};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::sync::Arc;
use std::time::Duration;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ChainAnchor {
    pub seq: u64,
    pub tip_hash: String,
    pub signed_at: DateTime<Utc>,
    pub key_id: String,
    pub alg: String,
    pub signature: String,
}

/// How often the orchestrator writes an anchor, and whether to POST it.
/// The webhook is off unless `ATTEST_ANCHOR_WEBHOOK` is set.
#[derive(Debug, Clone)]
pub struct AnchorSchedule {
    pub every: Duration,
    pub webhook_url: Option<String>,
    pub prefix: String,
}

impl Default for AnchorSchedule {
    fn default() -> Self {
        Self {
            every: Duration::from_secs(300),
            webhook_url: None,
            prefix: "anchors".into(),
        }
    }
}

impl AnchorSchedule {
    pub fn from_env() -> Self {
        let every_secs = std::env::var("ATTEST_ANCHOR_EVERY_SECS")
            .ok()
            .and_then(|s| s.parse().ok())
            .unwrap_or(300);
        let webhook = std::env::var("ATTEST_ANCHOR_WEBHOOK")
            .ok()
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty());
        let prefix = std::env::var("ATTEST_ANCHOR_PREFIX").unwrap_or_else(|_| "anchors".into());
        Self {
            every: Duration::from_secs(every_secs),
            webhook_url: webhook,
            prefix,
        }
    }

    pub fn posts_webhook(&self) -> bool {
        self.webhook_url.is_some()
    }
}

pub fn sign_anchor(
    log: &[AttestationEnvelope],
    signer: &LocalKey,
    signed_at: DateTime<Utc>,
) -> Result<ChainAnchor> {
    let mut anchor = ChainAnchor {
        seq: log.len() as u64,
        tip_hash: tip_hash(log),
        signed_at,
        key_id: signer.key_id(),
        alg: signer.alg().as_str().to_string(),
        signature: String::new(),
    };
    let digest = Sha256::digest(canonical_anchor(&anchor));
    let sig = signer.sign_digest(&digest)?;
    anchor.signature = hex::encode(sig);
    Ok(anchor)
}

pub fn tip_hash(log: &[AttestationEnvelope]) -> String {
    log.last()
        .map(AttestationEnvelope::chain_hash)
        .unwrap_or_else(|| GENESIS_HASH.to_string())
}

/// Fail when the log is shorter than the anchor, which means rows after the
/// anchor were removed. The detail is `truncated after seq N`.
pub fn check_anchor(log: &[AttestationEnvelope], anchor: &ChainAnchor) -> Result<(), String> {
    if (log.len() as u64) < anchor.seq {
        return Err(format!("truncated after seq {}", log.len()));
    }
    if anchor.seq == 0 {
        return Ok(());
    }
    let at = (anchor.seq as usize) - 1;
    let got = log[at].chain_hash();
    if got != anchor.tip_hash {
        return Err(format!(
            "anchor tip mismatch at seq {}: expected {} got {got}",
            anchor.seq, anchor.tip_hash
        ));
    }
    Ok(())
}

pub async fn put_anchor(
    store: &Arc<dyn ObjectStore>,
    prefix: &str,
    anchor: &ChainAnchor,
) -> Result<String> {
    let key = format!(
        "{}/seq-{:020}.json",
        prefix.trim_end_matches('/'),
        anchor.seq
    );
    let path = ObjPath::from(key.clone());
    let bytes = serde_json::to_vec(anchor)?;
    store
        .put(&path, PutPayload::from(bytes))
        .await
        .context("put chain anchor")?;
    Ok(key)
}

fn canonical_anchor(anchor: &ChainAnchor) -> Vec<u8> {
    let body = serde_json::json!({
        "alg": anchor.alg,
        "key_id": anchor.key_id,
        "seq": anchor.seq,
        "signed_at": anchor.signed_at,
        "tip_hash": anchor.tip_hash,
    });
    serde_json::to_vec(&body).expect("anchor serialisation is infallible")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::envelope::*;
    use object_store::memory::InMemory;
    use object_store::{ObjectStore, ObjectStoreExt};
    use std::sync::Arc;

    fn chained(n: usize) -> Vec<AttestationEnvelope> {
        let signer = LocalKey::generate();
        let mut prev = GENESIS_HASH.to_string();
        let mut out = Vec::new();
        for _ in 0..n {
            let mut env = unsigned();
            env.prev_hash = prev.clone();
            signer.sign(&mut env);
            prev = env.chain_hash();
            out.push(env);
        }
        out
    }

    fn unsigned() -> AttestationEnvelope {
        let now = Utc::now();
        AttestationEnvelope {
            envelope_version: "1.1".into(),
            agent_action_id: uuid::Uuid::new_v4(),
            case_id: uuid::Uuid::new_v4(),
            tenant_id: "tenant-a".into(),
            agent_id: "triager".into(),
            execution_path: ExecutionPathKind::Classifier,
            verdict: Verdict::Benign,
            evidence: EvidenceBlock::Classifier(ClassifierEvidence {
                model_artifact_hash: "abc".into(),
                feature_extractor_hash: "def".into(),
                input_features: Default::default(),
                shap_values: Default::default(),
                raw_prediction: 0.1,
                calibrated_confidence: 0.1,
                novelty_score: 0.1,
            }),
            timing: TimingBlock {
                started_at: now,
                finished_at: now,
                total_ms: 1,
            },
            prev_hash: String::new(),
            signature: String::new(),
            alg: String::new(),
            key_id: String::new(),
            signed_at: now,
        }
    }

    #[test]
    fn delete_last_three_fails_truncated_after_seq() {
        let log = chained(6);
        let signer = LocalKey::generate();
        let anchor = sign_anchor(&log, &signer, Utc::now()).unwrap();
        assert_eq!(anchor.seq, 6);
        let truncated = log[..log.len() - 3].to_vec();
        let err = check_anchor(&truncated, &anchor).unwrap_err();
        assert_eq!(err, "truncated after seq 3");
    }

    #[test]
    fn full_log_matches_its_anchor() {
        let log = chained(4);
        let signer = LocalKey::generate();
        let anchor = sign_anchor(&log, &signer, Utc::now()).unwrap();
        assert!(check_anchor(&log, &anchor).is_ok());
    }

    #[test]
    fn webhook_is_off_by_default() {
        let schedule = AnchorSchedule::default();
        assert!(!schedule.posts_webhook());
        assert!(schedule.webhook_url.is_none());
    }

    #[tokio::test]
    async fn anchor_object_lands_under_the_prefix() {
        let log = chained(2);
        let signer = LocalKey::generate();
        let anchor = sign_anchor(&log, &signer, Utc::now()).unwrap();
        let store: Arc<dyn ObjectStore> = Arc::new(InMemory::new());
        let key = put_anchor(&store, "anchors", &anchor).await.unwrap();
        assert!(key.starts_with("anchors/seq-"));
        let got = store.get(&ObjPath::from(key)).await.unwrap();
        let bytes = got.bytes().await.unwrap();
        let read: ChainAnchor = serde_json::from_slice(&bytes).unwrap();
        assert_eq!(read.seq, anchor.seq);
        assert_eq!(read.key_id, anchor.key_id);
    }

    #[test]
    fn schedule_ignores_empty_webhook() {
        std::env::set_var("ATTEST_ANCHOR_WEBHOOK", "  ");
        std::env::set_var("ATTEST_ANCHOR_EVERY_SECS", "60");
        let schedule = AnchorSchedule::from_env();
        assert!(!schedule.posts_webhook());
        assert_eq!(schedule.every, Duration::from_secs(60));
        std::env::remove_var("ATTEST_ANCHOR_WEBHOOK");
        std::env::remove_var("ATTEST_ANCHOR_EVERY_SECS");
        let _ = Arc::new(0);
    }
}
