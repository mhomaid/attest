//! GuardDuty findings → OCSF Detection Finding (2004).
//!
//! The shape follows AWS's published sample findings. Fixtures are synthetic.
//! Off unless `ATTEST_SOURCE_GUARDDUTY=on`.

use chrono::{DateTime, Utc};
use serde_json::Value;
use uuid::Uuid;

use attest_common::{Actor, Cloud, DetectionFinding, OcsfEvent, Severity, User};

use crate::error::CollectorError;
use crate::mapping::{apply, FieldMapping};
use crate::source::{NormalizeCtx, Source};

#[derive(Debug, Default, Clone, Copy)]
pub struct GuardDutySource;

impl Source for GuardDutySource {
    fn id(&self) -> &'static str {
        "guardduty"
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
            CollectorError::Normalization(format!("GuardDuty payload is not JSON: {e}"))
        })?;
        if let Some(detail) = raw.get("detail") {
            if detail.get("type").is_some() {
                return Ok(vec![detail.clone()]);
            }
        }
        if raw.get("type").is_some() && raw.get("accountId").is_some() {
            return Ok(vec![raw]);
        }
        Err(CollectorError::Normalization(
            "GuardDuty payload has no finding type".into(),
        ))
    }

    fn normalize(&self, raw: &Value, ctx: &NormalizeCtx) -> Result<OcsfEvent, CollectorError> {
        let (mapped, unmapped) = apply(raw, &FieldMapping::builtin());
        let kind = mapped
            .get("type")
            .and_then(|v| v.as_str())
            .ok_or_else(|| CollectorError::Normalization("GuardDuty finding has no type".into()))?;
        let title = mapped
            .get("title")
            .and_then(|v| v.as_str())
            .unwrap_or(kind)
            .to_string();
        let finding_uid = mapped
            .get("id")
            .and_then(|v| v.as_str())
            .unwrap_or("unknown")
            .to_string();
        let severity_num = mapped
            .get("severity")
            .and_then(|v| v.as_f64())
            .unwrap_or(1.0);
        let severity = if severity_num >= 7.0 {
            Severity::High
        } else if severity_num >= 4.0 {
            Severity::Medium
        } else {
            Severity::Low
        };
        let when = mapped
            .get("updatedAt")
            .and_then(|v| v.as_str())
            .and_then(|s| DateTime::parse_from_rfc3339(s).ok())
            .map(|t| t.with_timezone(&Utc))
            .unwrap_or_else(Utc::now);
        let region = mapped
            .get("region")
            .and_then(|v| v.as_str())
            .unwrap_or("unknown");
        let account = mapped
            .get("accountId")
            .and_then(|v| v.as_str())
            .unwrap_or("unknown");
        let resource = mapped
            .pointer("/resource/instanceDetails/instanceId")
            .or_else(|| mapped.pointer("/resource/resourceType"))
            .and_then(|v| v.as_str())
            .unwrap_or("unknown");
        Ok(OcsfEvent::DetectionFinding(DetectionFinding {
            event_id: Uuid::new_v4(),
            time: when,
            tenant_id: ctx.tenant_id.clone(),
            severity,
            title,
            finding_uid,
            priority: Some(severity_num as i64),
            actor: Actor {
                user: User {
                    name: resource.into(),
                    uid: resource.into(),
                    account_uid: Some(account.into()),
                },
            },
            cloud: Cloud {
                provider: "aws".into(),
                region: region.into(),
                account_uid: account.into(),
            },
            raw: Some(mapped),
            unmapped,
        }))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ctx() -> NormalizeCtx {
        NormalizeCtx {
            tenant_id: "tenant-a".into(),
            extension_prefix: "aws".into(),
        }
    }

    fn finding() -> Value {
        serde_json::json!({
            "schemaVersion": "2.0",
            "accountId": "123456789012",
            "region": "us-east-1",
            "id": "finding-1001",
            "type": "Recon:EC2/PortProbeUnprotectedPort",
            "severity": 8,
            "title": "Unprotected port probe",
            "updatedAt": "2024-01-15T10:30:00.000Z",
            "resource": {
                "resourceType": "Instance",
                "instanceDetails": { "instanceId": "i-1001" }
            },
            "ticket": "1001"
        })
    }

    #[test]
    fn sample_finding_is_a_high_detection() {
        let raw = finding();
        let bytes = serde_json::to_vec(&raw).unwrap();
        let records = GuardDutySource.decode(&bytes, None).unwrap();
        match GuardDutySource.normalize(&records[0], &ctx()).unwrap() {
            OcsfEvent::DetectionFinding(f) => {
                assert_eq!(f.finding_uid, "finding-1001");
                assert_eq!(f.title, "Unprotected port probe");
                assert_eq!(f.severity, Severity::High);
                assert_eq!(f.cloud.region, "us-east-1");
                assert_eq!(f.cloud.account_uid, "123456789012");
                assert!(f.unmapped.is_some());
            }
            other => panic!("expected detection finding, got {other:?}"),
        }
        assert!(GuardDutySource.is_alert());
    }

    #[test]
    fn eventbridge_envelope_uses_detail() {
        let wrapped = serde_json::json!({"detail": finding()});
        let bytes = serde_json::to_vec(&wrapped).unwrap();
        let records = GuardDutySource.decode(&bytes, None).unwrap();
        assert_eq!(records[0]["id"], "finding-1001");
    }

    #[test]
    fn malformed_finding_errors() {
        let err = GuardDutySource
            .decode(br#"{"ticket":"1001"}"#, None)
            .unwrap_err();
        assert!(err.to_string().contains("no finding type"));
    }
}
