//! 1 Hz sampler that builds `MetricsSnapshot` and broadcasts on `metrics_tx`.
//!
//! Kafka high-water mark:  rdkafka `BaseConsumer::fetch_watermarks`
//! Consumer lag:            HWM − committed offset for group `cloudtrail-rw-consumer`
//! Detections per sec:      delta on `alerts` Kafka topic HWM (fired-alert accumulator)
//! ClickHouse row delta:    HTTP SELECT count() against `cloudtrail_events`
//! Triage p95:              GET ORCHESTRATOR_URL/metrics → p95_ms
//! Load-gen status:         GET LOAD_GEN_URL/status (optional)

use std::sync::Arc;
use std::time::Duration;
use tokio::sync::{broadcast, Mutex};
use tracing::{debug, warn};

use chrono::Utc;
use rdkafka::{
    config::ClientConfig,
    consumer::{BaseConsumer, Consumer},
    TopicPartitionList,
};

use crate::state::MetricsSnapshot;
use attest_storage_clickhouse::ClickHouseClient;

struct SamplerState {
    prev_hwm: i64,
    prev_alerts_hwm: i64,
    prev_ch_rows: u64,
    total_events: u64,
    total_detections: u64,
}

pub async fn run_sampler(
    brokers: String,
    clickhouse: ClickHouseClient,
    load_gen_url: Option<String>,
    _orchestrator_url: Option<String>, // triage_p95_ms now comes directly from load-gen /status
    metrics_tx: broadcast::Sender<MetricsSnapshot>,
) {
    let state = Arc::new(Mutex::new(SamplerState {
        prev_hwm: 0,
        prev_alerts_hwm: 0,
        prev_ch_rows: 0,
        total_events: 0,
        total_detections: 0,
    }));

    let http_client = reqwest::Client::builder()
        .timeout(Duration::from_secs(2))
        .build()
        .expect("http client");

    let mut interval = tokio::time::interval(Duration::from_secs(1));

    loop {
        interval.tick().await;

        let snap = sample_once(
            &brokers,
            &clickhouse,
            load_gen_url.as_deref(),
            &http_client,
            &state,
        )
        .await;

        if let Err(e) = metrics_tx.send(snap) {
            debug!("metrics_tx: no subscribers ({e})");
        }
    }
}

async fn sample_once(
    brokers: &str,
    clickhouse: &ClickHouseClient,
    load_gen_url: Option<&str>,
    http: &reqwest::Client,
    state: &Arc<Mutex<SamplerState>>,
) -> MetricsSnapshot {
    let mut st = state.lock().await;

    // ── Kafka: cloudtrail HWM → events/sec + consumer lag ───────────────────
    let (hwm, consumer_lag) = kafka_hwm_and_lag(brokers).await;
    let events_per_sec = (hwm - st.prev_hwm).max(0) as u64;
    st.total_events += events_per_sec;
    st.prev_hwm = hwm;

    // ── Kafka: alerts HWM → detections/sec ──────────────────────────────────
    // The detection-runtime emits one message per fired alert to the `alerts` topic.
    // HWM delta gives us detections fired in the last second.
    let alerts_hwm = kafka_topic_hwm(brokers, "alerts").await;
    let detections_per_sec = (alerts_hwm - st.prev_alerts_hwm).max(0) as u64;
    st.total_detections += detections_per_sec;
    st.prev_alerts_hwm = alerts_hwm;

    // ── ClickHouse: event row count (confirms warm-path storage) ────────────
    let ch_rows = clickhouse
        .count_rows("cloudtrail_events")
        .await
        .unwrap_or_else(|e| {
            debug!("clickhouse row count unavailable: {e}");
            st.prev_ch_rows
        });
    let clickhouse_rows_per_sec = ch_rows.saturating_sub(st.prev_ch_rows);
    st.prev_ch_rows = ch_rows;

    // ── Load-gen status (optional) ──────────────────────────────────────────
    let (active_load_gen, current_load_gen_rate, triage_p95_ms) = if let Some(url) = load_gen_url {
        load_gen_status(url, http).await
    } else {
        (false, 0, 0.0)
    };

    MetricsSnapshot {
        ts: Utc::now(),
        events_per_sec,
        consumer_lag,
        detections_per_sec,
        clickhouse_rows_per_sec,
        triage_p95_ms,
        total_events: st.total_events,
        total_detections: st.total_detections,
        active_load_gen,
        current_load_gen_rate,
    }
}

