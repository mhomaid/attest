//! Pluggable signing for attestation envelopes.
//!
//! [`Signer`] is the abstraction: anything that can stamp an envelope with an
//! algorithm, a key id and a signature. Two implementations ship:
//!
//! - [`LocalKey`] — an in-process Ed25519 key (the original behaviour).
//! - [`crate::kms::AwsKms`] — AWS KMS (behind the `aws-kms` cargo feature, so the
//!   default build pulls in no AWS dependencies).
//!
//! Every signature is computed over `sha256(envelope.canonical_bytes())`. The
//! `alg` and `key_id` envelope fields are *not* part of the canonical body; the
//! signature already binds the body to one private key, so those fields are
//! non-forgeable hints that let a verifier pick the right public key.

use crate::envelope::AttestationEnvelope;
use anyhow::{Context, Result};
use ed25519_dalek::{Signature, Signer as DalekSigner, SigningKey, Verifier, VerifyingKey};
use rand::rngs::OsRng;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::sync::OnceLock;

static POLICY_HASH: OnceLock<String> = OnceLock::new();

/// Record the loaded policy file hash. Later `sign` calls copy it onto the
/// envelope when the envelope does not already carry one.
pub fn set_policy_hash(hash: impl Into<String>) {
    let _ = POLICY_HASH.set(hash.into());
}

pub fn installed_policy_hash() -> Option<String> {
    POLICY_HASH.get().cloned()
}

/// Signature algorithm recorded in an envelope's `alg` field.
///
/// Only Ed25519 is currently produced. AWS KMS signs with the same scheme
/// (`ECC_NIST_EDWARDS25519` / `ED25519_SHA_512`, which yields RFC 8032 pure
/// Ed25519), so a single algorithm covers both signer backends. The enum exists
/// so a future backend can add an algorithm without changing the wire format.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum SignatureAlg {
    Ed25519,
}

impl SignatureAlg {
    /// The string stored in `envelope.alg`.
    pub fn as_str(&self) -> &'static str {
        match self {
            SignatureAlg::Ed25519 => "ed25519",
        }
    }

    /// Parse an `alg` string. Returns `None` for an algorithm this build cannot verify.
    pub fn parse(s: &str) -> Option<Self> {
        match s {
            "ed25519" => Some(SignatureAlg::Ed25519),
            _ => None,
        }
    }
}

/// A public key as a keyring stores it: an id, an algorithm and the hex-encoded
/// public key material (for Ed25519, the 32-byte verifying key).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PublicKey {
    pub key_id: String,
    pub alg: SignatureAlg,
    pub public_key_hex: String,
}

/// The pluggable signer abstraction. Object-safe so an [`AttestationLog`] can
/// hold a `&dyn Signer`.
///
/// [`AttestationLog`]: crate::log::AttestationLog
pub trait Signer: Send + Sync {
    /// Identifier of this key, recorded in `envelope.key_id`.
    fn key_id(&self) -> String;

    /// Algorithm this signer uses, recorded in `envelope.alg`.
    fn alg(&self) -> SignatureAlg;

    /// Hex-encoded public key material a verifier needs for this signer.
    fn public_key_hex(&self) -> String;

    /// Produce the raw signature bytes over `digest` (the SHA-256 of the
    /// canonical envelope body). May perform a network call (KMS).
    fn sign_digest(&self, digest: &[u8]) -> Result<Vec<u8>>;

    /// Stamp `alg`, `key_id` and `signature` onto `envelope`.
    fn sign_envelope(&self, envelope: &mut AttestationEnvelope) -> Result<()> {
        envelope.alg = self.alg().as_str().to_string();
        envelope.key_id = self.key_id();
        envelope.signature = String::new();
        let digest = Sha256::digest(envelope.canonical_bytes());
        let sig = self.sign_digest(&digest)?;
        envelope.signature = hex::encode(sig);
        Ok(())
    }

    /// The keyring entry (id, algorithm, public key) that verifies this signer.
    fn verifying_key(&self) -> PublicKey {
        PublicKey {
            key_id: self.key_id(),
            alg: self.alg(),
            public_key_hex: self.public_key_hex(),
        }
    }
}

