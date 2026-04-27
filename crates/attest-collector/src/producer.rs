//! Kafka / Redpanda producer for OCSF events.
//!
//! Events are flattened into a single-level JSON document before being
//! produced, matching the RisingWave `cloudtrail_events` table schema.

use attest_common::OcsfEvent;
use rdkafka::{
    producer::{FutureProducer, FutureRecord},
    ClientConfig,
};
use serde::Serialize;
use std::time::Duration;

use crate::error::CollectorError;

/// Flat Kafka message schema — matches the RisingWave `cloudtrail_events` table.
#[derive(Serialize)]
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
                auth_status: Some(format!("{:?}", e.status).to_lowercase()),
                api_operation: None,
                api_service: None,
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

pub struct EventProducer {
    inner: FutureProducer,
}

impl EventProducer {
    /// Create a producer connected to the given broker list.
    pub fn new(brokers: &str) -> Result<Self, CollectorError> {
        let producer: FutureProducer = ClientConfig::new()
            .set("bootstrap.servers", brokers)
            .set("message.timeout.ms", "5000")
            .set("enable.idempotence", "true")
            .create()
            .map_err(|e| CollectorError::Kafka(e.to_string()))?;

        Ok(Self { inner: producer })
    }

    /// Produce a single `OcsfEvent` to the `cloudtrail` topic as a flat JSON record.
    pub async fn produce(&self, event: &OcsfEvent) -> Result<(), CollectorError> {
        let flat = FlatEvent::from_ocsf(event);
        let key = flat.tenant_id.clone();
        let payload = serde_json::to_string(&flat).map_err(CollectorError::Json)?;

        self.inner
            .send(
                FutureRecord::to("cloudtrail")
                    .key(&key)
                    .payload(&payload),
                Duration::from_secs(5),
            )
            .await
            .map_err(|(e, _)| CollectorError::Kafka(e.to_string()))?;

        tracing::debug!(
            event_id = %event.event_id(),
            class_uid = flat.class_uid,
            "produced flat event"
        );

        Ok(())
    }
}
