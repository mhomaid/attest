//! AWS KMS-backed signer, behind the `aws-kms` cargo feature.
//!
//! ## Algorithm choice
//!
//! AWS KMS supports Ed25519 signing through the `ECC_NIST_EDWARDS25519` key spec
//! with the `ED25519_SHA_512` signing algorithm and `MessageType::RAW`, which
//! produces a standard RFC 8032 *pure* Ed25519 signature over the supplied
//! message. That signature is byte-for-byte verifiable by the same
//! `ed25519-dalek` code [`LocalKey`] uses, so both backends record `alg =
//! "ed25519"` and share one keyring format. See `docs/20_Signer_KMS.md`.
//!
//! We send the 32-byte `sha256(canonical_bytes)` digest as the RAW *message*
//! (well under the KMS 4096-byte RAW limit), exactly as [`LocalKey`] signs it.
//!
//! ## Transport
//!
//! [`AwsKms`] is generic over [`KmsApi`], the two-call subset of the KMS API it
//! needs (`Sign`, `GetPublicKey`). A production build implements [`KmsApi`] over
//! `aws_sdk_kms::Client`; tests and local dev use [`RecordedKmsClient`], an
//! offline double that needs no AWS account. Keeping the transport behind a
//! trait is the generic extension point: organization-specific wiring (key
//! ARNs, regions, credentials) lives in the deploy overlay, never here.
//!
//! [`LocalKey`]: crate::signer::LocalKey

use crate::signer::{SignatureAlg, Signer};
use anyhow::{bail, Context, Result};

/// Mirrors the AWS KMS `MessageType` request field.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum KmsMessageType {
    /// Unhashed message; KMS applies the algorithm's hash. Required by `ED25519_SHA_512`.
    Raw,
    /// Pre-hashed message digest.
    Digest,
}

/// Mirrors the subset of AWS KMS `SigningAlgorithmSpec` this signer uses.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum KmsSigningAlgorithm {
    /// Pure Ed25519 (RFC 8032) over the RAW message. KMS key spec `ECC_NIST_EDWARDS25519`.
    Ed25519Sha512,
}

/// The subset of the AWS KMS API that [`AwsKms`] calls.
///
/// A production implementation wraps `aws_sdk_kms::Client`:
/// - [`sign`](KmsApi::sign) → `kms:Sign` with `MessageType=RAW`,
///   `SigningAlgorithm=ED25519_SHA_512`, returning the raw 64-byte signature.
/// - [`get_public_key`](KmsApi::get_public_key) → `kms:GetPublicKey`, returning
///   the DER-encoded `SubjectPublicKeyInfo`.
pub trait KmsApi: Send + Sync {
    /// Sign `message` with the KMS key `key_id`. Returns the raw signature bytes.
    fn sign(
        &self,
        key_id: &str,
        message: &[u8],
        message_type: KmsMessageType,
        algorithm: KmsSigningAlgorithm,
    ) -> Result<Vec<u8>>;

    /// Fetch the DER-encoded `SubjectPublicKeyInfo` for `key_id`.
    fn get_public_key(&self, key_id: &str) -> Result<Vec<u8>>;
}

/// Ed25519 `SubjectPublicKeyInfo` prefix (RFC 8410 §4): a 12-byte header
/// (`SEQUENCE { SEQUENCE { OID 1.3.101.112 }, BIT STRING }`) followed by the
/// 32-byte raw public key, for 44 bytes total.
const ED25519_SPKI_PREFIX: [u8; 12] = [
    0x30, 0x2a, 0x30, 0x05, 0x06, 0x03, 0x2b, 0x65, 0x70, 0x03, 0x21, 0x00,
];

/// Extract the raw 32-byte Ed25519 public key from a KMS `GetPublicKey` SPKI blob.
fn ed25519_from_spki(spki: &[u8]) -> Result<[u8; 32]> {
    if spki.len() != 44 || spki[..12] != ED25519_SPKI_PREFIX {
        bail!("GetPublicKey did not return an Ed25519 SubjectPublicKeyInfo");
    }
    let mut out = [0u8; 32];
    out.copy_from_slice(&spki[12..]);
    Ok(out)
}

/// A [`Signer`] that delegates the private-key operation to AWS KMS.
pub struct AwsKms<C: KmsApi> {
    client: C,
    key_id: String,
    public_key: [u8; 32],
}

