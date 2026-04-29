//! 1 Hz sampler that builds `MetricsSnapshot` and broadcasts on `metrics_tx`.
//!
//! Kafka high-water mark:  rdkafka `BaseConsumer::fetch_watermarks`
//! Consumer lag:            HWM − committed offset for group `cloudtrail-rw-consumer`
//! ClickHouse row delta:    HTTP SELECT count() against `cloudtrail_events`
//! Load-gen status:         HTTP GET to load-gen :9100/status (optional)

use std::sync::Arc;
use std::time::Duration;
use tokio::sync::{broadcast, Mutex};
use tracing::{debug, warn};

use rdkafka::{
    consumer::{BaseConsumer, Consumer},
    config::ClientConfig,
    TopicPartitionList,
};
use chrono::Utc;

use crate::state::MetricsSnapshot;

struct SamplerState {
    prev_hwm: i64,
    prev_ch_rows: u64,
    prev_detections: u64,
    total_events: u64,
    total_detections: u64,
}

pub async fn run_sampler(
    brokers: String,
    ch_url: String,
    load_gen_url: Option<String>,
    metrics_tx: broadcast::Sender<MetricsSnapshot>,
) {
    let state = Arc::new(Mutex::new(SamplerState {
        prev_hwm: 0,
        prev_ch_rows: 0,
        prev_detections: 0,
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
            &ch_url,
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
    ch_url: &str,
    load_gen_url: Option<&str>,
    http: &reqwest::Client,
    state: &Arc<Mutex<SamplerState>>,
) -> MetricsSnapshot {
    let mut st = state.lock().await;

    // ── Kafka high-water mark ────────────────────────────────────────────────
    let (hwm, consumer_lag) = kafka_hwm_and_lag(brokers).await;
    let events_per_sec = (hwm - st.prev_hwm).max(0) as u64;
    st.total_events += events_per_sec;
    st.prev_hwm = hwm;

    // ── ClickHouse row count ─────────────────────────────────────────────────
    let ch_rows = clickhouse_row_count(ch_url, http).await;
    let clickhouse_rows_per_sec = ch_rows.saturating_sub(st.prev_ch_rows);
    st.prev_ch_rows = ch_rows;

    // ── Detection count delta ────────────────────────────────────────────────
    let det_total = detection_count(ch_url, http).await;
    let detections_per_sec = det_total.saturating_sub(st.prev_detections);
    st.total_detections += detections_per_sec;
    st.prev_detections = det_total;

    // ── Load-gen status (optional) ──────────────────────────────────────────
    let (active_load_gen, current_load_gen_rate) = if let Some(url) = load_gen_url {
        load_gen_status(url, http).await
    } else {
        (false, 0)
    };

    MetricsSnapshot {
        ts: Utc::now(),
        events_per_sec,
        consumer_lag,
        detections_per_sec,
        clickhouse_rows_per_sec,
        triage_p95_ms: 0.0, // placeholder — triage p95 requires orchestrator metrics
        total_events: st.total_events,
        total_detections: st.total_detections,
        active_load_gen,
        current_load_gen_rate,
    }
}

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

            // Fetch committed offset for the RisingWave consumer group
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
        Ok(Err(e)) => { warn!("kafka sample error: {e}"); (0, 0) }
        Err(e) => { warn!("kafka sample join error: {e}"); (0, 0) }
    }
}

async fn clickhouse_row_count(ch_url: &str, http: &reqwest::Client) -> u64 {
    let query = "SELECT count() FROM cloudtrail_events FORMAT TabSeparated";
    let url = format!("{}/?query={}", ch_url, urlencoding(query));
    match http.get(&url).send().await {
        Ok(r) if r.status().is_success() => {
            r.text().await.ok()
                .and_then(|t| t.trim().parse::<u64>().ok())
                .unwrap_or(0)
        }
        _ => 0,
    }
}

async fn detection_count(ch_url: &str, http: &reqwest::Client) -> u64 {
    let query = "SELECT count() FROM detections FORMAT TabSeparated";
    let url = format!("{}/?query={}", ch_url, urlencoding(query));
    match http.get(&url).send().await {
        Ok(r) if r.status().is_success() => {
            r.text().await.ok()
                .and_then(|t| t.trim().parse::<u64>().ok())
                .unwrap_or(0)
        }
        _ => 0,
    }
}

async fn load_gen_status(load_gen_url: &str, http: &reqwest::Client) -> (bool, u64) {
    let url = format!("{}/status", load_gen_url);
    match http.get(&url).send().await {
        Ok(r) if r.status().is_success() => {
            if let Ok(v) = r.json::<serde_json::Value>().await {
                let running = v["running"].as_bool().unwrap_or(false);
                let rate = v["rate_actual"].as_f64().unwrap_or(0.0) as u64;
                return (running, rate);
            }
            (false, 0)
        }
        _ => (false, 0),
    }
}

fn urlencoding(s: &str) -> String {
    let mut out = String::with_capacity(s.len() * 2);
    for byte in s.bytes() {
        match byte {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                out.push(byte as char);
            }
            b' ' => out.push('+'),
            b => out.push_str(&format!("%{:02X}", b)),
        }
    }
    out
}
