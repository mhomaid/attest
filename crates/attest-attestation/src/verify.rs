//! Offline verification of a hash-chained attestation log.

use crate::envelope::{AttestationEnvelope, ExecutionPathKind, GENESIS_HASH};
use crate::signer::{verify_with, LocalKey, PublicKey, SignatureAlg};
use serde::Serialize;
use sha2::{Digest, Sha256};
use std::collections::{HashMap, HashSet};
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

        let sig = match LocalKey::verify(env, verifying_key_hex) {
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

/// A set of trusted public keys indexed by `key_id`, with an optional fallback
/// for legacy envelopes that carry no `key_id`.
///
/// This is how a verifier trusts more than one key at once — several current
/// keys (for rotation), plus a legacy key for logs written before key ids
/// existed.
#[derive(Default, Clone)]
pub struct Keyring {
    by_id: HashMap<String, PublicKey>,
    legacy: Option<PublicKey>,
}

impl Keyring {
    /// An empty keyring.
    pub fn new() -> Self {
        Self::default()
    }

    /// Trust `key` for envelopes whose `key_id` matches it.
    pub fn add(&mut self, key: PublicKey) -> &mut Self {
        self.by_id.insert(key.key_id.clone(), key);
        self
    }

    /// Set the key used for legacy envelopes that carry no `key_id`.
    pub fn with_legacy(&mut self, key: PublicKey) -> &mut Self {
        self.legacy = Some(key);
        self
    }

    /// Convenience: trust a single legacy Ed25519 key (hex) for unkeyed
    /// envelopes, mirroring the single-key [`verify_log`] path.
    pub fn with_legacy_ed25519_hex(&mut self, verifying_key_hex: impl Into<String>) -> &mut Self {
        self.with_legacy(PublicKey {
            key_id: String::new(),
            alg: SignatureAlg::Ed25519,
            public_key_hex: verifying_key_hex.into(),
        })
    }

    /// Build a keyring from an iterator of keys.
    pub fn from_keys(keys: impl IntoIterator<Item = PublicKey>) -> Self {
        let mut ring = Self::new();
        for key in keys {
            ring.add(key);
        }
        ring
    }

    /// Number of keyed entries (excludes the legacy fallback).
    pub fn len(&self) -> usize {
        self.by_id.len()
    }

    /// True when there are no keyed entries and no legacy key.
    pub fn is_empty(&self) -> bool {
        self.by_id.is_empty() && self.legacy.is_none()
    }

    /// Resolve the key that should verify `env`, or an error string describing why not.
    fn resolve(&self, env: &AttestationEnvelope) -> Result<&PublicKey, String> {
        if env.key_id.is_empty() {
            self.legacy.as_ref().ok_or_else(|| {
                "envelope has no key_id and the keyring has no legacy key".to_string()
            })
        } else {
            self.by_id
                .get(&env.key_id)
                .ok_or_else(|| format!("unknown key_id: {}", env.key_id))
        }
    }
}

/// Check one envelope's signature against a keyring, returning `Err(detail)` on
/// any failure (unknown key id, unsupported or mismatched algorithm, bad
/// signature). This is the keyring equivalent of [`LocalKey::verify`].
fn verify_sig_with_keyring(
    env: &AttestationEnvelope,
    keyring: &Keyring,
) -> std::result::Result<(), String> {
    let key = keyring.resolve(env)?;
    let alg =
        SignatureAlg::parse(&env.alg).ok_or_else(|| format!("unsupported alg: {}", env.alg))?;
    if alg != key.alg {
        return Err(format!(
            "alg mismatch: envelope says {} but key {} is {}",
            env.alg,
            key.key_id,
            key.alg.as_str()
        ));
    }
    // canonical_bytes already excludes signature/alg/key_id.
    let digest = Sha256::digest(env.canonical_bytes());
    match verify_with(alg, &key.public_key_hex, &digest, &env.signature) {
        Ok(true) => Ok(()),
        Ok(false) => Err("signature mismatch".into()),
        Err(e) => Err(format!("invalid signature material: {e}")),
    }
}

/// Verify every envelope in `log` against a [`Keyring`].
///
/// Each envelope is verified with the key named by its `key_id` (or the legacy
/// key when it has none). An envelope whose `key_id` is not in the keyring is
/// rejected with `unknown key_id: <id>`. Duplicate `agent_action_id` and chain
/// breaks are caught exactly as in [`verify_log`].
pub fn verify_log_with_keyring(log: &[AttestationEnvelope], keyring: &Keyring) -> VerifyReport {
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

        if let Err(detail) = verify_sig_with_keyring(env, keyring) {
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
    use crate::signer::{LocalKey, Signer};
    use chrono::Utc;
    use std::collections::HashMap;

    fn unsigned(prev: &str) -> AttestationEnvelope {
        let now = Utc::now();
        AttestationEnvelope {
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
            alg: String::new(),
            key_id: String::new(),
            policy_hash: String::new(),
            signed_at: now,
        }
    }

    fn signed(signer: &LocalKey, prev: &str) -> AttestationEnvelope {
        let mut env = unsigned(prev);
        signer.sign(&mut env);
        env
    }

    #[test]
    fn verifies_chain_and_catches_delete() {
        let signer = LocalKey::generate();
        let a = signed(&signer, GENESIS_HASH);
        let b = signed(&signer, &a.chain_hash());
        let key = signer.verifying_key_hex();
        assert!(verify_log(&[a.clone(), b.clone()], &key).all_ok());
        let report = verify_log(&[b], &key);
        assert!(!report.all_ok());
        assert!(report.results[0].detail.contains("chain break"));
    }

    #[test]
    fn keyring_verifies_known_keys_and_rejects_unknown_key_id() {
        // Two distinct keys; the keyring trusts only the first.
        let trusted = LocalKey::generate();
        let stranger = LocalKey::generate();
        let good = signed(&trusted, "");
        let bad = signed(&stranger, "");

        let mut keyring = Keyring::new();
        keyring.add(trusted.verifying_key());

        let report = verify_log_with_keyring(&[good], &keyring);
        assert!(report.all_ok(), "{:?}", report.results);

        // Negative: the stranger's key_id is not in the keyring → rejected.
        let report = verify_log_with_keyring(&[bad], &keyring);
        assert!(!report.all_ok());
        assert!(
            report.results[0].detail.contains("unknown key_id"),
            "{:?}",
            report.results
        );
    }

    #[test]
    fn keyring_rotation_old_and_new_both_verify() {
        // Rotate from `old` to `new`; a keyring holding both verifies envelopes
        // signed on either side of the rotation.
        let old = LocalKey::generate();
        let new = LocalKey::generate();
        assert_ne!(old.key_id(), new.key_id());

        let before = signed(&old, GENESIS_HASH);
        let after = {
            let mut e = unsigned(&before.chain_hash());
            new.sign(&mut e);
            e
        };

        let mut keyring = Keyring::new();
        keyring.add(old.verifying_key()).add(new.verifying_key());

        let report = verify_log_with_keyring(&[before, after], &keyring);
        assert!(report.all_ok(), "{:?}", report.results);
    }

    #[test]
    fn legacy_envelope_without_key_id_still_verifies() {
        // Simulate an old-format envelope: signed, then its key_id/alg stripped
        // as if it predated those fields.
        let signer = LocalKey::generate();
        let mut env = signed(&signer, "");
        env.key_id = String::new();
        env.alg = crate::envelope::default_alg(); // old logs deserialize to this

        // Single-key path (unchanged behaviour) still verifies it.
        assert!(verify_log(&[env.clone()], &signer.verifying_key_hex()).all_ok());

        // Keyring path verifies it via the legacy fallback key...
        let mut keyring = Keyring::new();
        keyring.with_legacy_ed25519_hex(signer.verifying_key_hex());
        assert!(verify_log_with_keyring(&[env.clone()], &keyring).all_ok());

        // ...but a keyring without a legacy key refuses an unkeyed envelope.
        let empty = Keyring::new();
        let report = verify_log_with_keyring(&[env], &empty);
        assert!(!report.all_ok());
        assert!(report.results[0].detail.contains("no key_id"));
    }

    #[test]
    fn keyring_rejects_tampered_body() {
        let signer = LocalKey::generate();
        let mut env = signed(&signer, "");
        env.verdict = Verdict::TruePositive; // tamper after signing

        let mut keyring = Keyring::new();
        keyring.add(signer.verifying_key());
        let report = verify_log_with_keyring(&[env], &keyring);
        assert!(!report.all_ok());
        assert!(report.results[0].detail.contains("signature mismatch"));
    }

    #[test]
    fn keyring_rejects_unsupported_alg() {
        let signer = LocalKey::generate();
        let mut env = signed(&signer, "");
        env.alg = "rsa-pss-sha256".into(); // an algorithm this build cannot verify

        let mut keyring = Keyring::new();
        keyring.add(signer.verifying_key());
        let report = verify_log_with_keyring(&[env], &keyring);
        assert!(!report.all_ok());
        assert!(report.results[0].detail.contains("unsupported alg"));
    }

    #[test]
    fn keyring_catches_chain_break() {
        let signer = LocalKey::generate();
        let a = signed(&signer, GENESIS_HASH);
        let b = signed(&signer, &a.chain_hash());
        let mut keyring = Keyring::new();
        keyring.add(signer.verifying_key());
        assert!(verify_log_with_keyring(&[a.clone(), b.clone()], &keyring).all_ok());
        let report = verify_log_with_keyring(&[b], &keyring);
        assert!(!report.all_ok());
        assert!(report.results[0].detail.contains("chain break"));
    }
}
