//! Integration: sign an envelope, verify it, replay the classifier path, and
//! prove a tampered feature value is detected even when the signature is valid.

use std::collections::HashMap;
use std::path::{Path, PathBuf};

use attest_attestation::{
    AttestationEnvelope, ClassifierEvidence, EvidenceBlock, ExecutionPathKind, IntermediateBelief,
    LlmEvidence, Signer, TimingBlock, ToolCallRecord, Verdict,
};
use attest_cli::{read_log, replay, verify_log};
use attest_feature_extractor::AlertFeatures;
use attest_onnx_runtime::OnnxClassifier;
use chrono::{Duration, Utc};
use uuid::Uuid;

fn workspace_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..")
}

fn model_path() -> PathBuf {
    workspace_root().join("ml/triager/artifacts/model.onnx")
}

fn test_features() -> AlertFeatures {
    AlertFeatures {
        severity_score: 0.8,
        source_class_id: 3002.0,
        entity_reputation_score: 0.1,
        baseline_deviation: 1.5,
        threat_intel_hit_count: 0.0,
        hour_of_day: 3.0,
        asset_criticality: 0.5,
        prior_disposition_ratio: 0.2,
    }
}

fn signed_classifier_envelope(
    signer: &Signer,
    features: &AlertFeatures,
    raw: f32,
    shap: HashMap<String, f64>,
) -> AttestationEnvelope {
    let now = Utc::now();
    let verdict = if raw >= 0.5 {
        Verdict::TruePositive
    } else {
        Verdict::Benign
    };
    let mut env = AttestationEnvelope {
        envelope_version: "1.1".into(),
        agent_action_id: Uuid::new_v4(),
        case_id: Uuid::new_v4(),
        tenant_id: "test".into(),
        agent_id: "triager-hybrid-v1".into(),
        execution_path: ExecutionPathKind::Classifier,
        verdict,
        evidence: EvidenceBlock::Classifier(ClassifierEvidence {
            model_artifact_hash: attest_cli::file_sha256_hex(&model_path()).unwrap(),
            feature_extractor_hash: AlertFeatures::column_order_hash(),
            input_features: features.to_named_map(),
            shap_values: shap,
            raw_prediction: raw,
            calibrated_confidence: raw,
            novelty_score: 0.05,
        }),
        timing: TimingBlock {
            started_at: now,
            finished_at: now,
            total_ms: 4,
        },
        signature: String::new(),
        signed_at: now,
    };
    signer.sign(&mut env);
    env
}

fn write_log(dir: &Path, envelopes: &[AttestationEnvelope]) -> PathBuf {
    let path = dir.join("attestations.ndjson");
    let mut body = String::new();
    for env in envelopes {
        body.push_str(&serde_json::to_string(env).unwrap());
        body.push('\n');
    }
    std::fs::write(&path, body).unwrap();
    path
}

#[test]
fn verify_roundtrip_and_tampered_verdict() {
    let signer = Signer::generate();
    let vk = signer.verifying_key_hex();
    let features = test_features();
    let classifier = OnnxClassifier::load(model_path(), None).unwrap();
    let (raw, shap) = classifier.predict(&features).unwrap();
    let mut env = signed_classifier_envelope(&signer, &features, raw, shap);

    let dir = tempfile::tempdir().unwrap();
    let log_path = write_log(dir.path(), &[env.clone()]);
    let report = verify_log(&read_log(&log_path).unwrap(), &vk);
    assert!(report.all_ok(), "{:?}", report.results);
    assert_eq!(report.passed(), 1);

    env.verdict = if env.verdict == Verdict::Benign {
        Verdict::TruePositive
    } else {
        Verdict::Benign
    };
    let tampered = write_log(dir.path(), &[env]);
    let report = verify_log(&read_log(&tampered).unwrap(), &vk);
    assert!(!report.all_ok());
    assert!(report.results[0].detail.contains("signature"));
}

