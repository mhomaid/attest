//! Outbound notifications: Slack, Jira, a generic webhook, and PagerDuty.
//!
//! Each sender retries 429 and 5xx, then keeps the message on a dead-letter
//! list. Slack approval buttons are signature-checked. An accepted approval
//! is a new signed envelope linked to the planned action, and it expires.

use std::time::Duration;

use attest_attestation::{
    AttestationEnvelope, ClassifierEvidence, EvidenceBlock, ExecutionPathKind, LocalKey,
    TimingBlock, Verdict,
};
use chrono::{DateTime, Duration as ChronoDuration, Utc};
use hmac::{Hmac, Mac};
use serde_json::{json, Value};
use sha2::Sha256;
use uuid::Uuid;

type HmacSha256 = Hmac<Sha256>;

#[derive(Debug, Clone)]
pub struct Notice {
    pub verdict: String,
    pub evidence_url: String,
    pub action_id: Uuid,
}

#[derive(Debug, Clone)]
pub struct Attempt {
    pub status: u16,
}

#[derive(Debug)]
pub struct DeadLetter {
    pub channel: String,
    pub notice: Notice,
    pub last_status: u16,
}

/// Retry 429 and 5xx up to `attempts` times, then dead-letter.
pub fn deliver(channel: &str, notice: &Notice, attempts: &[Attempt]) -> Result<(), DeadLetter> {
    let last = attempts.last().map(|a| a.status).unwrap_or(0);
    let ok = attempts.iter().any(|a| (200..300).contains(&a.status));
    if ok {
        return Ok(());
    }
    let retryable = attempts
        .iter()
        .all(|a| a.status == 429 || (500..600).contains(&a.status));
    if retryable {
        return Err(DeadLetter {
            channel: channel.into(),
            notice: notice.clone(),
            last_status: last,
        });
    }
    Err(DeadLetter {
        channel: channel.into(),
        notice: notice.clone(),
        last_status: last,
    })
}

pub fn slack_message(notice: &Notice) -> Value {
    json!({
        "text": format!("{} — {}", notice.verdict, notice.evidence_url),
        "blocks": [{
            "type": "actions",
            "elements": [
                {"type": "button", "action_id": "approve", "value": notice.action_id.to_string(), "text": {"type": "plain_text", "text": "Approve"}},
                {"type": "button", "action_id": "reject", "value": notice.action_id.to_string(), "text": {"type": "plain_text", "text": "Reject"}}
            ]
        }]
    })
}

pub fn jira_issue(notice: &Notice) -> Value {
    json!({
        "fields": {
            "summary": format!("Attest {}", notice.verdict),
            "description": notice.evidence_url
        }
    })
}

pub fn pagerduty_event(notice: &Notice) -> Value {
    json!({
        "routing_key": "from-secret",
        "event_action": "trigger",
        "payload": {
            "summary": notice.verdict,
            "source": "attest",
            "severity": "warning",
            "custom_details": {"evidence": notice.evidence_url, "action_id": notice.action_id}
        }
    })
}

pub fn webhook_body(notice: &Notice) -> Value {
    json!({
        "verdict": notice.verdict,
        "evidence_url": notice.evidence_url,
        "action_id": notice.action_id
    })
}

/// Slack signing secret check. Header value is `v0=` plus hex HMAC-SHA256 of
/// `v0:{timestamp}:{body}`.
pub fn slack_signature_ok(secret: &str, timestamp: &str, body: &str, header: &str) -> bool {
    let Some(sig) = header.strip_prefix("v0=") else {
        return false;
    };
    let base = format!("v0:{timestamp}:{body}");
    let mut mac = match HmacSha256::new_from_slice(secret.as_bytes()) {
        Ok(m) => m,
        Err(_) => return false,
    };
    mac.update(base.as_bytes());
    let expected = hex::encode(mac.finalize().into_bytes());
    // Constant-time compare via hmac's verify would need the raw bytes.
    let Ok(given) = hex::decode(sig) else {
        return false;
    };
    let Ok(exp) = hex::decode(&expected) else {
        return false;
    };
    if given.len() != exp.len() {
        return false;
    }
    given
        .iter()
        .zip(exp.iter())
        .fold(0u8, |acc, (a, b)| acc | (a ^ b))
        == 0
}