/// Verify a `signature_hex` over `digest` with a public key of a given algorithm.
///
/// Returns `Ok(false)` when the signature is well-formed but does not match, and
/// `Err` when the key or signature material is malformed. This is the single
/// place that maps an algorithm to its verification routine.
pub fn verify_with(
    alg: SignatureAlg,
    public_key_hex: &str,
    digest: &[u8],
    signature_hex: &str,
) -> Result<bool> {
    match alg {
        SignatureAlg::Ed25519 => verify_ed25519(public_key_hex, digest, signature_hex),
    }
}

fn verify_ed25519(verifying_key_hex: &str, digest: &[u8], signature_hex: &str) -> Result<bool> {
    let vk_bytes = hex::decode(verifying_key_hex).context("invalid verifying key hex")?;
    let vk_arr: [u8; 32] = vk_bytes
        .try_into()
        .map_err(|_| anyhow::anyhow!("verifying key must be 32 bytes"))?;
    let vk = VerifyingKey::from_bytes(&vk_arr).context("invalid verifying key")?;

    let sig_bytes = hex::decode(signature_hex).context("invalid signature hex")?;
    let sig_arr: [u8; 64] = sig_bytes
        .try_into()
        .map_err(|_| anyhow::anyhow!("signature must be 64 bytes"))?;
    let sig = Signature::from_bytes(&sig_arr);

    Ok(vk.verify(digest, &sig).is_ok())
}

/// An in-process Ed25519 signing key — the original signer behaviour.
pub struct LocalKey {
    signing_key: SigningKey,
}

impl LocalKey {
    /// Generate a fresh keypair using the OS RNG.
    pub fn generate() -> Self {
        let signing_key = SigningKey::generate(&mut OsRng);
        Self { signing_key }
    }

    /// Load from a 32-byte hex-encoded seed (for deterministic test keys).
    pub fn from_hex_seed(hex_seed: &str) -> Result<Self> {
        let bytes = hex::decode(hex_seed).context("invalid hex seed")?;
        let arr: [u8; 32] = bytes
            .try_into()
            .map_err(|_| anyhow::anyhow!("seed must be 32 bytes"))?;
        Ok(Self {
            signing_key: SigningKey::from_bytes(&arr),
        })
    }

    /// Export the verifying (public) key as hex.
    pub fn verifying_key_hex(&self) -> String {
        hex::encode(self.signing_key.verifying_key().as_bytes())
    }

    /// Stable key id derived from the public key: `ed25519-<first 16 hex of sha256(pubkey)>`.
    pub fn key_id(&self) -> String {
        let digest = Sha256::digest(self.signing_key.verifying_key().as_bytes());
        format!("ed25519-{}", &hex::encode(digest)[..16])
    }

    /// Sign an envelope: sets `alg`, `key_id`, and the hex-encoded Ed25519
    /// signature over `sha256(canonical_bytes)`. Infallible for a local key.
    pub fn sign(&self, envelope: &mut AttestationEnvelope) {
        if envelope.policy_hash.is_empty() {
            if let Some(hash) = installed_policy_hash() {
                envelope.policy_hash = hash;
            }
        }
        envelope.alg = SignatureAlg::Ed25519.as_str().to_string();
        envelope.key_id = self.key_id();
        envelope.signature = String::new(); // clear before signing
        let digest = Sha256::digest(envelope.canonical_bytes());
        let sig: Signature = self.signing_key.sign(&digest);
        envelope.signature = hex::encode(sig.to_bytes());
    }

    /// Verify an envelope's signature given the hex-encoded verifying key.
    ///
    /// Ed25519 only; this is the legacy single-key path used by [`verify_log`]
    /// and by `attest replay`. It ignores `key_id` so that both old (unkeyed)
    /// and new envelopes verify against one published key.
    ///
    /// [`verify_log`]: crate::verify::verify_log
    pub fn verify(envelope: &AttestationEnvelope, verifying_key_hex: &str) -> Result<bool> {
        // canonical_bytes already excludes signature/alg/key_id.
        let digest = Sha256::digest(envelope.canonical_bytes());
        verify_ed25519(verifying_key_hex, &digest, &envelope.signature)
    }
}

impl Signer for LocalKey {
    fn key_id(&self) -> String {
        LocalKey::key_id(self)
    }

    fn alg(&self) -> SignatureAlg {
        SignatureAlg::Ed25519
    }

    fn public_key_hex(&self) -> String {
        self.verifying_key_hex()
    }