// ── Kafka helpers ─────────────────────────────────────────────────────────────

/// Fetch HWM for partition 0 of `cloudtrail` and the consumer lag for the
/// RisingWave consumer group.
async fn kafka_hwm_and_lag(brokers: &str) -> (i64, u64) {
    let result = tokio::task::spawn_blocking({
        let brokers = brokers.to_string();
        move || -> anyhow::Result<(i64, u64)> {
            let consumer: BaseConsumer = ClientConfig::new()
                .set("bootstrap.servers", &brokers)
                .set("group.id", "metrics-sampler")
                .set("socket.timeout.ms", "2000")
                .create()?;

            let timeout = Duration::from_secs(2);
            let (_, hwm) = consumer.fetch_watermarks("cloudtrail", 0, timeout)?;

            let mut tpl = TopicPartitionList::new();
            tpl.add_partition("cloudtrail", 0);
            let committed = consumer.committed_offsets(tpl, timeout)?;
            let committed_offset = committed
                .find_partition("cloudtrail", 0)
                .and_then(|e| {
                    if let rdkafka::Offset::Offset(o) = e.offset() {
                        Some(o)
                    } else {
                        None
                    }
                })
                .unwrap_or(0);

            let lag = (hwm - committed_offset).max(0) as u64;
            Ok((hwm, lag))
        }
    })
    .await;

    match result {
        Ok(Ok((hwm, lag))) => (hwm, lag),
        Ok(Err(e)) => {
            warn!("kafka cloudtrail sample error: {e}");
            (0, 0)
        }
        Err(e) => {
            warn!("kafka cloudtrail join error: {e}");
            (0, 0)
        }
    }
}

/// Fetch the high-water mark for partition 0 of an arbitrary topic.
/// Returns 0 on any error (topic may not exist yet).
async fn kafka_topic_hwm(brokers: &str, topic: &str) -> i64 {
    let result = tokio::task::spawn_blocking({
        let brokers = brokers.to_string();
        let topic = topic.to_string();
        move || -> anyhow::Result<i64> {
            let consumer: BaseConsumer = ClientConfig::new()
                .set("bootstrap.servers", &brokers)
                .set("group.id", "metrics-sampler-hwm")
                .set("socket.timeout.ms", "2000")
                .create()?;
            let (_, hwm) = consumer.fetch_watermarks(&topic, 0, Duration::from_secs(2))?;
            Ok(hwm)
        }
    })
    .await;

    match result {
        Ok(Ok(hwm)) => hwm,
        Ok(Err(e)) => {
            debug!("kafka {topic} hwm: {e}");
            0
        }
        Err(e) => {
            warn!("kafka {topic} hwm join error: {e}");
            0
        }
    }
}

// ── HTTP helpers ──────────────────────────────────────────────────────────────

async fn load_gen_status(load_gen_url: &str, http: &reqwest::Client) -> (bool, u64, f32) {
    let url = format!("{load_gen_url}/status");
    match http.get(&url).send().await {
        Ok(r) if r.status().is_success() => {
            if let Ok(v) = r.json::<serde_json::Value>().await {
                let running = v["running"].as_bool().unwrap_or(false);
                let rate = v["rate_actual"].as_f64().unwrap_or(0.0) as u64;
                let triage_p95 = v["triage_p95_ms"].as_f64().unwrap_or(0.0) as f32;
                return (running, rate, triage_p95);
            }
            (false, 0, 0.0)
        }
        _ => (false, 0, 0.0),
    }
}