#[test]
fn replay_matches_recorded_prediction() {
    let signer = Signer::generate();
    let vk = signer.verifying_key_hex();
    let features = test_features();
    let classifier = OnnxClassifier::load(model_path(), None).unwrap();
    let (raw, shap) = classifier.predict(&features).unwrap();
    let env = signed_classifier_envelope(&signer, &features, raw, shap);

    let report = replay(&env, &vk, Some(&model_path())).unwrap();
    assert!(report.ok, "{}", report.lines.join("\n"));
    assert!(report
        .lines
        .iter()
        .any(|l| l.starts_with("raw_prediction: pass")));
    assert!(report.lines.iter().any(|l| l.starts_with("verdict: pass")));
}

#[test]
fn replay_detects_tampered_feature_on_a_valid_signature() {
    // Simulates a signer that recorded features that do not produce `raw_prediction`.
    let signer = Signer::generate();
    let vk = signer.verifying_key_hex();
    let features = test_features();
    let classifier = OnnxClassifier::load(model_path(), None).unwrap();
    let (raw, shap) = classifier.predict(&features).unwrap();
    let mut env = signed_classifier_envelope(&signer, &features, raw, shap);

    if let EvidenceBlock::Classifier(ev) = &mut env.evidence {
        ev.input_features.insert("severity_score".into(), 0.0);
    }
    signer.sign(&mut env);
    assert!(
        Signer::verify(&env, &vk).unwrap(),
        "precondition: envelope must still verify"
    );

    let report = replay(&env, &vk, Some(&model_path())).unwrap();
    assert!(!report.ok, "tampered features must fail replay");
    assert!(
        report
            .lines
            .iter()
            .any(|l| l.starts_with("raw_prediction: FAIL")),
        "{}",
        report.lines.join("\n")
    );
}

#[test]
fn replay_llm_is_integrity_only_and_rejects_reordered_tools() {
    let signer = Signer::generate();
    let vk = signer.verifying_key_hex();
    let now = Utc::now();
    let later = now + Duration::seconds(2);
    let mut env = AttestationEnvelope {
        envelope_version: "1.1".into(),
        agent_action_id: Uuid::new_v4(),
        case_id: Uuid::new_v4(),
        tenant_id: "test".into(),
        agent_id: "investigator-v1".into(),
        execution_path: ExecutionPathKind::Llm,
        verdict: Verdict::NeedsInvestigation,
        evidence: EvidenceBlock::Llm(LlmEvidence {
            model_provider: "test".into(),
            model_id: "test-llm".into(),
            model_version_hash: "aa".into(),
            system_prompt_hash: "bb".into(),
            tool_calls: vec![
                ToolCallRecord {
                    tool_id: "query_hot_tier".into(),
                    args_hash: "1".into(),
                    result_hash: "2".into(),
                    latency_ms: 10,
                    policy_decision: "allow".into(),
                    timestamp: later,
                },
                ToolCallRecord {
                    tool_id: "query_warm_tier".into(),
                    args_hash: "3".into(),
                    result_hash: "4".into(),
                    latency_ms: 12,
                    policy_decision: "allow".into(),
                    timestamp: now,
                },
            ],
            intermediate_beliefs: vec![IntermediateBelief {
                iteration: 1,
                content_summary: "looking".into(),
                self_reported_confidence: Some(0.4),
                timestamp: now,
            }],
            evidence_citations: vec![],
            total_iterations: 1,
            validation_retries: 0,
            cross_review: None,
        }),
        timing: TimingBlock {
            started_at: now,
            finished_at: later,
            total_ms: 2000,
        },
        signature: String::new(),
        signed_at: later,
    };
    signer.sign(&mut env);

    let report = replay(&env, &vk, None).unwrap();
    assert!(!report.ok);
    assert!(report.lines.iter().any(|l| l.contains("integrity only")));
    assert!(report
        .lines
        .iter()
        .any(|l| l.starts_with("tool_call_chain: FAIL")));

    if let EvidenceBlock::Llm(ev) = &mut env.evidence {
        ev.tool_calls.swap(0, 1);
    }
    signer.sign(&mut env);
    let report = replay(&env, &vk, None).unwrap();
    assert!(report.ok, "{}", report.lines.join("\n"));
    assert!(report.lines.iter().any(|l| l.contains("integrity only")));
}
