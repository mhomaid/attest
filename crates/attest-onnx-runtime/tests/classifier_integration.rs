//! Integration test — loads the actual ONNX artifacts and verifies predictions.
//! Only runs if ARTIFACTS_DIR env var points to built artifacts.

use attest_feature_extractor::FeatureExtractor;
use attest_onnx_runtime::{NoveltyDetector, OnnxClassifier};
use std::path::PathBuf;
use std::time::Instant;

fn artifacts_dir() -> Option<PathBuf> {
    // Try explicit env var first (for CI / Railway)
    if let Ok(p) = std::env::var("ARTIFACTS_DIR") {
        return Some(PathBuf::from(p));
    }
    // Auto-discover relative to CARGO_MANIFEST_DIR (workspace root / ml/triager/artifacts)
    let manifest = std::env::var("CARGO_MANIFEST_DIR").ok()?;
    let candidate = PathBuf::from(&manifest)
        .parent()? // crates/
        .parent()? // workspace root
        .join("ml/triager/artifacts");
    if candidate.join("model.onnx").exists() {
        Some(candidate)
    } else {
        None
    }
}

#[test]
fn classifier_predicts_tp_for_brute_force_pattern() {
    let Some(dir) = artifacts_dir() else {
        eprintln!("SKIP: ARTIFACTS_DIR not set");
        return;
    };

    let model_path = dir.join("model.onnx");
    let bg_path = dir.join("shap_background.npy");

    if !model_path.exists() {
        eprintln!("SKIP: model.onnx not found at {model_path:?}");
        return;
    }

    let classifier = OnnxClassifier::load(&model_path, Some(bg_path.to_str().unwrap()))
        .expect("failed to load classifier");

    // Known brute-force pattern — high confidence TP
    let features = FeatureExtractor::extract_from_json(&serde_json::json!({
        "severity_score": 0.80,
        "source_class_id": 3002.0,
        "entity_reputation_score": 0.70,
        "baseline_deviation": 3.5,
        "threat_intel_hit_count": 2.0,
        "hour_of_day": 3.0,
        "asset_criticality": 0.50,
        "prior_disposition_ratio": 0.85,
    }));

    let (score, shap) = classifier.predict(&features).expect("prediction failed");

    assert!(
        score > 0.80,
        "brute-force should have P(TP) > 0.80; got {score:.4}"
    );
    assert_eq!(shap.len(), 8, "SHAP must have 8 entries");
    println!("Brute-force P(TP)={score:.4}, SHAP={shap:?}");
}

#[test]
fn classifier_predicts_benign_for_routine_login() {
    let Some(dir) = artifacts_dir() else {
        eprintln!("SKIP: ARTIFACTS_DIR not set");
        return;
    };

    let model_path = dir.join("model.onnx");
    if !model_path.exists() {
        return;
    }

    let classifier =
        OnnxClassifier::load(&model_path, None::<&str>).expect("failed to load classifier");

    let features = FeatureExtractor::extract_from_json(&serde_json::json!({
        "severity_score": 0.10,
        "source_class_id": 3002.0,
        "entity_reputation_score": 0.0,
        "baseline_deviation": 0.1,
        "threat_intel_hit_count": 0.0,
        "hour_of_day": 9.0,
        "asset_criticality": 0.3,
        "prior_disposition_ratio": 0.05,
    }));

    let (score, _) = classifier.predict(&features).expect("prediction failed");

    assert!(
        score < 0.20,
        "benign login should have P(TP) < 0.20; got {score:.4}"
    );
    println!("Benign P(TP)={score:.4}");
}

/// Latency budget for one classifier-path prediction: inference plus the eight
/// perturbation passes behind the per-feature attributions. Public latency
/// claims rely on the release budget (CI runs this test with `--release`);
/// unoptimized builds are ~50x slower, so the debug budget is only a smoke check.
const PREDICT_P99_BUDGET_MS: f64 = if cfg!(debug_assertions) { 100.0 } else { 5.0 };

#[test]
fn classifier_predict_p99_under_budget() {
    let Some(dir) = artifacts_dir() else {
        eprintln!("SKIP: ARTIFACTS_DIR not set");
        return;
    };

    let model_path = dir.join("model.onnx");
    let bg_path = dir.join("shap_background.npy");
    if !model_path.exists() {
        eprintln!("SKIP: model.onnx not found at {model_path:?}");
        return;
    }

    let classifier = OnnxClassifier::load(&model_path, Some(bg_path.to_str().unwrap()))
        .expect("failed to load classifier");

    let features = FeatureExtractor::extract_from_json(&serde_json::json!({
        "severity_score": 0.80,
        "source_class_id": 3002.0,
        "entity_reputation_score": 0.70,
        "baseline_deviation": 3.5,
        "threat_intel_hit_count": 2.0,
        "hour_of_day": 3.0,
        "asset_criticality": 0.50,
        "prior_disposition_ratio": 0.85,
    }));

    for _ in 0..50 {
        classifier
            .predict(&features)
            .expect("warm-up prediction failed");
    }

    let n = 1_000;
    let mut latencies_ms: Vec<f64> = (0..n)
        .map(|_| {
            let t0 = Instant::now();
            classifier.predict(&features).expect("prediction failed");
            t0.elapsed().as_secs_f64() * 1_000.0
        })
        .collect();
    latencies_ms.sort_by(|a, b| a.total_cmp(b));

    let p50 = latencies_ms[n / 2];
    let p99 = latencies_ms[(n * 99) / 100];
    println!("classifier predict (n={n}): p50={p50:.2}ms p99={p99:.2}ms");

    assert!(
        p99 < PREDICT_P99_BUDGET_MS,
        "classifier predict P99 {p99:.2}ms exceeds {PREDICT_P99_BUDGET_MS}ms budget"
    );
}

#[test]
fn novelty_detector_scores_ood_higher_than_in_distribution() {
    let Some(dir) = artifacts_dir() else {
        eprintln!("SKIP: ARTIFACTS_DIR not set");
        return;
    };

    let mean_path = dir.join("novelty_mean.npy");
    let inv_cov_path = dir.join("novelty_inv_cov.npy");
    let threshold_path = dir.join("novelty_threshold.txt");

    if !mean_path.exists() {
        return;
    }

    let detector = NoveltyDetector::load(&mean_path, &inv_cov_path, &threshold_path)
        .expect("failed to load novelty detector");

    let in_dist = FeatureExtractor::extract_from_json(&serde_json::json!({
        "severity_score": 0.80,
        "source_class_id": 3002.0,
        "entity_reputation_score": 0.70,
        "baseline_deviation": 3.5,
        "threat_intel_hit_count": 2.0,
        "hour_of_day": 3.0,
        "asset_criticality": 0.50,
        "prior_disposition_ratio": 0.85,
    }));

    let ood = FeatureExtractor::extract_from_json(&serde_json::json!({
        "severity_score": 0.50,
        "source_class_id": 9999.0,   // completely unknown class
        "entity_reputation_score": 0.50,
        "baseline_deviation": 0.3,
        "threat_intel_hit_count": 0.0,
        "hour_of_day": 12.0,
        "asset_criticality": 0.50,
        "prior_disposition_ratio": 0.50,
    }));

    let in_score = detector.score(&in_dist);
    let ood_score = detector.score(&ood);

    println!("In-distribution novelty score: {in_score:.4}");
    println!("OOD novelty score: {ood_score:.4}");

    assert!(
        ood_score > in_score,
        "OOD score {ood_score:.4} should be > in-distribution score {in_score:.4}"
    );
}