    fn sign_digest(&self, digest: &[u8]) -> Result<Vec<u8>> {
        let sig: Signature = self.signing_key.sign(digest);
        Ok(sig.to_bytes().to_vec())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::envelope::*;
    use chrono::Utc;
    use std::collections::HashMap;
    use uuid::Uuid;

    fn dummy_envelope() -> AttestationEnvelope {
        let now = Utc::now();
        AttestationEnvelope {
            envelope_version: "1.1".into(),
            agent_action_id: Uuid::new_v4(),
            case_id: Uuid::new_v4(),
            tenant_id: "test-tenant".into(),
            agent_id: "triager-hybrid-v1".into(),
            execution_path: ExecutionPathKind::Classifier,
            verdict: Verdict::Benign,
            evidence: EvidenceBlock::Classifier(ClassifierEvidence {
                model_artifact_hash: "abc123".into(),
                feature_extractor_hash: "def456".into(),
                input_features: HashMap::new(),
                shap_values: HashMap::new(),
                raw_prediction: 0.1,
                calibrated_confidence: 0.08,
                novelty_score: 0.05,
            }),
            timing: TimingBlock {
                started_at: now,
                finished_at: now,
                total_ms: 12,
            },
            prev_hash: String::new(),
            signature: String::new(),
            alg: String::new(),
            key_id: String::new(),
            policy_hash: String::new(),
            signed_at: now,
        }
    }

    #[test]
    fn sign_copies_the_installed_policy_hash() {
        set_policy_hash("policy-hash-for-test");
        let signer = LocalKey::generate();
        let mut env = dummy_envelope();
        signer.sign(&mut env);
        assert_eq!(env.policy_hash, "policy-hash-for-test");
        assert!(LocalKey::verify(&env, &signer.verifying_key_hex()).unwrap());
    }

    #[test]
    fn sign_and_verify_roundtrip() {
        let signer = LocalKey::generate();
        let vk = signer.verifying_key_hex();
        let mut env = dummy_envelope();
        signer.sign(&mut env);
        assert!(!env.signature.is_empty());
        assert!(LocalKey::verify(&env, &vk).unwrap());
    }

    #[test]
    fn sign_records_alg_and_stable_key_id() {
        let signer = LocalKey::generate();
        let mut env = dummy_envelope();
        signer.sign(&mut env);
        assert_eq!(env.alg, "ed25519");
        assert!(env.key_id.starts_with("ed25519-"));
        // key_id is a pure function of the public key.
        assert_eq!(env.key_id, signer.key_id());
    }

    #[test]
    fn signature_survives_json_roundtrip_with_populated_maps() {
        let signer = LocalKey::generate();
        let vk = signer.verifying_key_hex();
        let mut env = dummy_envelope();
        if let EvidenceBlock::Classifier(ev) = &mut env.evidence {
            for i in 0..16 {
                ev.input_features
                    .insert(format!("feature_{i}"), i as f64 * 0.5);
                ev.shap_values
                    .insert(format!("feature_{i}"), -(i as f64) * 0.01);
            }
        }
        signer.sign(&mut env);

        for _ in 0..32 {
            let line = serde_json::to_string(&env).unwrap();
            let read_back: AttestationEnvelope = serde_json::from_str(&line).unwrap();
            assert!(LocalKey::verify(&read_back, &vk).unwrap());
        }
    }

    #[test]
    fn tampered_envelope_fails_verification() {
        let signer = LocalKey::generate();
        let vk = signer.verifying_key_hex();
        let mut env = dummy_envelope();
        signer.sign(&mut env);
        env.verdict = Verdict::TruePositive; // tamper
        assert!(!LocalKey::verify(&env, &vk).unwrap());
    }

    #[test]
    fn signer_trait_object_signs_like_inherent() {
        let signer = LocalKey::generate();
        let dynamic: &dyn Signer = &signer;
        let mut env = dummy_envelope();
        dynamic.sign_envelope(&mut env).unwrap();
        assert_eq!(env.alg, "ed25519");
        assert!(LocalKey::verify(&env, &signer.verifying_key_hex()).unwrap());
    }

    #[test]
    fn unsupported_alg_does_not_parse() {
        assert_eq!(SignatureAlg::parse("ed25519"), Some(SignatureAlg::Ed25519));
        assert_eq!(SignatureAlg::parse("rsa-pss-sha256"), None);
    }
}
