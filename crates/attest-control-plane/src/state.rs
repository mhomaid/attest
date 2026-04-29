use std::sync::{Arc, OnceLock};
use chrono::{DateTime, Utc};
use serde::Serialize;
use crate::db::Db;

/// Live platform metrics snapshot, broadcast at 1 Hz from the sampler task.
#[derive(Debug, Clone, Serialize)]
pub struct MetricsSnapshot {
    pub ts: DateTime<Utc>,
    pub events_per_sec: u64,
    pub consumer_lag: u64,
    pub detections_per_sec: u64,
    pub clickhouse_rows_per_sec: u64,
    pub triage_p95_ms: f32,
    pub total_events: u64,
    pub total_detections: u64,
    pub active_load_gen: bool,
    pub current_load_gen_rate: u64,
}

/// Shared application state injected into every route handler.
#[derive(Clone)]
pub struct AppState {
    /// Set exactly once by the background setup task when RisingWave is ready.
    pub db: Arc<OnceLock<Db>>,
    /// Broadcast channel: each message is a JSON-serialised `FiredAlert`.
    pub alert_tx: tokio::sync::broadcast::Sender<String>,
    /// Broadcast channel: 1 Hz `MetricsSnapshot` frames.
    pub metrics_tx: tokio::sync::broadcast::Sender<MetricsSnapshot>,
}

impl AppState {
    pub fn get_db(&self) -> Option<Db> {
        self.db.get().cloned()
    }
}
