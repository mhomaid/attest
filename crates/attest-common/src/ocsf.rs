//! OCSF 1.3 types for the Attest streaming platform.
//!
//! Coverage for Phase 1:
//! - Class 3002 — Authentication (CloudTrail ConsoleLogin, Okta sign-in)
//! - Class 6003 — Cloud API (CloudTrail management events)

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// Top-level OCSF event envelope.  One variant per source class.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "class_uid", content = "data")]
pub enum OcsfEvent {
    /// Class 3002 — Authentication
    #[serde(rename = "3002")]
    Authentication(AuthenticationEvent),

    /// Class 6003 — Cloud API activity
    #[serde(rename = "6003")]
    CloudActivity(CloudActivityEvent),
}

impl OcsfEvent {
    pub fn event_id(&self) -> Uuid {
        match self {
            OcsfEvent::Authentication(e) => e.event_id,
            OcsfEvent::CloudActivity(e) => e.event_id,
        }
    }

    pub fn tenant_id(&self) -> &str {
        match self {
            OcsfEvent::Authentication(e) => &e.tenant_id,
            OcsfEvent::CloudActivity(e) => &e.tenant_id,
        }
    }

    pub fn time(&self) -> DateTime<Utc> {
        match self {
            OcsfEvent::Authentication(e) => e.time,
            OcsfEvent::CloudActivity(e) => e.time,
        }
    }
}

// ── Authentication event (OCSF class 3002) ────────────────────────────────

/// OCSF 1.3 Authentication event.
/// Covers CloudTrail `ConsoleLogin`, Okta sign-in, Entra ID interactive sign-in.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuthenticationEvent {
    pub event_id: Uuid,
    pub time: DateTime<Utc>,
    pub tenant_id: String,
    pub actor: Actor,
    pub cloud: Cloud,
    pub severity: Severity,
    pub status: AuthStatus,
    /// Raw source payload preserved for attribution.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub raw: Option<serde_json::Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum AuthStatus {
    Success,
    Failure,
    Unknown,
}

// ── Cloud API activity event (OCSF class 6003) ───────────────────────────

/// OCSF 1.3 Cloud API activity event.
/// Covers CloudTrail management events (CreateUser, PutBucketPolicy, …).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CloudActivityEvent {
    pub event_id: Uuid,
    pub time: DateTime<Utc>,
    pub tenant_id: String,
    pub actor: Actor,
    pub cloud: Cloud,
    pub api: ApiInfo,
    pub severity: Severity,
    pub resources: Vec<Resource>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub raw: Option<serde_json::Value>,
}

/// API call metadata.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ApiInfo {
    pub operation: String,
    pub service: String,
}

/// Cloud resource referenced by the event.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Resource {
    pub uid: String,
    #[serde(rename = "type")]
    pub resource_type: String,
}

// ── Shared sub-types ──────────────────────────────────────────────────────

/// The entity that triggered the event.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Actor {
    pub user: User,
}

/// A user or service principal.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct User {
    /// Human-readable name (email, username).
    pub name: String,
    /// Provider-specific identifier (ARN, UPN, Okta ID).
    pub uid: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub account_uid: Option<String>,
}

/// Cloud context (provider, account, region).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Cloud {
    pub provider: String,
    pub region: String,
    pub account_uid: String,
}

/// OCSF severity integer mapped to a readable label.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Severity {
    Unknown,
    Informational,
    Low,
    Medium,
    High,
    Critical,
    Fatal,
}

impl Severity {
    /// Map to OCSF severity_id integer.
    pub fn id(self) -> u8 {
        match self {
            Severity::Unknown => 0,
            Severity::Informational => 1,
            Severity::Low => 2,
            Severity::Medium => 3,
            Severity::High => 4,
            Severity::Critical => 5,
            Severity::Fatal => 6,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Utc;

    #[test]
    fn authentication_event_round_trips() {
        let event = AuthenticationEvent {
            event_id: Uuid::new_v4(),
            time: Utc::now(),
            tenant_id: "t1".into(),
            actor: Actor {
                user: User {
                    name: "alice@example.com".into(),
                    uid: "arn:aws:iam::123:user/alice".into(),
                    account_uid: Some("123456789".into()),
                },
            },
            cloud: Cloud {
                provider: "aws".into(),
                region: "us-west-2".into(),
                account_uid: "123456789".into(),
            },
            severity: Severity::Informational,
            status: AuthStatus::Success,
            raw: None,
        };

        let json = serde_json::to_string(&event).unwrap();
        let back: AuthenticationEvent = serde_json::from_str(&json).unwrap();
        assert_eq!(back.actor.user.name, "alice@example.com");
        assert_eq!(back.cloud.region, "us-west-2");
    }

    #[test]
    fn ocsf_event_enum_round_trips() {
        let ev = OcsfEvent::Authentication(AuthenticationEvent {
            event_id: Uuid::new_v4(),
            time: Utc::now(),
            tenant_id: "t1".into(),
            actor: Actor {
                user: User {
                    name: "bob@example.com".into(),
                    uid: "uid-1".into(),
                    account_uid: None,
                },
            },
            cloud: Cloud {
                provider: "aws".into(),
                region: "eu-west-1".into(),
                account_uid: "99".into(),
            },
            severity: Severity::Low,
            status: AuthStatus::Failure,
            raw: None,
        });

        let json = serde_json::to_string(&ev).unwrap();
        let back: OcsfEvent = serde_json::from_str(&json).unwrap();
        assert_eq!(back.tenant_id(), "t1");
    }
}
