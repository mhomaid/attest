//! attest-control-plane — axum REST + WebSocket API backed by RisingWave + ClickHouse.
//!
//! Startup order (resilient to infra being offline at boot):
//!   1. Bind TCP listener immediately → /healthz returns 200 straight away.
//!   2. Background task: retry-connect to RisingWave, apply DDL, fill OnceLock.
//!   3. Routes that need DB return 503 until the OnceLock is populated.
//!
//! Routes:
//!   GET  /healthz
//!   GET  /v1/events/recent?id=<uuid>
//!   GET  /v1/baselines/user/{name}
//!   POST /v1/warm/query             { "sql": "SELECT ..." }
//!   GET  /v1/detections/fired
//!   GET  /v1/ws/alerts

mod db;
mod routes;
mod state;

use anyhow::Context;
use axum::{Router, routing::{get, post}};
use rdkafka::admin::{AdminClient, AdminOptions, NewTopic, TopicReplication};
use rdkafka::client::DefaultClientContext;
use rdkafka::config::ClientConfig;
use std::collections::HashSet;
use std::sync::{Arc, OnceLock};
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

    // ── Broadcast channel ───────────────────────────────────────────────────
    let (alert_tx, _) = broadcast::channel::<String>(1024);

    // ── AppState with an empty db slot ─────────────────────────────────────
    let db_slot: Arc<OnceLock<crate::db::Db>> = Arc::new(OnceLock::new());
    let state = AppState { db: db_slot.clone(), alert_tx: alert_tx.clone() };

    // ── Background setup: connect + DDL, then fill the db slot ─────────────
    {
        let db_slot = db_slot.clone();
        let state_for_poll = state.clone();
        tokio::spawn(async move {
            // Retry until RisingWave is reachable.
            let db = connect_with_retry(&rw_host, rw_port).await;

            // Kafka topic — non-fatal on error.
            tracing::info!("ensuring Kafka topic 'cloudtrail'");
            ensure_kafka_topic(&kafka_brokers, "cloudtrail").await;

            // DDL — non-fatal; schema may already exist.
            if let Err(e) = apply_phase1_ddl(&db, &kafka_brokers).await {
                tracing::warn!("DDL warning (continuing): {e}");
            }

            // Make DB available to all route handlers.
            let _ = db_slot.set(db);
            tracing::info!("control-plane fully ready — DB slot filled");

            // ── Background alert polling ────────────────────────────────────
            let mut seen: HashSet<String> = HashSet::new();
            let mut interval = tokio::time::interval(Duration::from_secs(poll_secs));
            loop {
                interval.tick().await;
                match fetch_all_fired(&state_for_poll).await {
                    Ok(alerts) => {
                        for alert in alerts {
                            let key = format!("{}:{}", alert.detection_id, alert.event_id);
                            if seen.contains(&key) { continue; }
                            seen.insert(key);
                            if let Ok(json) = serde_json::to_string(&alert) {
                                let _ = state_for_poll.alert_tx.send(json);
                            }
                        }
                    }
                    Err(e) => tracing::warn!("alert poll error: {e}"),
                }
            }
        });
    }

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

    // ── Bind and serve — must happen AFTER setup spawn so healthcheck passes ─
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

/// Connect to RisingWave, retrying every 5 s with no overall timeout.
async fn connect_with_retry(host: &str, port: u16) -> crate::db::Db {
    let mut attempt = 0u32;
    loop {
        attempt += 1;
        match connect(host, port).await {
            Ok(db) => {
                tracing::info!("connected to RisingWave at {host}:{port} (attempt {attempt})");
                return db;
            }
            Err(e) => {
                tracing::warn!(
                    "RisingWave not ready at {host}:{port} (attempt {attempt}): {e} — retrying in 5s"
                );
                tokio::time::sleep(Duration::from_secs(5)).await;
            }
        }
    }
}

/// Idempotently ensure a Kafka topic exists.  Logs and returns on any error.
async fn ensure_kafka_topic(brokers: &str, topic: &str) {
    let admin: AdminClient<DefaultClientContext> = match ClientConfig::new()
        .set("bootstrap.servers", brokers)
        .set("socket.timeout.ms", "10000")
        .create()
    {
        Ok(c) => c,
        Err(e) => { tracing::warn!("kafka admin create failed: {e}"); return; }
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
                    Err((name, e)) => tracing::warn!("kafka topic '{name}' error: {e}"),
                }
            }
        }
        Err(e) => tracing::warn!("kafka create_topics failed: {e}"),
    }
}