impl<C: KmsApi> AwsKms<C> {
    /// Construct by downloading the public key once (one `GetPublicKey` call).
    ///
    /// `key_id` is any KMS key identifier (key id, alias or ARN). It is recorded
    /// verbatim in `envelope.key_id`, so in this public repo's tests it must be a
    /// neutral placeholder, never a real ARN or account id.
    pub fn new(client: C, key_id: impl Into<String>) -> Result<Self> {
        let key_id = key_id.into();
        let spki = client
            .get_public_key(&key_id)
            .context("KMS GetPublicKey failed")?;
        let public_key = ed25519_from_spki(&spki)?;
        Ok(Self {
            client,
            key_id,
            public_key,
        })
    }

    /// The verifying (public) key as hex — add this to a verifier's keyring.
    pub fn verifying_key_hex(&self) -> String {
        hex::encode(self.public_key)
    }
}

impl<C: KmsApi> Signer for AwsKms<C> {
    fn key_id(&self) -> String {
        self.key_id.clone()
    }

    fn alg(&self) -> SignatureAlg {
        SignatureAlg::Ed25519
    }

    fn public_key_hex(&self) -> String {
        hex::encode(self.public_key)
    }

    fn sign_digest(&self, digest: &[u8]) -> Result<Vec<u8>> {
        let sig = self
            .client
            .sign(
                &self.key_id,
                digest,
                KmsMessageType::Raw,
                KmsSigningAlgorithm::Ed25519Sha512,
            )
            .context("KMS Sign failed")?;
        if sig.len() != 64 {
            bail!(
                "KMS returned a {}-byte signature; expected 64 for Ed25519",
                sig.len()
            );
        }
        Ok(sig)
    }
}

/// A synthetic, offline stand-in for AWS KMS for tests and local development.
///
/// It holds a fixed Ed25519 key and answers `Sign` / `GetPublicKey` exactly as a
/// KMS `ECC_NIST_EDWARDS25519` key would — pure Ed25519 over the RAW message, and
/// an SPKI-encoded public key. No network and no AWS account are involved. This
/// is **not** for production use.
pub struct RecordedKmsClient {
    signing_key: ed25519_dalek::SigningKey,
    expected_key_id: String,
}

impl RecordedKmsClient {
    /// A deterministic client from a 32-byte hex seed (a "recorded" key).
    pub fn from_seed(key_id: impl Into<String>, seed_hex: &str) -> Result<Self> {
        let bytes = hex::decode(seed_hex).context("invalid KMS mock seed hex")?;
        let arr: [u8; 32] = bytes
            .try_into()
            .map_err(|_| anyhow::anyhow!("KMS mock seed must be 32 bytes"))?;
        Ok(Self {
            signing_key: ed25519_dalek::SigningKey::from_bytes(&arr),
            expected_key_id: key_id.into(),
        })
    }

    fn check_key_id(&self, key_id: &str) -> Result<()> {
        if key_id != self.expected_key_id {
            bail!("KMS: unknown key id {key_id}");
        }
        Ok(())
    }
}

impl KmsApi for RecordedKmsClient {
    fn sign(
        &self,
        key_id: &str,
        message: &[u8],
        message_type: KmsMessageType,
        algorithm: KmsSigningAlgorithm,
    ) -> Result<Vec<u8>> {
        use ed25519_dalek::Signer as _;
        self.check_key_id(key_id)?;
        // An Edwards-curve KMS key only accepts ED25519_SHA_512 with RAW.
        if message_type != KmsMessageType::Raw {
            bail!("ED25519_SHA_512 requires MessageType::RAW");
        }
        if algorithm != KmsSigningAlgorithm::Ed25519Sha512 {
            bail!("recorded KMS key only supports ED25519_SHA_512");
        }
        let sig = self.signing_key.sign(message);
        Ok(sig.to_bytes().to_vec())
    }

    fn get_public_key(&self, key_id: &str) -> Result<Vec<u8>> {
        self.check_key_id(key_id)?;
        let mut spki = ED25519_SPKI_PREFIX.to_vec();
        spki.extend_from_slice(self.signing_key.verifying_key().as_bytes());
        Ok(spki)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::envelope::*;
    use crate::signer::LocalKey;
    use crate::verify::{verify_log_with_keyring, Keyring};
    use chrono::Utc;
    use std::collections::HashMap;
    use uuid::Uuid;

    const MOCK_SEED: &str = "11112222333344445555666677778888aaaabbbbccccddddeeeeffff00001111";
    const MOCK_KEY_ID: &str = "kms-test-key-1";

    fn dummy() -> AttestationEnvelope {
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
            prev_hash: String::new(),
            signature: String::new(),
            alg: String::new(),
            key_id: String::new(),
            policy_hash: String::new(),
            signed_at: now,
        }
    }

