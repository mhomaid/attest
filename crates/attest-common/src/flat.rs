//! Flat Kafka message schema for the `cloudtrail` topic.
//!
//! Matches the RisingWave `cloudtrail_events` table.  Both the collector and
//! the load generator must produce this exact shape so consumers see a
//! consistent schema.

use crate::ocsf::OcsfEvent;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FlatEvent {
    pub event_id: String,
    pub class_uid: String,
    pub time: String,
    pub tenant_id: String,
    pub actor_user_name: String,
    pub actor_user_uid: String,
    pub cloud_region: String,
    pub cloud_account_uid: String,
    pub severity: String,
    pub auth_status: Option<String>,
    pub api_operation: Option<String>,
    pub api_service: Option<String>,
    pub raw: Option<serde_json::Value>,
    /// HTTP path (OCSF http_request.url.path). Absent on CloudTrail rows.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub http_path: Option<String>,
    /// HTTP status code, as text so the RisingWave VARCHAR column can ingest it.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub http_status: Option<String>,
    /// Source IP, when the event carries one.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub src_ip: Option<String>,
    /// Autonomous system number, as text (`AS15169`) once enrichment fills it.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub asn: Option<String>,
    /// OCSF disposition id or name (for example `Blocked`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub disposition: Option<String>,
}

impl FlatEvent {
    pub fn from_ocsf(event: &OcsfEvent) -> Self {
        match event {
            OcsfEvent::Authentication(e) => Self {
                event_id: e.event_id.to_string(),
                class_uid: "3002".into(),
                time: e.time.to_rfc3339(),
                tenant_id: e.tenant_id.clone(),
                actor_user_name: e.actor.user.name.clone(),
                actor_user_uid: e.actor.user.uid.clone(),
                cloud_region: e.cloud.region.clone(),
                cloud_account_uid: e.cloud.account_uid.clone(),
                severity: format!("{:?}", e.severity).to_lowercase(),
                auth_status: Some(format!("{:?}", e.status)),
                api_operation: e
                    .raw
                    .as_ref()
                    .and_then(|r| r.get("eventName"))
                    .and_then(|v| v.as_str())
                    .map(|s| s.to_string()),
                api_service: e
                    .raw
                    .as_ref()
                    .and_then(|r| r.get("eventSource"))
                    .and_then(|v| v.as_str())
                    .map(|s| s.to_string()),
                raw: e.raw.clone(),
                http_path: None,
                http_status: None,
                src_ip: source_ip(e.raw.as_ref()),
                asn: None,
                disposition: None,
            },
            OcsfEvent::CloudActivity(e) => Self {
                event_id: e.event_id.to_string(),
                class_uid: "6003".into(),
                time: e.time.to_rfc3339(),
                tenant_id: e.tenant_id.clone(),
                actor_user_name: e.actor.user.name.clone(),
                actor_user_uid: e.actor.user.uid.clone(),
                cloud_region: e.cloud.region.clone(),
                cloud_account_uid: e.cloud.account_uid.clone(),
                severity: format!("{:?}", e.severity).to_lowercase(),
                auth_status: None,
                api_operation: Some(e.api.operation.clone()),
                api_service: Some(e.api.service.clone()),
                raw: e.raw.clone(),
                http_path: None,
                http_status: None,
                src_ip: source_ip(e.raw.as_ref()),
                asn: None,
                disposition: None,
            },
        }
    }
}

fn source_ip(raw: Option<&serde_json::Value>) -> Option<String> {
    raw.and_then(|r| r.get("sourceIPAddress"))
        .and_then(|v| v.as_str())
        .map(str::to_string)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn old_flat_json_without_http_fields_still_parses() {
        let old = r#"{
            "event_id": "1",
            "class_uid": "3002",
            "time": "2024-01-15T10:30:00Z",
            "tenant_id": "tenant-a",
            "actor_user_name": "alice@example.com",
            "actor_user_uid": "arn:aws:iam::123456789012:user/alice",
            "cloud_region": "us-east-1",
            "cloud_account_uid": "123456789012",
            "severity": "informational"
        }"#;
        let event: FlatEvent = serde_json::from_str(old).unwrap();
        assert!(event.http_path.is_none());
        assert!(event.src_ip.is_none());
        assert!(event.disposition.is_none());
    }
}
