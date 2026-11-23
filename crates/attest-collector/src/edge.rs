//! Edge firewall events and audit logs.
//!
//! Firewall rows become OCSF HTTP Activity (4002). Audit rows become Cloud API
//! activity (6003). Shapes follow the public Logpush field names. Synthetic only.
//! Off unless `ATTEST_SOURCE_EDGE=on`.

use chrono::{DateTime, Utc};
use serde_json::Value;
use uuid::Uuid;

use attest_common::{
    Actor, ApiInfo, Cloud, CloudActivityEvent, HttpActivity, OcsfEvent, Resource, Severity, User,
};

use crate::error::CollectorError;
use crate::mapping::{apply, FieldMapping};
use crate::source::{NormalizeCtx, Source};

#[derive(Debug, Default, Clone, Copy)]
pub struct EdgeFirewallSource;

impl Source for EdgeFirewallSource {
    fn id(&self) -> &'static str {
        "edge-firewall"
    }

    fn decode(
        &self,
        bytes: &[u8],
        _content_type: Option<&str>,
    ) -> Result<Vec<Value>, CollectorError> {
        decode_lines_or_array(bytes, "edge firewall")
    }

    fn normalize(&self, raw: &Value, ctx: &NormalizeCtx) -> Result<OcsfEvent, CollectorError> {
        if raw.get("ClientIP").is_none() && raw.get("Action").is_none() {
            return Err(CollectorError::Normalization(
                "edge firewall event is missing Action and ClientIP".into(),
            ));
        }
        let (mapped, unmapped) = apply(raw, &FieldMapping::builtin());
        let action = str_of(&mapped, "Action").unwrap_or_else(|| "unknown".into());
        let path = str_of(&mapped, "ClientRequestURI")
            .or_else(|| str_of(&mapped, "ClientRequestPath"))
            .unwrap_or_else(|| "/".into());
        let status = mapped
            .get("EdgeResponseStatus")
            .map(|v| match v {
                Value::String(s) => s.clone(),
                Value::Number(n) => n.to_string(),
                _ => String::new(),
            })
            .filter(|s| !s.is_empty());
        let when = str_of(&mapped, "Datetime")
            .and_then(|s| DateTime::parse_from_rfc3339(&s).ok())
            .map(|t| t.with_timezone(&Utc))
            .unwrap_or_else(Utc::now);
        let disposition = match action.to_ascii_lowercase().as_str() {
            "block" | "drop" => "Blocked",
            "challenge" | "jschallenge" | "managed_challenge" => "Challenge",
            "allow" | "skip" => "Allowed",
            _ => "Unknown",
        };
        Ok(OcsfEvent::HttpActivity(HttpActivity {
            event_id: Uuid::new_v4(),
            time: when,
            tenant_id: ctx.tenant_id.clone(),
            severity: if disposition == "Blocked" {
                Severity::Medium
            } else {
                Severity::Informational
            },
            actor: Actor {
                user: User {
                    name: str_of(&mapped, "ClientIP").unwrap_or_else(|| "unknown".into()),
                    uid: str_of(&mapped, "RayID").unwrap_or_else(|| "unknown".into()),
                    account_uid: None,
                },
            },
            cloud: Cloud {
                provider: "edge".into(),
                region: "unknown".into(),
                account_uid: ctx.tenant_id.clone(),
            },
            http_path: path,
            http_status: status,
            src_ip: str_of(&mapped, "ClientIP"),
            disposition: Some(disposition.into()),
            raw: Some(mapped),
            unmapped,
        }))
    }
}

#[derive(Debug, Default, Clone, Copy)]
pub struct EdgeAuditSource;

impl Source for EdgeAuditSource {
    fn id(&self) -> &'static str {
        "edge-audit"
    }

    fn is_alert(&self) -> bool {
        true
    }

    fn decode(
        &self,
        bytes: &[u8],
        _content_type: Option<&str>,
    ) -> Result<Vec<Value>, CollectorError> {
        decode_lines_or_array(bytes, "edge audit")
    }

    fn normalize(&self, raw: &Value, ctx: &NormalizeCtx) -> Result<OcsfEvent, CollectorError> {
        let (mapped, unmapped) = apply(raw, &FieldMapping::builtin());
        let action = mapped
            .get("Action")
            .and_then(|a| a.get("type"))
            .and_then(|v| v.as_str())
            .unwrap_or("unknown");
        let resource_type = mapped
            .get("Resource")
            .and_then(|r| r.get("type"))
            .and_then(|v| v.as_str())
            .unwrap_or("unknown");
        let resource_id = mapped
            .get("Resource")
            .and_then(|r| r.get("id"))
            .and_then(|v| v.as_str())
            .unwrap_or("unknown");
        let actor_name = mapped
            .get("Actor")
            .and_then(|a| a.get("email"))
            .and_then(|v| v.as_str())
            .or_else(|| {
                mapped
                    .get("Actor")
                    .and_then(|a| a.get("id"))
                    .and_then(|v| v.as_str())
            })
            .unwrap_or("unknown");
        let when = str_of(&mapped, "When")
            .and_then(|s| DateTime::parse_from_rfc3339(&s).ok())
            .map(|t| t.with_timezone(&Utc))
            .unwrap_or_else(Utc::now);
        let security_change = resource_type.contains("firewall") || resource_type.contains("waf");
        Ok(OcsfEvent::CloudActivity(CloudActivityEvent {
            event_id: Uuid::new_v4(),
            time: when,
            tenant_id: ctx.tenant_id.clone(),
            actor: Actor {
                user: User {
                    name: actor_name.into(),
                    uid: actor_name.into(),
                    account_uid: None,
                },
            },
            cloud: Cloud {
                provider: "edge".into(),
                region: "unknown".into(),
                account_uid: ctx.tenant_id.clone(),
            },
            api: ApiInfo {
                operation: action.into(),
                service: "edge-audit".into(),
            },
            severity: if security_change {
                Severity::High
            } else {
                Severity::Informational
            },
            resources: vec![Resource {
                uid: resource_id.into(),
                resource_type: resource_type.into(),
            }],
            raw: Some(mapped),
            unmapped,
        }))
    }
}

