//! Ed25519 signing and verification for attestation envelopes.

use crate::envelope::AttestationEnvelope;
use anyhow::{Context, Result};
use ed25519_dalek::{Signature, Signer as DalekSigner, SigningKey, VerifyingKey};
use rand::rngs::OsRng;
use sha2::{Digest, Sha256};

/// Holds an Ed25519 signing key and exposes sign/verify operations.
pub struct Signer {
    signing_key: SigningKey,
}

impl Signer {
    /// Generate a fresh keypair using the OS RNG.
    pub fn generate() -> Self {
        let signing_key = SigningKey::generate(&mut OsRng);
        Self { signing_key }
    }

    /// Load from a 32-byte hex-encoded seed (for deterministic test keys).
    pub fn from_hex_seed(hex_seed: &str) -> Result<Self> {
        let bytes = hex::decode(hex_seed).context("invalid hex seed")?;
        let arr: [u8; 32] = bytes.try_into().map_err(|_| anyhow::anyhow!("seed must be 32 bytes"))?;
        Ok(Self { signing_key: SigningKey::from_bytes(&arr) })
    }

    /// Export the verifying (public) key as hex.
    pub fn verifying_key_hex(&self) -> String {
        hex::encode(self.signing_key.verifying_key().as_bytes())
    }

    /// Sign an envelope: sets `envelope.signature` to the hex-encoded Ed25519 signature
    /// over `sha256(canonical_bytes)`.
    pub fn sign(&self, envelope: &mut AttestationEnvelope) {
        envelope.signature = String::new(); // clear before signing
        let digest = Sha256::digest(envelope.canonical_bytes());
        let sig: Signature = self.signing_key.sign(&digest);
        envelope.signature = hex::encode(sig.to_bytes());
    }

    /// Verify an envelope's signature given the hex-encoded verifying key.
    pub fn verify(envelope: &AttestationEnvelope, verifying_key_hex: &str) -> Result<bool> {
        let vk_bytes = hex::decode(verifying_key_hex).context("invalid verifying key hex")?;
        let vk_arr: [u8; 32] = vk_bytes.try_into().map_err(|_| anyhow::anyhow!("verifying key must be 32 bytes"))?;
        let vk = VerifyingKey::from_bytes(&vk_arr).context("invalid verifying key")?;

        let sig_bytes = hex::decode(&envelope.signature).context("invalid signature hex")?;
        let sig_arr: [u8; 64] = sig_bytes.try_into().map_err(|_| anyhow::anyhow!("signature must be 64 bytes"))?;
        let sig = Signature::from_bytes(&sig_arr);

        // Re-compute canonical bytes with empty signature field.
        let mut tmp = envelope.clone();
        tmp.signature = String::new();
        let digest = Sha256::digest(tmp.canonical_bytes());

        use ed25519_dalek::Verifier;
        Ok(vk.verify(&digest, &sig).is_ok())
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
            signature: String::new(),
            signed_at: now,
        }
    }

    #[test]
    fn sign_and_verify_roundtrip() {
        let signer = Signer::generate();
        let vk = signer.verifying_key_hex();
        let mut env = dummy_envelope();
        signer.sign(&mut env);
        assert!(!env.signature.is_empty());
        assert!(Signer::verify(&env, &vk).unwrap());
    }

    #[test]
    fn tampered_envelope_fails_verification() {
        let signer = Signer::generate();
        let vk = signer.verifying_key_hex();
        let mut env = dummy_envelope();
        signer.sign(&mut env);
        env.verdict = Verdict::TruePositive; // tamper
        assert!(!Signer::verify(&env, &vk).unwrap());
    }
}
