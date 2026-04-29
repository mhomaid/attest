use anyhow::Context;
use clap::Parser;
use std::sync::Arc;
use tokio::sync::RwLock;
use tracing_subscriber::{fmt, EnvFilter};

mod control;
mod generator;
mod scenarios;

#[derive(Parser, Debug)]
#[command(name = "attest-load-gen", about = "Direct-to-Kafka load generator")]
pub struct Cli {
    #[arg(long, env = "KAFKA_BROKERS", default_value = "redpanda:9092")]
    pub brokers: String,

    #[arg(long, env = "LOAD_GEN_PORT", default_value = "9100")]
    pub port: u16,
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    fmt()
        .with_env_filter(
            EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info")),
        )
        .init();

    let cli = Cli::parse();

    let state: control::SharedState =
        Arc::new(RwLock::new(control::AppState::new(cli.brokers.clone())));

    let app = control::build_router(state);
    let addr = format!("0.0.0.0:{}", cli.port);
    tracing::info!("attest-load-gen listening on {addr}");
    let listener = tokio::net::TcpListener::bind(&addr)
        .await
        .context("bind failed")?;

    axum::serve(listener, app).await.context("server error")?;
    Ok(())
}
