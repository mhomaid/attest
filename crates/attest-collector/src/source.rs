//! Pluggable ingest sources.
//!
//! A source decodes one transport payload into raw records and maps each record
//! to OCSF. CloudTrail is the first implementation. Topic names are
//! `{prefix}.raw.{id}` and, for sources that are already alerts, `{prefix}.alerts`.

use serde_json::Value;

use attest_common::OcsfEvent;

use crate::error::CollectorError;
use crate::normalizer::normalize_record;

/// Tenant and the extension prefix used for fields this source does not map.
#[derive(Debug, Clone)]
pub struct NormalizeCtx {
    pub tenant_id: String,
    /// Segment under `unmapped`, for example `aws`. Never a deployer field name.
    pub extension_prefix: String,
}

pub trait Source: Send + Sync {
    /// Stable id, used in topic names (`{prefix}.raw.<id>`) and metrics.
    fn id(&self) -> &'static str;

    /// Decode one transport payload into raw records.
    fn decode(
        &self,
        bytes: &[u8],
        content_type: Option<&str>,
    ) -> Result<Vec<Value>, CollectorError>;

    /// Map one raw record to OCSF. Unknown fields go under `unmapped.<prefix>`.
    fn normalize(&self, raw: &Value, ctx: &NormalizeCtx) -> Result<OcsfEvent, CollectorError>;

    /// Sources that are already alerts also publish to `{prefix}.alerts`.
    fn is_alert(&self) -> bool {
        false
    }
}

/// CloudTrail management and console events.
#[derive(Debug, Default, Clone, Copy)]
pub struct CloudTrailSource;

impl Source for CloudTrailSource {
    fn id(&self) -> &'static str {
        "cloudtrail"
    }

    fn decode(
        &self,
        bytes: &[u8],
        _content_type: Option<&str>,
    ) -> Result<Vec<Value>, CollectorError> {
        let raw: Value = serde_json::from_slice(bytes).map_err(|e| {
            CollectorError::Normalization(format!("CloudTrail payload is not JSON: {e}"))
        })?;
        if let Some(records) = raw.get("Records") {
            let arr = records.as_array().ok_or_else(|| {
                CollectorError::Normalization("CloudTrail Records is not an array".into())
            })?;
            return Ok(arr.clone());
        }
        if raw.is_object() {
            Ok(vec![raw])
        } else {
            Err(CollectorError::Normalization(
                "CloudTrail payload is neither an object nor a Records envelope".into(),
            ))
        }
    }

    fn normalize(&self, raw: &Value, ctx: &NormalizeCtx) -> Result<OcsfEvent, CollectorError> {
        normalize_record(raw, ctx)
    }
}

/// Topic names derived from a configurable prefix. The default prefix is `attest`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TopicNames {
    prefix: String,
    /// Also write CloudTrail to the historical `cloudtrail` topic so the existing
    /// RisingWave table keeps its connector.
    pub mirror_legacy_cloudtrail: bool,
}

impl TopicNames {
    pub fn new(prefix: impl Into<String>, mirror_legacy_cloudtrail: bool) -> Self {
        Self {
            prefix: sanitize_prefix(&prefix.into()),
            mirror_legacy_cloudtrail,
        }
    }

    pub fn from_env() -> Self {
        let prefix = std::env::var("ATTEST_TOPIC_PREFIX").unwrap_or_else(|_| "attest".into());
        let mirror = match std::env::var("ATTEST_MIRROR_LEGACY_CLOUDTRAIL") {
            Ok(v) => !matches!(v.as_str(), "0" | "off" | "false"),
            Err(_) => true,
        };
        Self::new(prefix, mirror)
    }

    pub fn raw(&self, source_id: &str) -> String {
        format!("{}.raw.{source_id}", self.prefix)
    }

    pub fn alerts(&self) -> String {
        format!("{}.alerts", self.prefix)
    }
}

/// Topics one produced event is written to, in order.
pub fn topics_for(layout: &TopicNames, source_id: &str, is_alert: bool) -> Vec<String> {
    let mut topics = vec![layout.raw(source_id)];
    if is_alert {
        topics.push(layout.alerts());
    }
    if source_id == "cloudtrail" && layout.mirror_legacy_cloudtrail {
        topics.push("cloudtrail".into());
    }
    topics
}

fn sanitize_prefix(raw: &str) -> String {
    let trimmed = raw.trim();
    let ok = !trimmed.is_empty()
        && trimmed.len() <= 64
        && trimmed
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, '-' | '_' | '.'));
    if ok {
        trimmed.to_string()
    } else {
        "attest".into()
    }
}

/// Keys CloudTrail maps onto OCSF. Everything else lands under `unmapped`.
pub fn cloudtrail_known_fields() -> &'static [&'static str] {
    &[
        "eventId",
        "eventTime",
        "eventName",
        "eventSource",
        "awsRegion",
        "sourceIPAddress",
        "userIdentity",
        "recipientAccountId",
        "resources",
        "errorCode",
    ]
}

