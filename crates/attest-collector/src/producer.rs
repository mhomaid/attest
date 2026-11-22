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
use crate::source::{topics_for, CloudTrailSource, Source, TopicNames};

pub struct EventProducer {
    inner: FutureProducer,
    topics: TopicNames,
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

        Ok(Self {
            inner: producer,
            topics: TopicNames::from_env(),
        })
    }

    /// Produce a CloudTrail event. Writes `{prefix}.raw.cloudtrail` and, unless
    /// `ATTEST_MIRROR_LEGACY_CLOUDTRAIL=off`, the historical `cloudtrail` topic.
    pub async fn produce(&self, event: &OcsfEvent) -> Result<(), CollectorError> {
        let source = CloudTrailSource;
        self.produce_source(event, source.id(), source.is_alert())
            .await
    }

    /// Produce one event for any source.
    pub async fn produce_source(
        &self,
        event: &OcsfEvent,
        source_id: &str,
        is_alert: bool,
    ) -> Result<(), CollectorError> {
        let flat = FlatEvent::from_ocsf(event);
        let key = flat.tenant_id.clone();
        let payload = serde_json::to_string(&flat).map_err(CollectorError::Json)?;

        for topic in topics_for(&self.topics, source_id, is_alert) {
            self.inner
                .send(
                    FutureRecord::to(&topic).key(&key).payload(&payload),
                    Duration::from_secs(5),
                )
                .await
                .map_err(|(e, _)| CollectorError::Kafka(e.to_string()))?;
            tracing::debug!(
                event_id = %event.event_id(),
                class_uid = flat.class_uid,
                topic,
                "produced flat event"
            );
        }

        Ok(())
    }
}
