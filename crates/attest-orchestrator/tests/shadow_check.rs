//! Integration tests for `auto_close::try_auto_close` (Phase 6).
//!
//! These are deterministic unit-integration tests — they do NOT need a live
//! orchestrator. They use `ShadowChecker::new` with hard-coded policies and a
//! freshly generated `Signer`, verifying that `try_auto_close` gates correctly.

use attest_attestation::{CaseState, ClassifierEvidence, EvidenceBlock, Signer, Verdict};
use attest_orchestrator::auto_close::try_auto_close;
use attest_orchestrator::shadow_check::ShadowChecker;
use serde_json::json;
use std::sync::Arc;
use uuid::Uuid;

fn test_signer() -> Arc<Signer> {
    Arc::new(Signer::generate())
}

fn test_checker(tenant_allows: bool) -> ShadowChecker {
    ShadowChecker::new(
        ["ceo@corp.com".to_string()].into(),
        0.90,
        tenant_allows,
        ["login".to_string(), "api_call".to_string()].into(),
    )
}

fn test_classifier_evidence(confidence: f32) -> ClassifierEvidence {
    ClassifierEvidence {
        model_artifact_hash: "aabbcc".into(),
        feature_extractor_hash: "ddeeff".into(),
        input_features: Default::default(),
        shap_values: Default::default(),
        raw_prediction: confidence,
        calibrated_confidence: confidence,
        novelty_score: 0.1,
    }
}

fn benign_alert() -> serde_json::Value {
    json!({
        "principal": "alice@corp.com",
        "event_type": "login",
        "severity_score": 0.1
    })
}

// ── Test 1 ────────────────────────────────────────────────────────────────────

/// High-confidence benign login: all gates pass → AutoClosed + signed envelope.
#[tokio::test]
async fn auto_close_approves_benign_login() {
    unsafe {
        std::env::set_var("AUTO_CLOSE_THRESHOLD", "0.90");
    }

    let result = try_auto_close(
        &Verdict::Benign,
        0.95,
        &benign_alert(),
        test_classifier_evidence(0.95),
        &test_checker(true),
        &test_signer(),
        "test-agent",
        "test-tenant",
        Uuid::new_v4(),
        Uuid::new_v4(),
    )
    .await;

    assert_eq!(
        result.case_state,
        CaseState::AutoClosed,
        "expected AutoClosed"
    );
    assert!(result.shadow_check.allowed, "shadow check must be allowed");
    assert!(result.envelope.is_some(), "signed envelope must be present");

    let env = result.envelope.unwrap();
    assert!(matches!(env.evidence, EvidenceBlock::AutoClose(_)));
}

// ── Test 2 ────────────────────────────────────────────────────────────────────

/// TruePositive verdict is never auto-closeable.
#[tokio::test]
async fn auto_close_rejects_true_positive() {
    let result = try_auto_close(
        &Verdict::TruePositive,
        0.99,
        &benign_alert(),
        test_classifier_evidence(0.99),
        &test_checker(true),
        &test_signer(),
        "test-agent",
        "test-tenant",
        Uuid::new_v4(),
        Uuid::new_v4(),
    )
    .await;

    assert_eq!(result.case_state, CaseState::PendingHumanReview);
    assert!(!result.shadow_check.allowed);
    assert!(result.envelope.is_none());
}

// ── Test 3 ────────────────────────────────────────────────────────────────────

/// Confidence below threshold routes to PendingHumanReview.
#[tokio::test]
async fn auto_close_rejects_low_confidence() {
    unsafe {
        std::env::set_var("AUTO_CLOSE_THRESHOLD", "0.90");
    }

    let result = try_auto_close(
        &Verdict::Benign,
        0.85, // below 0.90 floor
        &benign_alert(),
        test_classifier_evidence(0.85),
        &test_checker(true),
        &test_signer(),
        "test-agent",
        "test-tenant",
        Uuid::new_v4(),
        Uuid::new_v4(),
    )
    .await;

    assert_eq!(result.case_state, CaseState::PendingHumanReview);
    assert!(!result.shadow_check.allowed);
    assert!(
        result.shadow_check.reason.contains("threshold"),
        "{}",
        result.shadow_check.reason
    );
}

// ── Test 4 ────────────────────────────────────────────────────────────────────

/// Principal on do-not-touch list: denied even at max confidence.
#[tokio::test]
async fn auto_close_rejects_do_not_touch_principal() {
    unsafe {
        std::env::set_var("AUTO_CLOSE_THRESHOLD", "0.90");
    }

    let alert = json!({
        "principal": "ceo@corp.com",
        "event_type": "login",
    });

    let result = try_auto_close(
        &Verdict::Benign,
        0.99,
        &alert,
        test_classifier_evidence(0.99),
        &test_checker(true),
        &test_signer(),
        "test-agent",
        "test-tenant",
        Uuid::new_v4(),
        Uuid::new_v4(),
    )
    .await;

    assert_eq!(result.case_state, CaseState::PendingHumanReview);
    assert!(!result.shadow_check.allowed);
    assert!(
        result.shadow_check.reason.contains("do-not-touch"),
        "{}",
        result.shadow_check.reason
    );
}

// ── Test 5 ────────────────────────────────────────────────────────────────────

/// Tenant automation disabled: denied even if everything else passes.
#[tokio::test]
async fn auto_close_rejects_when_tenant_automation_off() {
    unsafe {
        std::env::set_var("AUTO_CLOSE_THRESHOLD", "0.90");
    }

    let result = try_auto_close(
        &Verdict::Benign,
        0.97,
        &benign_alert(),
        test_classifier_evidence(0.97),
        &test_checker(false), // tenant_allows_automation = false
        &test_signer(),
        "test-agent",
        "test-tenant",
        Uuid::new_v4(),
        Uuid::new_v4(),
    )
    .await;

    assert_eq!(result.case_state, CaseState::PendingHumanReview);
    assert!(!result.shadow_check.allowed);
}
