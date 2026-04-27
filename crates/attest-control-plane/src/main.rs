//! attest-control-plane — axum REST + WebSocket API backed by RisingWave + ClickHouse.
//!
//! Routes:
//!   GET  /healthz
//!   GET  /v1/events/recent?id=<uuid>
//!   GET  /v1/baselines/user/{name}
//!   POST /v1/warm/query             { "sql": "SELECT ..." }
//!   GET  /v1/detections/fired       — all rows from deployed det_* views
//!   GET  /v1/ws/alerts              — WebSocket: streams new fired rows

mod db;
mod routes;
mod state;

use anyhow::Context;
use axum::{Router, routing::{get, post}};
use rdkafka::admin::{AdminClient, AdminOptions, NewTopic, TopicReplication};
use rdkafka::client::DefaultClientContext;
use rdkafka::config::ClientConfig;
use std::collections::HashSet;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::broadcast;
use tower_http::cors::{Any, CorsLayer};
use tracing_subscriber::{fmt, EnvFilter};

use crate::{
    db::{apply_phase1_ddl, connect},
    routes::{
        baselines::get_user_baseline,
        detections::{fetch_all_fired, get_detections_fired, ws_alerts},
        events::get_recent_event,
        healthz::healthz,
        warm::{post_warm_query, ChUrl},
    },
    state::AppState,
};

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    fmt()
        .with_env_filter(
            EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info")),
        )
        .init();

    let rw_host = std::env::var("RISINGWAVE_HOST").unwrap_or_else(|_| "localhost".into());
    let rw_port: u16 = std::env::var("RISINGWAVE_PORT")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(4566);
    let port: u16 = std::env::var("PORT")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(8080);
    let ch_url: ChUrl = Arc::new(
        std::env::var("CLICKHOUSE_URL").unwrap_or_else(|_| "http://localhost:8123".into()),
    );
    let kafka_brokers =
        std::env::var("KAFKA_BROKERS").unwrap_or_else(|_| "redpanda:9092".into());
    let poll_secs: u64 = std::env::var("ALERT_POLL_SECS")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(2);

    tracing::info!("connecting to RisingWave at {rw_host}:{rw_port}");
    let db = connect(&rw_host, rw_port).await?;

    tracing::info!("ensuring Kafka topic 'cloudtrail' exists");
    ensure_kafka_topic(&kafka_brokers, "cloudtrail").await;

    tracing::info!("applying Phase 1 DDL (kafka_brokers={kafka_brokers})");
    apply_phase1_ddl(&db, &kafka_brokers)
        .await
        .context("DDL migration failed")?;

    tracing::info!("ClickHouse URL: {ch_url}");

    // ── Broadcast channel for real-time alert push ──────────────────────────
    let (alert_tx, _) = broadcast::channel::<String>(1024);

    let state = AppState { db, alert_tx: alert_tx.clone() };

    // ── Background polling task ─────────────────────────────────────────────
    // Polls all det_* views every `poll_secs` seconds and broadcasts new rows.
    let poll_state = state.clone();
    tokio::spawn(async move {
        let mut seen: HashSet<String> = HashSet::new();
        let mut interval = tokio::time::interval(Duration::from_secs(poll_secs));
        loop {
            interval.tick().await;
            match fetch_all_fired(&poll_state).await {
                Ok(alerts) => {
                    for alert in alerts {
                        let key = format!("{}:{}", alert.detection_id, alert.event_id);
                        if seen.contains(&key) {
                            continue;
                        }
                        seen.insert(key);
                        if let Ok(json) = serde_json::to_string(&alert) {
                            // Ignore send errors — no subscribers is fine.
                            let _ = poll_state.alert_tx.send(json);
                        }
                    }
                }
                Err(e) => tracing::warn!("alert poll error: {e}"),
            }
        }
    });

    // ── CORS ────────────────────────────────────────────────────────────────
    let cors = CorsLayer::new()
        .allow_origin(Any)
        .allow_methods(Any)
        .allow_headers(Any);

    // ── Router ──────────────────────────────────────────────────────────────
    let app = Router::new()
        .route("/healthz",                   get(healthz))
        .route("/v1/events/recent",          get(get_recent_event))
        .route("/v1/baselines/user/{name}",  get(get_user_baseline))
        .route("/v1/detections/fired",       get(get_detections_fired))
        .route("/v1/ws/alerts",              get(ws_alerts))
        .with_state(state)
        .route("/v1/warm/query",             post(post_warm_query))
        .with_state(ch_url)
        .layer(cors);

    let addr = format!("0.0.0.0:{port}");
    tracing::info!("attest-control-plane listening on {addr}");

    let listener = tokio::net::TcpListener::bind(&addr)
        .await
        .context("bind failed")?;

    axum::serve(listener, app)
        .await
        .context("server error")?;

    Ok(())
}

/// Idempotently create the Kafka topic if it doesn't exist.
/// Logs and continues on error so a pre-existing topic or transient failure
/// doesn't abort startup.
async fn ensure_kafka_topic(brokers: &str, topic: &str) {
    let admin: AdminClient<DefaultClientContext> = match ClientConfig::new()
        .set("bootstrap.servers", brokers)
        .set("socket.timeout.ms", "10000")
        .create()
    {
        Ok(c) => c,
        Err(e) => {
            tracing::warn!("kafka admin client creation failed: {e}");
            return;
        }
    };

    let new_topic = NewTopic::new(topic, 1, TopicReplication::Fixed(1));
    let opts = AdminOptions::new().operation_timeout(Some(Duration::from_secs(10)));
    match admin.create_topics(&[new_topic], &opts).await {
        Ok(results) => {
            for res in results {
                match res {
                    Ok(name) => tracing::info!("kafka topic '{name}' created"),
                    Err((name, rdkafka::error::RDKafkaErrorCode::TopicAlreadyExists)) => {
                        tracing::info!("kafka topic '{name}' already exists");
                    }
                    Err((name, e)) => tracing::warn!("kafka topic '{name}' create error: {e}"),
                }
            }
        }
        Err(e) => tracing::warn!("kafka admin create_topics failed: {e}"),
    }
}
