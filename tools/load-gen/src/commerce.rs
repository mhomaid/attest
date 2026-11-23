//! Synthetic commerce scenarios for the load generator.
//!
//! Patterns only: card-testing declines, OTP bursts, and an edge block on a
//! callback path. No real card numbers, customers, or merchants.

use attest_common::FlatEvent;
use chrono::Utc;
use uuid::Uuid;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CommerceLabel {
    CardTesting,
    OtpBurst,
    EdgeBlock,
}

pub struct CommerceEvent {
    pub event: FlatEvent,
    pub label: CommerceLabel,
}

fn base(class_uid: &str, tenant: &str) -> FlatEvent {
    FlatEvent {
        event_id: Uuid::new_v4().to_string(),
        class_uid: class_uid.into(),
        time: Utc::now().to_rfc3339(),
        tenant_id: tenant.into(),
        actor_user_name: "shopper-1001".into(),
        actor_user_uid: "user-1001".into(),
        cloud_region: "us-east-1".into(),
        cloud_account_uid: "123456789012".into(),
        severity: "medium".into(),
        auth_status: None,
        api_operation: None,
        api_service: None,
        raw: None,
        http_path: None,
        http_status: None,
        src_ip: Some("203.0.113.10".into()),
        asn: Some("AS64500".into()),
        disposition: None,
    }
}

pub fn card_testing(tenant: &str) -> CommerceEvent {
    let mut event = base("4002", tenant);
    event.http_path = Some("/checkout".into());
    event.http_status = Some("402".into());
    event.api_operation = Some("card_testing".into());
    event.severity = "high".into();
    CommerceEvent {
        event,
        label: CommerceLabel::CardTesting,
    }
}

pub fn otp_burst(tenant: &str) -> CommerceEvent {
    let mut event = base("3002", tenant);
    event.auth_status = Some("Failure".into());
    event.api_operation = Some("otp_request".into());
    CommerceEvent {
        event,
        label: CommerceLabel::OtpBurst,
    }
}

pub fn edge_block(tenant: &str, path: &str) -> CommerceEvent {
    let mut event = base("4002", tenant);
    event.http_path = Some(path.into());
    event.http_status = Some("403".into());
    event.disposition = Some("Blocked".into());
    CommerceEvent {
        event,
        label: CommerceLabel::EdgeBlock,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pack_covers_card_testing_otp_and_edge_block() {
        let card = card_testing("tenant-a");
        assert_eq!(card.label, CommerceLabel::CardTesting);
        assert_eq!(card.event.class_uid, "4002");
        assert_eq!(card.event.http_path.as_deref(), Some("/checkout"));
        assert!(card.event.raw.is_none());

        let otp = otp_burst("tenant-a");
        assert_eq!(otp.label, CommerceLabel::OtpBurst);
        assert_eq!(otp.event.class_uid, "3002");
        assert_eq!(otp.event.auth_status.as_deref(), Some("Failure"));

        let edge = edge_block("tenant-a", "/hooks/callback");
        assert_eq!(edge.label, CommerceLabel::EdgeBlock);
        assert_eq!(edge.event.disposition.as_deref(), Some("Blocked"));
        assert_eq!(edge.event.http_path.as_deref(), Some("/hooks/callback"));
    }
}
