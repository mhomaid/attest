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
            },
        }
    }
}
