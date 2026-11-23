//! Graylog HTTP event notifications → OCSF Detection Finding (2004).
//!
//! The payload shape follows Graylog's public notification documentation.
//! Fixtures are synthetic. The source is off unless `ATTEST_SOURCE_GRAYLOG=on`.

use chrono::{DateTime, Utc};
use serde_json::Value;
use uuid::Uuid;

use attest_common::{Actor, Cloud, DetectionFinding, OcsfEvent, Severity, User};

use crate::error::CollectorError;
use crate::mapping::{apply, FieldMapping};
use crate::source::{NormalizeCtx, Source};

#[derive(Debug, Default, Clone, Copy)]
pub struct GraylogSource;

impl Source for GraylogSource {
    fn id(&self) -> &'static str {
        "graylog"
    }

    fn is_alert(&self) -> bool {
        true
    }

    fn decode(
        &self,
        bytes: &[u8],
        _content_type: Option<&str>,
    ) -> Result<Vec<Value>, CollectorError> {
        let raw: Value = serde_json::from_slice(bytes).map_err(|e| {
            CollectorError::Normalization(format!("Graylog payload is not JSON: {e}"))
        })?;
        if raw.get("event").and_then(|v| v.as_object()).is_none() {
            return Err(CollectorError::Normalization(
                "Graylog notification is missing an event object".into(),
            ));
        }
        Ok(vec![raw])
    }

    fn normalize(&self, raw: &Value, ctx: &NormalizeCtx) -> Result<OcsfEvent, CollectorError> {
        let (mapped, unmapped) = apply(raw, &FieldMapping::builtin());
        let event = mapped
            .get("event")
            .or_else(|| raw.get("event"))
            .ok_or_else(|| {
                CollectorError::Normalization("Graylog notification is missing event".into())
            })?;
        if !event.is_object() {
            return Err(CollectorError::Normalization(
                "Graylog event must be an object".into(),
            ));
        }
        let title = str_at(&mapped, "event_definition_title")
            .or_else(|| str_field(event, "message"))
            .unwrap_or_else(|| "graylog alert".into());
        let finding_uid = str_at(&mapped, "event_definition_id")
            .or_else(|| str_field(event, "id"))
            .unwrap_or_else(|| "unknown".into());
        let when = str_field(event, "timestamp")
            .and_then(|s| DateTime::parse_from_rfc3339(&s).ok())
            .map(|t| t.with_timezone(&Utc))
            .unwrap_or_else(Utc::now);
        let priority = event.get("priority").and_then(|v| v.as_i64());
        let severity = match priority {
            Some(n) if n <= 1 => Severity::Critical,
            Some(2) => Severity::High,
            Some(3) => Severity::Medium,
            _ => Severity::Low,
        };
        let user_name = event
            .get("fields")
            .and_then(|f| f.get("user"))
            .and_then(|v| v.as_str())
            .unwrap_or("unknown")
            .to_string();
        Ok(OcsfEvent::DetectionFinding(DetectionFinding {
            event_id: Uuid::new_v4(),
            time: when,
            tenant_id: ctx.tenant_id.clone(),
            severity,
            title,
            finding_uid,
            priority,
            actor: Actor {
                user: User {
                    name: user_name.clone(),
                    uid: user_name,
                    account_uid: None,
                },
            },
            cloud: Cloud {
                provider: "siem".into(),
                region: "unknown".into(),
                account_uid: ctx.tenant_id.clone(),
            },
            raw: Some(mapped),
            unmapped,
        }))
    }
}

fn str_at(root: &Value, key: &str) -> Option<String> {
    root.get(key).and_then(|v| v.as_str()).map(str::to_string)
}

fn str_field(event: &Value, key: &str) -> Option<String> {
    event.get(key).and_then(|v| v.as_str()).map(str::to_string)
}

/// A definition whose last run is older than twice its schedule is itself an alert.
#[derive(Debug, Clone)]
pub struct DefinitionHeartbeat {
    pub id: String,
    pub interval_secs: i64,
    pub last_run_unix: Option<i64>,
}

pub fn heartbeat_missed(def: &DefinitionHeartbeat, now_unix: i64) -> bool {
    let _ = def.id.as_str();
    match def.last_run_unix {
        None => true,
        Some(last) => now_unix.saturating_sub(last) > def.interval_secs.saturating_mul(2),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use attest_common::OcsfEvent;

    fn ctx() -> NormalizeCtx {
        NormalizeCtx {
            tenant_id: "tenant-a".into(),
            extension_prefix: "siem".into(),
        }
    }

    fn notification() -> Value {
        serde_json::json!({
            "event_definition_id": "def-1001",
            "event_definition_title": "Repeated login failures",
            "event": {
                "id": "evt-1001",
                "timestamp": "2024-01-15T10:30:00.000Z",
                "timerange_start": "2024-01-15T10:20:00.000Z",
                "timerange_end": "2024-01-15T10:30:00.000Z",
                "message": "Repeated login failures",
                "source": "siem.example.com",
                "priority": 2,
                "alert": true,
                "fields": { "user": "alice@example.com" }
            },
            "backlog": [{ "message": "failed login", "source": "app.example.com" }],
            "ticket": "1001"
        })
    }

    #[test]
    fn normal_alert_is_a_detection_finding() {
        let raw = notification();
        let bytes = serde_json::to_vec(&raw).unwrap();
        let records = GraylogSource
            .decode(&bytes, Some("application/json"))
            .unwrap();
        match GraylogSource.normalize(&records[0], &ctx()).unwrap() {
            OcsfEvent::DetectionFinding(f) => {
                assert_eq!(f.title, "Repeated login failures");
                assert_eq!(f.finding_uid, "def-1001");
                assert_eq!(f.priority, Some(2));
                assert_eq!(f.severity, Severity::High);
                assert!(f.unmapped.is_some());
            }
            other => panic!("expected detection finding, got {other:?}"),
        }
        assert!(GraylogSource.is_alert());
    }

    #[test]
    fn malformed_notification_errors() {
        let err = GraylogSource
            .decode(br#"{"event":"nope"}"#, None)
            .unwrap_err();
        assert!(err.to_string().contains("event object"));
    }

    #[test]
    fn heartbeat_flags_a_silent_definition() {
        let stale = DefinitionHeartbeat {
            id: "def-1001".into(),
            interval_secs: 60,
            last_run_unix: Some(1_000),
        };
        assert!(heartbeat_missed(&stale, 1_000 + 121));
        let fresh = DefinitionHeartbeat {
            last_run_unix: Some(1_000),
            ..stale
        };
        assert!(!heartbeat_missed(&fresh, 1_050));
        let never = DefinitionHeartbeat {
            id: "def-1002".into(),
            interval_secs: 60,
            last_run_unix: None,
        };
        assert!(heartbeat_missed(&never, 5_000));
    }
}
