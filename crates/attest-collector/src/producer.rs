//! Kafka / Redpanda producer for OCSF events.

use attest_common::OcsfEvent;
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

    /// Produce a single `OcsfEvent` to the topic matching its class.
    pub async fn produce(&self, event: &OcsfEvent) -> Result<(), CollectorError> {
        let topic = topic_for(event);
        let key = event.tenant_id().to_string();
        let payload = serde_json::to_string(event)
            .map_err(CollectorError::Json)?;

        self.inner
            .send(
                FutureRecord::to(topic)
                    .key(&key)
                    .payload(&payload),
                Duration::from_secs(5),
            )
            .await
            .map_err(|(e, _)| CollectorError::Kafka(e.to_string()))?;

        tracing::debug!(
            topic,
            event_id = %event.event_id(),
            "produced event"
        );

        Ok(())
    }
}

fn topic_for(event: &OcsfEvent) -> &'static str {
    match event {
        OcsfEvent::Authentication(_) => "cloudtrail",
        OcsfEvent::CloudActivity(_) => "cloudtrail",
    }
}
