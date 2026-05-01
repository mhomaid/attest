//! Auto-close orchestration — evaluates shadow check and emits a signed
//! `EvidenceBlock::AutoClose` envelope when the gate approves.
//!
//! Called from `triage.rs` after every `Benign` or `FalsePositive` verdict.

use crate::shadow_check::ShadowChecker;
use attest_attestation::{
    AttestationEnvelope, AutoCloseEvidence, CaseState, ClassifierEvidence, EvidenceBlock,
    ExecutionPathKind, ShadowCheckDecision, Signer, TimingBlock, Verdict,
};
use chrono::Utc;
use serde_json::Value;
use std::sync::Arc;
use std::time::Instant;
use uuid::Uuid;

/// Result of attempting an auto-close on a triage verdict.
#[derive(Debug)]
pub struct AutoCloseResult {
    /// Whether the case was auto-closed.
    pub case_state: CaseState,
    /// The shadow-check decision (always present after Phase 6).
    pub shadow_check: ShadowCheckDecision,
    /// The signed auto-close envelope (only present when `case_state == AutoClosed`).
    pub envelope: Option<AttestationEnvelope>,
}

/// Configured auto-close threshold — read once at startup.
pub fn auto_close_threshold() -> f32 {
    std::env::var("AUTO_CLOSE_THRESHOLD")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(0.90)
}

/// Verdicts eligible for auto-close.
fn is_auto_closeable(verdict: &Verdict) -> bool {
    matches!(verdict, Verdict::Benign | Verdict::FalsePositive)
}

/// Extract the principal from an alert JSON.
/// Falls back through a priority list of common field names.
pub fn extract_principal(alert: &Value) -> String {
    for key in &[
        "principal",
        "actor_user_name",
        "actor_user",
        "user",
        "subject",
    ] {
        if let Some(s) = alert[key].as_str() {
            if !s.is_empty() {
                return s.to_string();
            }
        }
    }
    "unknown".into()
}

/// Extract the action class from an alert JSON.
pub fn extract_action_class(alert: &Value) -> String {
    for key in &["event_type", "action_class", "action", "activity_name"] {
        if let Some(s) = alert[key].as_str() {
            if !s.is_empty() {
                return s.to_ascii_lowercase();
            }
        }
    }
    "unknown".into()
}

/// Attempt to auto-close a case.
///
/// Returns `CaseState::PendingHumanReview` without an envelope when:
/// - The verdict is not `Benign` or `FalsePositive`, OR
/// - The calibrated confidence is below `AUTO_CLOSE_THRESHOLD`, OR
/// - The shadow check denies.
///
/// Returns `CaseState::AutoClosed` with a signed envelope when all gates pass.
#[allow(clippy::too_many_arguments)]
pub async fn try_auto_close(
    verdict: &Verdict,
    calibrated_confidence: f32,
    alert: &Value,
    classifier_evidence: ClassifierEvidence,
    shadow_checker: &ShadowChecker,
    signer: &Arc<Signer>,
    agent_id: &str,
    tenant_id: &str,
    case_id: Uuid,
    action_id: Uuid,
) -> AutoCloseResult {
    let threshold = auto_close_threshold();

    // Gate 1: verdict must be auto-closeable
    if !is_auto_closeable(verdict) {
        let sc = make_denied_decision(
            format!("verdict '{verdict:?}' is not auto-closeable"),
            vec!["verdict_allowlist".into()],
        );
        return AutoCloseResult {
            case_state: CaseState::PendingHumanReview,
            shadow_check: sc,
            envelope: None,
        };
    }

    // Gate 2: confidence floor
    if calibrated_confidence < threshold {
        let sc = make_denied_decision(
            format!("calibrated_confidence {calibrated_confidence:.3} < threshold {threshold:.3}"),
            vec!["confidence_floor".into()],
        );
        return AutoCloseResult {
            case_state: CaseState::PendingHumanReview,
            shadow_check: sc,
            envelope: None,
        };
    }

    // Gate 3: shadow check (do-not-touch, action class, policy engine)
    let principal = extract_principal(alert);
    let action_class = extract_action_class(alert);
    let sc = shadow_checker.check(&principal, &action_class, calibrated_confidence);

    if !sc.allowed {
        tracing::info!(
            case_id = %case_id,
            action_id = %action_id,
            reason = %sc.reason,
            "auto-close denied by shadow check"
        );
        return AutoCloseResult {
            case_state: CaseState::PendingHumanReview,
            shadow_check: sc,
            envelope: None,
        };
    }

    // All gates passed — build and sign the auto-close envelope
    let t0 = Instant::now();
    let started_at = Utc::now();

    let evidence = AutoCloseEvidence {
        classifier_evidence,
        shadow_check: sc.clone(),
        auto_close_threshold: threshold,
        principal: principal.clone(),
        action_class: action_class.clone(),
    };

    let finished_at = Utc::now();
    let total_ms = t0.elapsed().as_millis() as u64;

    let mut envelope = AttestationEnvelope {
        envelope_version: "1.1".into(),
        agent_action_id: action_id,
        case_id,
        tenant_id: tenant_id.to_string(),
        agent_id: agent_id.to_string(),
        execution_path: ExecutionPathKind::Classifier,
        verdict: verdict.clone(),
        evidence: EvidenceBlock::AutoClose(evidence),
        timing: TimingBlock {
            started_at,
            finished_at,
            total_ms,
        },
        signature: String::new(),
        signed_at: finished_at,
    };
    signer.sign(&mut envelope);

    tracing::info!(
        case_id = %case_id,
        action_id = %action_id,
        principal = %principal,
        action_class = %action_class,
        calibrated_confidence,
        "auto-close approved — signed envelope emitted"
    );

    AutoCloseResult {
        case_state: CaseState::AutoClosed,
        shadow_check: sc,
        envelope: Some(envelope),
    }
}

fn make_denied_decision(reason: String, policies: Vec<String>) -> ShadowCheckDecision {
    ShadowCheckDecision {
        allowed: false,
        reason,
        policies_evaluated: policies,
    }
}
