//! attest-control-plane — axum REST API backed by RisingWave + ClickHouse.
//!
//! Routes:
//!   GET  /healthz
//!   GET  /v1/events/recent?id=<uuid>
//!   GET  /v1/baselines/user/{name}
//!   POST /v1/warm/query      { "sql": "SELECT ..." }

mod db;
mod routes;
mod state;

use anyhow::Context;
use axum::{Router, routing::{get, post}};
use std::sync::Arc;
use tracing_subscriber::{fmt, EnvFilter};

use crate::{
    db::{apply_phase1_ddl, connect},
    routes::{
        baselines::get_user_baseline,
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

    tracing::info!("connecting to RisingWave at {rw_host}:{rw_port}");
    let db = connect(&rw_host, rw_port).await?;

    tracing::info!("applying Phase 1 DDL");
    apply_phase1_ddl(&db)
        .await
        .context("DDL migration failed")?;

    tracing::info!("ClickHouse URL: {ch_url}");

    let state = AppState { db };

    let app = Router::new()
        .route("/healthz", get(healthz))
        .route("/v1/events/recent", get(get_recent_event))
        .route("/v1/baselines/user/{name}", get(get_user_baseline))
        .with_state(state)
        .route("/v1/warm/query", post(post_warm_query))
        .with_state(ch_url);

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