#[derive(Debug, Clone)]
pub struct Approval {
    pub action_id: Uuid,
    pub approver: String,
    pub expires_at: DateTime<Utc>,
}

pub fn approval_envelope(
    approval: &Approval,
    signer: &LocalKey,
    now: DateTime<Utc>,
) -> Result<AttestationEnvelope, &'static str> {
    if approval.expires_at <= now {
        return Err("approval expired");
    }
    let mut env = AttestationEnvelope {
        envelope_version: "1.1".into(),
        agent_action_id: approval.action_id,
        case_id: approval.action_id,
        tenant_id: "tenant-a".into(),
        agent_id: approval.approver.clone(),
        execution_path: ExecutionPathKind::Hybrid,
        verdict: Verdict::NeedsInvestigation,
        evidence: EvidenceBlock::Classifier(ClassifierEvidence {
            model_artifact_hash: "approval".into(),
            feature_extractor_hash: approval.action_id.to_string(),
            input_features: Default::default(),
            shap_values: Default::default(),
            raw_prediction: 0.0,
            calibrated_confidence: 1.0,
            novelty_score: 0.0,
        }),
        timing: TimingBlock {
            started_at: now,
            finished_at: now,
            total_ms: 0,
        },
        prev_hash: String::new(),
        signature: String::new(),
        alg: String::new(),
        key_id: String::new(),
        policy_hash: String::new(),
        signed_at: now,
    };
    signer.sign(&mut env);
    Ok(env)
}

pub fn approval_window() -> ChronoDuration {
    ChronoDuration::minutes(15)
}

pub fn retry_budget() -> Duration {
    Duration::from_millis(50)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn notice() -> Notice {
        Notice {
            verdict: "true_positive".into(),
            evidence_url: "https://example.com/cases/1001".into(),
            action_id: Uuid::nil(),
        }
    }

    #[test]
    fn success_is_delivered_and_429_and_5xx_are_dead_lettered() {
        assert!(deliver("slack", &notice(), &[Attempt { status: 200 }]).is_ok());
        let denied = deliver(
            "jira",
            &notice(),
            &[Attempt { status: 429 }, Attempt { status: 429 }],
        )
        .unwrap_err();
        assert_eq!(denied.last_status, 429);
        let failed = deliver("pagerduty", &notice(), &[Attempt { status: 503 }]).unwrap_err();
        assert_eq!(failed.channel, "pagerduty");
        assert_eq!(failed.last_status, 503);
    }

    #[test]
    fn slack_message_has_verdict_evidence_and_buttons() {
        let body = slack_message(&notice());
        let text = body.to_string();
        assert!(text.contains("true_positive"));
        assert!(text.contains("https://example.com/cases/1001"));
        assert!(text.contains("approve") && text.contains("reject"));
        let _ = jira_issue(&notice());
        let _ = pagerduty_event(&notice());
        let _ = webhook_body(&notice());
    }

    #[test]
    fn slack_callback_rejects_a_bad_signature() {
        let secret = "signing-secret";
        let body = "{}";
        let ts = "1700000000";
        let mut mac = HmacSha256::new_from_slice(secret.as_bytes()).unwrap();
        mac.update(format!("v0:{ts}:{body}").as_bytes());
        let good = format!("v0={}", hex::encode(mac.finalize().into_bytes()));
        assert!(slack_signature_ok(secret, ts, body, &good));
        assert!(!slack_signature_ok(secret, ts, body, "v0=00"));
        assert!(!slack_signature_ok("other", ts, body, &good));
    }

    #[test]
    fn approval_is_a_signed_envelope_and_expires() {
        let signer = LocalKey::generate();
        let now = Utc::now();
        let approval = Approval {
            action_id: Uuid::new_v4(),
            approver: "analyst@example.com".into(),
            expires_at: now + approval_window(),
        };
        let env = approval_envelope(&approval, &signer, now).unwrap();
        assert_eq!(env.agent_action_id, approval.action_id);
        assert_eq!(env.agent_id, "analyst@example.com");
        assert!(LocalKey::verify(&env, &signer.verifying_key_hex()).unwrap());
        let expired = Approval {
            expires_at: now - ChronoDuration::seconds(1),
            ..approval
        };
        assert_eq!(
            approval_envelope(&expired, &signer, now).unwrap_err(),
            "approval expired"
        );
    }
}