fn decode_lines_or_array(bytes: &[u8], kind: &str) -> Result<Vec<Value>, CollectorError> {
    let text = std::str::from_utf8(bytes)
        .map_err(|_| CollectorError::Normalization(format!("{kind} payload is not UTF-8")))?;
    let trimmed = text.trim();
    if trimmed.starts_with('[') || trimmed.starts_with('{') {
        let value: Value = serde_json::from_str(trimmed).map_err(|e| {
            CollectorError::Normalization(format!("{kind} payload is not JSON: {e}"))
        })?;
        if let Some(arr) = value.as_array() {
            return Ok(arr.clone());
        }
        if value.is_object() {
            return Ok(vec![value]);
        }
    }
    let mut out = Vec::new();
    for line in text.lines().filter(|l| !l.trim().is_empty()) {
        let value: Value = serde_json::from_str(line)
            .map_err(|e| CollectorError::Normalization(format!("{kind} line is not JSON: {e}")))?;
        out.push(value);
    }
    if out.is_empty() {
        return Err(CollectorError::Normalization(format!(
            "{kind} payload was empty"
        )));
    }
    Ok(out)
}

fn str_of(value: &Value, key: &str) -> Option<String> {
    value.get(key).and_then(|v| v.as_str()).map(str::to_string)
}

#[cfg(test)]
mod tests {
    use super::*;
    use attest_common::OcsfEvent;

    fn ctx() -> NormalizeCtx {
        NormalizeCtx {
            tenant_id: "tenant-a".into(),
            extension_prefix: "edge".into(),
        }
    }

    #[test]
    fn firewall_block_is_http_activity() {
        let raw = serde_json::json!({
            "Action": "block",
            "ClientIP": "203.0.113.10",
            "ClientRequestHost": "shop.example.com",
            "ClientRequestMethod": "POST",
            "ClientRequestURI": "/checkout",
            "EdgeResponseStatus": 403,
            "RayID": "ray-1001",
            "Datetime": "2024-01-15T10:30:00Z",
            "ticket": "1001"
        });
        let bytes = serde_json::to_vec(&raw).unwrap();
        let records = EdgeFirewallSource.decode(&bytes, None).unwrap();
        match EdgeFirewallSource.normalize(&records[0], &ctx()).unwrap() {
            OcsfEvent::HttpActivity(e) => {
                assert_eq!(e.http_path, "/checkout");
                assert_eq!(e.http_status.as_deref(), Some("403"));
                assert_eq!(e.src_ip.as_deref(), Some("203.0.113.10"));
                assert_eq!(e.disposition.as_deref(), Some("Blocked"));
                assert!(e.unmapped.is_some());
            }
            other => panic!("expected http activity, got {other:?}"),
        }
    }

    #[test]
    fn firewall_without_action_or_ip_errors() {
        let err = EdgeFirewallSource
            .normalize(&serde_json::json!({"ticket": "1"}), &ctx())
            .unwrap_err();
        assert!(err.to_string().contains("missing Action"));
    }

    #[test]
    fn audit_firewall_change_is_high_severity_api_activity() {
        let raw = serde_json::json!({
            "When": "2024-01-15T10:30:00Z",
            "Action": { "type": "update", "result": true },
            "Actor": { "email": "alice@example.com", "id": "user-1001", "ip": "203.0.113.20" },
            "Resource": { "type": "firewall_rule", "id": "rule-1001" },
            "note": "ticket-1001"
        });
        match EdgeAuditSource.normalize(&raw, &ctx()).unwrap() {
            OcsfEvent::CloudActivity(e) => {
                assert_eq!(e.api.operation, "update");
                assert_eq!(e.severity, Severity::High);
                assert_eq!(e.resources[0].resource_type, "firewall_rule");
                let blob = serde_json::to_string(&e).unwrap();
                assert!(!blob.contains("alice@example.com"), "{blob}");
            }
            other => panic!("expected cloud activity, got {other:?}"),
        }
        assert!(EdgeAuditSource.is_alert());
    }
}
