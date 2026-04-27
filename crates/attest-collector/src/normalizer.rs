//! CloudTrail JSON → OCSF normalizer.
//!
//! CloudTrail delivers events as `{"Records": [...]}`.  Each record is mapped
//! to the appropriate OCSF class based on `eventName`.

use attest_common::{
    Actor, ApiInfo, AuthStatus, AuthenticationEvent, Cloud, CloudActivityEvent, OcsfEvent,
    Resource, Severity, User,
};
use chrono::{DateTime, Utc};
use serde::Deserialize;
use uuid::Uuid;

use crate::error::CollectorError;

/// Raw CloudTrail record shape.  Only the fields we consume are listed; the
/// full record is preserved in `raw` for attribution.
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CloudTrailRecord {
    #[allow(dead_code)]
    pub event_id: Option<String>,
    pub event_time: Option<String>,
    pub event_name: Option<String>,
    pub event_source: Option<String>,
    pub aws_region: Option<String>,
    #[allow(dead_code)]
    pub source_ip_address: Option<String>,
    pub user_identity: Option<UserIdentity>,
    pub recipient_account_id: Option<String>,
    pub resources: Option<Vec<CloudTrailResource>>,
    pub error_code: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UserIdentity {
    #[allow(dead_code)]
    pub r#type: Option<String>,
    pub user_name: Option<String>,
    pub arn: Option<String>,
    pub account_id: Option<String>,
    pub principal_id: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CloudTrailResource {
    #[serde(rename = "ARN")]
    pub arn: Option<String>,
    pub resource_type: Option<String>,
}

/// Wrapper for the `{"Records": [...]}` envelope.
#[derive(Debug, Deserialize)]
#[allow(dead_code)]
pub struct CloudTrailEnvelope {
    #[serde(rename = "Records")]
    pub records: Vec<serde_json::Value>,
}

/// Normalize a raw CloudTrail JSON payload to zero or more `OcsfEvent`s.
pub fn normalize_cloudtrail(
    raw_json: &serde_json::Value,
    tenant_id: &str,
) -> Result<Vec<OcsfEvent>, CollectorError> {
    // Accept both a bare record and the `{"Records": [...]}` envelope.
    let records: Vec<serde_json::Value> = if let Some(arr) = raw_json.get("Records") {
        serde_json::from_value(arr.clone())?
    } else {
        vec![raw_json.clone()]
    };

    let mut events = Vec::with_capacity(records.len());
    for record_val in records {
        match normalize_record(&record_val, tenant_id) {
            Ok(ev) => events.push(ev),
            Err(e) => {
                tracing::warn!("skipping malformed CloudTrail record: {e}");
            }
        }
    }
    Ok(events)
}

fn normalize_record(
    val: &serde_json::Value,
    tenant_id: &str,
) -> Result<OcsfEvent, CollectorError> {
    let rec: CloudTrailRecord = serde_json::from_value(val.clone())?;

    let event_id = Uuid::new_v4();
    let time = parse_time(rec.event_time.as_deref())?;
    let identity = rec.user_identity.as_ref();

    let actor = Actor {
        user: User {
            name: identity
                .and_then(|u| u.user_name.clone())
                .or_else(|| identity.and_then(|u| u.arn.clone()))
                .unwrap_or_else(|| "unknown".into()),
            uid: identity
                .and_then(|u| u.arn.clone())
                .or_else(|| identity.and_then(|u| u.principal_id.clone()))
                .unwrap_or_else(|| "unknown".into()),
            account_uid: identity.and_then(|u| u.account_id.clone()),
        },
    };

    let cloud = Cloud {
        provider: "aws".into(),
        region: rec.aws_region.clone().unwrap_or_else(|| "unknown".into()),
        account_uid: rec
            .recipient_account_id
            .clone()
            .unwrap_or_else(|| "unknown".into()),
    };

    let event_name = rec.event_name.as_deref().unwrap_or("");

    // Route to the correct OCSF class based on event name.
    let ocsf_event = if is_authentication_event(event_name) {
        let status = if rec.error_code.is_some() {
            AuthStatus::Failure
        } else {
            AuthStatus::Success
        };
        OcsfEvent::Authentication(AuthenticationEvent {
            event_id,
            time,
            tenant_id: tenant_id.to_string(),
            actor,
            cloud,
            severity: Severity::Informational,
            status,
            raw: Some(val.clone()),
        })
    } else {
        let service = rec
            .event_source
            .clone()
            .unwrap_or_else(|| "aws".into());
        let resources = rec
            .resources
            .unwrap_or_default()
            .into_iter()
            .map(|r| Resource {
                uid: r.arn.unwrap_or_else(|| "unknown".into()),
                resource_type: r.resource_type.unwrap_or_else(|| "unknown".into()),
            })
            .collect();

        OcsfEvent::CloudActivity(CloudActivityEvent {
            event_id,
            time,
            tenant_id: tenant_id.to_string(),
            actor,
            cloud,
            api: ApiInfo {
                operation: event_name.to_string(),
                service,
            },
            severity: Severity::Informational,
            resources,
            raw: Some(val.clone()),
        })
    };

    Ok(ocsf_event)
}

fn is_authentication_event(event_name: &str) -> bool {
    matches!(
        event_name,
        "ConsoleLogin" | "AssumeRoleWithWebIdentity" | "AssumeRoleWithSAML" | "GetFederationToken"
    )
}

fn parse_time(s: Option<&str>) -> Result<DateTime<Utc>, CollectorError> {
    match s {
        Some(ts) => DateTime::parse_from_rfc3339(ts)
            .map(|dt| dt.with_timezone(&Utc))
            .map_err(|e| CollectorError::Normalization(format!("invalid time '{ts}': {e}"))),
        None => Ok(Utc::now()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn normalizes_console_login() {
        let raw = serde_json::json!({
            "Records": [{
                "eventId": "abc",
                "eventTime": "2024-01-15T10:30:00Z",
                "eventName": "ConsoleLogin",
                "eventSource": "signin.amazonaws.com",
                "awsRegion": "us-west-2",
                "recipientAccountId": "123456789012",
                "userIdentity": {
                    "type": "IAMUser",
                    "userName": "alice@example.com",
                    "arn": "arn:aws:iam::123456789012:user/alice",
                    "accountId": "123456789012"
                }
            }]
        });

        let events = normalize_cloudtrail(&raw, "tenant-1").unwrap();
        assert_eq!(events.len(), 1);
        match &events[0] {
            OcsfEvent::Authentication(e) => {
                assert_eq!(e.actor.user.name, "alice@example.com");
                assert_eq!(e.cloud.region, "us-west-2");
                assert_eq!(e.status, AuthStatus::Success);
            }
            _ => panic!("expected Authentication event"),
        }
    }

    #[test]
    fn normalizes_failed_login() {
        let raw = serde_json::json!({
            "eventName": "ConsoleLogin",
            "eventTime": "2024-01-15T10:30:00Z",
            "awsRegion": "eu-west-1",
            "errorCode": "Failed authentication",
            "userIdentity": { "userName": "bob@example.com", "arn": "arn:x" }
        });

        let events = normalize_cloudtrail(&raw, "t1").unwrap();
        assert_eq!(events.len(), 1);
        match &events[0] {
            OcsfEvent::Authentication(e) => assert_eq!(e.status, AuthStatus::Failure),
            _ => panic!("expected Authentication"),
        }
    }

    #[test]
    fn normalizes_cloud_activity() {
        let raw = serde_json::json!({
            "eventName": "CreateUser",
            "eventTime": "2024-01-15T11:00:00Z",
            "eventSource": "iam.amazonaws.com",
            "awsRegion": "us-east-1",
            "recipientAccountId": "999",
            "userIdentity": { "userName": "admin", "arn": "arn:y" }
        });

        let events = normalize_cloudtrail(&raw, "t1").unwrap();
        assert_eq!(events.len(), 1);
        match &events[0] {
            OcsfEvent::CloudActivity(e) => {
                assert_eq!(e.api.operation, "CreateUser");
                assert_eq!(e.cloud.region, "us-east-1");
            }
            _ => panic!("expected CloudActivity"),
        }
    }
}