    fn kms_signer() -> AwsKms<RecordedKmsClient> {
        let client = RecordedKmsClient::from_seed(MOCK_KEY_ID, MOCK_SEED).unwrap();
        AwsKms::new(client, MOCK_KEY_ID).unwrap()
    }

    #[test]
    fn aws_kms_signs_ed25519_and_verifies_in_keyring() {
        let signer = kms_signer();
        let mut env = dummy();
        signer.sign_envelope(&mut env).unwrap();

        assert_eq!(env.alg, "ed25519");
        assert_eq!(env.key_id, MOCK_KEY_ID);

        let mut keyring = Keyring::new();
        keyring.add(signer.verifying_key());
        let report = verify_log_with_keyring(&[env], &keyring);
        assert!(report.all_ok(), "{:?}", report.results);
    }

    #[test]
    fn aws_kms_signature_is_plain_ed25519() {
        // The KMS-produced signature verifies with the ordinary Ed25519 single-key
        // path, proving the KMS algorithm choice is byte-compatible with LocalKey.
        let signer = kms_signer();
        let mut env = dummy();
        signer.sign_envelope(&mut env).unwrap();
        assert!(LocalKey::verify(&env, &signer.verifying_key_hex()).unwrap());
    }

    #[test]
    fn aws_kms_tampered_body_fails() {
        let signer = kms_signer();
        let mut env = dummy();
        signer.sign_envelope(&mut env).unwrap();
        env.verdict = Verdict::TruePositive; // tamper after signing

        let mut keyring = Keyring::new();
        keyring.add(signer.verifying_key());
        let report = verify_log_with_keyring(&[env], &keyring);
        assert!(!report.all_ok());
        assert!(report.results[0].detail.contains("signature"));
    }

    #[test]
    fn rotation_from_local_to_kms_both_verify() {
        // Old envelope signed by a local key, new one by the KMS key; a keyring
        // holding both verifies the whole log across the rotation boundary.
        let local = LocalKey::generate();
        let kms = kms_signer();

        let mut old = dummy();
        old.prev_hash = GENESIS_HASH.to_string();
        local.sign(&mut old);
        let mut new = dummy();
        new.prev_hash = old.chain_hash();
        kms.sign_envelope(&mut new).unwrap();

        let mut keyring = Keyring::new();
        keyring.add(local.verifying_key());
        keyring.add(kms.verifying_key());

        let report = verify_log_with_keyring(&[old, new], &keyring);
        assert!(report.all_ok(), "{:?}", report.results);
    }

    #[test]
    fn get_public_key_rejects_non_ed25519_spki() {
        // Guard: a malformed SPKI blob must be refused, not silently truncated.
        assert!(ed25519_from_spki(&[0u8; 10]).is_err());
        let mut wrong = ED25519_SPKI_PREFIX.to_vec();
        wrong[0] = 0x31; // corrupt the SEQUENCE tag
        wrong.extend_from_slice(&[0u8; 32]);
        assert!(ed25519_from_spki(&wrong).is_err());
    }

    #[test]
    fn kms_client_rejects_unknown_key_id() {
        let client = RecordedKmsClient::from_seed(MOCK_KEY_ID, MOCK_SEED).unwrap();
        assert!(client.get_public_key("some-other-key").is_err());
        assert!(client
            .sign(
                "some-other-key",
                &[0u8; 32],
                KmsMessageType::Raw,
                KmsSigningAlgorithm::Ed25519Sha512
            )
            .is_err());
    }

    #[test]
    fn kms_client_rejects_wrong_message_type() {
        let client = RecordedKmsClient::from_seed(MOCK_KEY_ID, MOCK_SEED).unwrap();
        // ED25519_SHA_512 is RAW-only; DIGEST must be refused.
        assert!(client
            .sign(
                MOCK_KEY_ID,
                &[0u8; 32],
                KmsMessageType::Digest,
                KmsSigningAlgorithm::Ed25519Sha512
            )
            .is_err());
    }
}
