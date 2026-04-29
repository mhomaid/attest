//! Kafka / Redpanda producer for OCSF events.
//!
//! Events are flattened into a single-level JSON document before being
//! produced, matching the RisingWave `cloudtrail_events` table schema.
//! The `FlatEvent` type now lives in `attest-common` so the load generator
//! can produce identical bytes without pulling in this crate.

use attest_common::{FlatEvent, OcsfEvent};
use rdkafka::{
    producer::{FutureProducer, FutureRecord},
    ClientConfig,
};
use std::time::Duration;

use crate::error::CollectorError;

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
