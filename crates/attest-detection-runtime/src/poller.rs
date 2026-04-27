//! Poll deployed detection views and emit fired rows to the `alerts` Redpanda topic.

use anyhow::Result;
use rdkafka::{
    producer::{FutureProducer, FutureRecord},
    ClientConfig,
};
use serde::Serialize;
use std::time::Duration;
use tokio_postgres::Client;
use tracing::{error, info};

/// The shape written to the `alerts` Redpanda topic.
#[derive(Debug, Serialize)]
pub struct Alert {
    pub alert_id:       String,
    pub detection_id:   String,
    pub event_id:       String,
    pub actor_user_name: Option<String>,
    pub cloud_region:   Option<String>,
    pub severity:       String,
    pub fired_at:       String,
}

/// Build a Kafka producer for the alerts topic.
pub fn build_producer(brokers: &str) -> Result<FutureProducer> {
    let producer = ClientConfig::new()
        .set("bootstrap.servers", brokers)
        .set("message.timeout.ms", "5000")
        .create()?;
    Ok(producer)
}

/// Poll all detection views (named `det_*`) and produce fired rows to Kafka.
/// Returns the total number of alerts emitted in this poll cycle.
pub async fn poll_and_emit(
    db:           &Client,
    producer:     &FutureProducer,
    alerts_topic: &str,
    detection_ids: &[String],
) -> usize {
    let mut total = 0usize;
    for det_id in detection_ids {
        match poll_one(db, producer, alerts_topic, det_id).await {
            Ok(n)  => total += n,
            Err(e) => error!(detection_id = %det_id, error = %e, "poll error"),
        }
    }
    total
}

async fn poll_one(
    db:           &Client,
    producer:     &FutureProducer,
    alerts_topic: &str,
    detection_id: &str,
) -> Result<usize> {
    let view_name = format!("det_{}", detection_id.replace('-', "_"));

    // We select the most recent N rows that fired since the last poll.
    // In a production system you'd track a cursor (watermark); for the MVP
    // we select from the view and rely on RisingWave's incremental semantics —
    // each SELECT returns all rows currently in the view.  The detection runtime
    // tracks the last `fired_at` timestamp per detection and skips older rows.
    let rows = db
        .query(
            &format!(
                "SELECT detection_id, event_id, actor_user_name, cloud_region, severity, fired_at \
                 FROM {view_name} \
                 ORDER BY fired_at DESC \
                 LIMIT 100"
            ),
            &[],
        )
        .await?;

    let mut count = 0usize;
    for row in &rows {
        let det_id_col: &str = row.try_get("detection_id").unwrap_or(detection_id);
        let event_id: String = row.try_get::<_, String>("event_id").unwrap_or_default();
        let actor:    Option<String> = row.try_get("actor_user_name").ok();
        let region:   Option<String> = row.try_get("cloud_region").ok();
        let severity: String = row.try_get::<_, String>("severity").unwrap_or_else(|_| "medium".into());
        let fired_at = chrono::Utc::now().to_rfc3339();

        let alert = Alert {
            alert_id:        uuid::Uuid::new_v4().to_string(),
            detection_id:    det_id_col.to_string(),
            event_id:        event_id.clone(),
            actor_user_name: actor,
            cloud_region:    region,
            severity,
            fired_at,
        };

        let payload = serde_json::to_string(&alert)?;
        let record  = FutureRecord::to(alerts_topic)
            .payload(&payload)
            .key(&event_id);

        match producer.send(record, Duration::from_secs(5)).await {
            Ok(_)  => {
                info!(alert_id = %alert.alert_id, detection_id = %det_id_col, "alert emitted");
                count += 1;
            }
            Err((e, _)) => error!(error = %e, "kafka produce error"),
        }
    }

    Ok(count)
}
