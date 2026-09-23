//! Regenerate `examples/verify/`: three hash-chained classifier envelopes signed
//! with a public demo key. Run from the workspace root:
//!
//! cargo run -q -p attest-cli --example make_sample

use std::fs;
use std::path::Path;

use anyhow::Result;
use attest_attestation::{
    AttestationEnvelope, ClassifierEvidence, EvidenceBlock, ExecutionPathKind, Signer, TimingBlock,
    Verdict, GENESIS_HASH,
};
use attest_feature_extractor::AlertFeatures;
use attest_onnx_runtime::OnnxClassifier;
use chrono::{TimeZone, Utc};
use uuid::Uuid;

/// Demo-only seed. Anyone can sign with it; never use it outside this sample.
const DEMO_SEED: &str = "a77e57a77e57a77e57a77e57a77e57a77e57a77e57a77e57a77e57a77e57a77e";

fn main() -> Result<()> {
    let out = Path::new("examples/verify");
    fs::create_dir_all(out)?;

    let model = Path::new("ml/triager/artifacts/model.onnx");
    let model_hash = attest_cli::file_sha256_hex(model)?;
    let classifier = OnnxClassifier::load(model, None)?;
    let signer = Signer::from_hex_seed(DEMO_SEED)?;

    let alerts = [
        (0.2, 0.9, 0.1, 14.0, 0.9),
        (0.3, 0.8, 0.3, 10.0, 0.8),
        (0.9, 0.05, 3.2, 3.0, 0.05),
    ];

    let mut prev = GENESIS_HASH.to_string();
    let mut body = String::new();
    for (i, (sev, rep, dev, hour, prior)) in alerts.into_iter().enumerate() {
        let features = AlertFeatures {
            severity_score: sev,
            source_class_id: 3002.0,
            entity_reputation_score: rep,
            baseline_deviation: dev,
            threat_intel_hit_count: 0.0,
            hour_of_day: hour,
            asset_criticality: 0.5,
            prior_disposition_ratio: prior,
        };
        let (raw, shap) = classifier.predict(&features)?;
        let at = Utc.with_ymd_and_hms(2026, 9, 1, 12, 0, i as u32).unwrap();
        let mut env = AttestationEnvelope {
            envelope_version: "1.1".into(),
            agent_action_id: Uuid::from_u128(0x5a3e_0000_0000_4000_8000_0000_0000_0001 + i as u128),
            case_id: Uuid::from_u128(0xca5e_0000_0000_4000_8000_0000_0000_0001 + i as u128),
            tenant_id: "demo".into(),
            agent_id: "triager-hybrid-v1".into(),
            execution_path: ExecutionPathKind::Classifier,
            verdict: if raw >= 0.5 {
                Verdict::TruePositive
            } else {
                Verdict::Benign
            },
            evidence: EvidenceBlock::Classifier(ClassifierEvidence {
                model_artifact_hash: model_hash.clone(),
                feature_extractor_hash: AlertFeatures::column_order_hash(),
                input_features: features.to_named_map(),
                shap_values: shap,
                raw_prediction: raw,
                calibrated_confidence: raw,
                novelty_score: 0.05,
            }),
            timing: TimingBlock {
                started_at: at,
                finished_at: at,
                total_ms: 3,
            },
            prev_hash: prev.clone(),
            signature: String::new(),
            signed_at: at,
        };
        signer.sign(&mut env);
        prev = env.chain_hash();
        body.push_str(&serde_json::to_string(&env)?);
        body.push('\n');
        eprintln!("row {}: {:?} raw={raw:.3}", i + 1, env.verdict);
    }

    fs::write(out.join("attestations.ndjson"), body)?;
    fs::write(
        out.join("verifying-key.txt"),
        signer.verifying_key_hex() + "\n",
    )?;
    fs::write(out.join("model-hash.txt"), model_hash + "\n")?;
    Ok(())
}