pub fn unmapped_extras(raw: &Value, prefix: &str) -> Option<Value> {
    let obj = raw.as_object()?;
    let known = cloudtrail_known_fields();
    let mut extras = serde_json::Map::new();
    for (key, value) in obj {
        if !known.contains(&key.as_str()) {
            extras.insert(key.clone(), value.clone());
        }
    }
    if extras.is_empty() {
        None
    } else {
        Some(serde_json::json!({ prefix: Value::Object(extras) }))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use attest_common::{AuthStatus, OcsfEvent};

    #[test]
    fn cloudtrail_id_and_not_an_alert() {
        let source = CloudTrailSource;
        assert_eq!(source.id(), "cloudtrail");
        assert!(!source.is_alert());
    }

    #[test]
    fn decode_envelope_and_bare_record() {
        let source = CloudTrailSource;
        let envelope = br#"{"Records":[{"eventName":"CreateUser"},{"eventName":"DeleteUser"}]}"#;
        let records = source.decode(envelope, Some("application/json")).unwrap();
        assert_eq!(records.len(), 2);

        let bare = br#"{"eventName":"ConsoleLogin"}"#;
        let records = source.decode(bare, None).unwrap();
        assert_eq!(records.len(), 1);
    }

    #[test]
    fn decode_rejects_non_json() {
        let err = CloudTrailSource.decode(b"not-json", None).unwrap_err();
        let msg = err.to_string();
        assert!(msg.contains("not JSON"), "{msg}");
    }

    #[test]
    fn normalize_console_login_matches_existing_class() {
        let raw = serde_json::json!({
            "eventTime": "2024-01-15T10:30:00Z",
            "eventName": "ConsoleLogin",
            "eventSource": "signin.amazonaws.com",
            "awsRegion": "us-west-2",
            "sourceIPAddress": "203.0.113.10",
            "recipientAccountId": "123456789012",
            "userIdentity": {
                "userName": "alice@example.com",
                "arn": "arn:aws:iam::123456789012:user/alice"
            }
        });
        let ctx = NormalizeCtx {
            tenant_id: "tenant-a".into(),
            extension_prefix: "aws".into(),
        };
        match CloudTrailSource.normalize(&raw, &ctx).unwrap() {
            OcsfEvent::Authentication(e) => {
                assert_eq!(e.actor.user.name, "alice@example.com");
                assert_eq!(e.status, AuthStatus::Success);
                assert_eq!(e.tenant_id, "tenant-a");
                assert!(e.unmapped.is_none());
            }
            other => panic!("expected Authentication, got {other:?}"),
        }
    }

    #[test]
    fn normalize_malformed_record_errors() {
        let ctx = NormalizeCtx {
            tenant_id: "tenant-a".into(),
            extension_prefix: "aws".into(),
        };
        let err = CloudTrailSource
            .normalize(&serde_json::json!([]), &ctx)
            .unwrap_err();
        assert!(!err.to_string().is_empty());
    }

    #[test]
    fn unknown_fields_land_under_unmapped_prefix() {
        let raw = serde_json::json!({
            "eventName": "CreateUser",
            "eventTime": "2024-01-15T11:00:00Z",
            "eventSource": "iam.amazonaws.com",
            "awsRegion": "us-east-1",
            "ticket": "1001"
        });
        let ctx = NormalizeCtx {
            tenant_id: "tenant-a".into(),
            extension_prefix: "aws".into(),
        };
        match CloudTrailSource.normalize(&raw, &ctx).unwrap() {
            OcsfEvent::CloudActivity(e) => {
                let unmapped = e.unmapped.expect("unknown field");
                assert_eq!(unmapped["aws"]["ticket"], "1001");
            }
            other => panic!("expected CloudActivity, got {other:?}"),
        }
    }

    #[test]
    fn topics_use_prefix_and_alerts_channel() {
        let layout = TopicNames::new("attest", true);
        assert_eq!(
            topics_for(&layout, "cloudtrail", false),
            vec![
                "attest.raw.cloudtrail".to_string(),
                "cloudtrail".to_string()
            ]
        );
        assert_eq!(
            topics_for(&layout, "graylog", true),
            vec![
                "attest.raw.graylog".to_string(),
                "attest.alerts".to_string()
            ]
        );

        let custom = TopicNames::new("tenant-a", false);
        assert_eq!(custom.raw("cloudtrail"), "tenant-a.raw.cloudtrail");
        assert_eq!(custom.alerts(), "tenant-a.alerts");
        assert_eq!(
            topics_for(&custom, "cloudtrail", false),
            vec!["tenant-a.raw.cloudtrail".to_string()]
        );
    }

    #[test]
    fn unsafe_prefix_falls_back_to_attest() {
        assert_eq!(
            TopicNames::new("../x", false).raw("cloudtrail"),
            "attest.raw.cloudtrail"
        );
        assert_eq!(
            TopicNames::new("", false).raw("cloudtrail"),
            "attest.raw.cloudtrail"
        );
    }

    #[test]
    fn risingwave_migration_adds_http_and_network_columns() {
        let sql = include_str!("../../../infra/risingwave/phase1_baseline.sql");
        for column in ["http_path", "http_status", "src_ip", "asn", "disposition"] {
            assert!(sql.contains(column), "phase1 baseline is missing {column}");
        }
    }
}
