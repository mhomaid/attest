//! attest-detection-runtime
//!
//! Lifecycle:
//!   1. Load all *.heliql rules from the rules directory.
//!   2. Compile each rule to RisingWave SQL and deploy as a materialized view.
//!   3. Poll every `POLL_INTERVAL_SECS` seconds for newly fired rows.
//!   4. Emit fired rows to the `alerts` Redpanda topic as JSON.

mod deployer;
mod loader;
mod poller;

use anyhow::{Context, Result};
use clap::Parser;
use std::path::PathBuf;
use std::time::Duration;
use tokio_postgres::NoTls;
use tracing::info;

#[derive(Parser, Debug)]
#[command(name = "attest-detection-runtime", about = "HELIQL detection runtime")]
struct Cli {
    /// Directory containing *.heliql rule files
    #[arg(long, env = "RULES_DIR", default_value = "/rules")]
    rules_dir: PathBuf,

    #[arg(long, env = "RISINGWAVE_HOST", default_value = "risingwave")]
    rw_host: String,

    #[arg(long, env = "RISINGWAVE_PORT", default_value_t = 4566)]
    rw_port: u16,

    #[arg(long, env = "KAFKA_BROKERS", default_value = "redpanda:9092")]
    kafka_brokers: String,

    #[arg(long, env = "ALERTS_TOPIC", default_value = "alerts")]
    alerts_topic: String,

    /// How often (seconds) to poll detection views for new alerts
    #[arg(long, env = "POLL_INTERVAL_SECS", default_value_t = 2)]
    poll_interval_secs: u64,
}

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "attest_detection_runtime=info".parse().unwrap()),
        )
        .init();

    let cli = Cli::parse();

    info!(
        rules_dir = %cli.rules_dir.display(),
        rw = format!("{}:{}", cli.rw_host, cli.rw_port),
        kafka = %cli.kafka_brokers,
        "attest-detection-runtime starting"
    );

    // ── Load rules ──────────────────────────────────────────────────────────
    let detections = loader::load_rules(&cli.rules_dir)
        .context("failed to load rules")?;

    if detections.is_empty() {
        tracing::warn!("no rules found in {}; continuing anyway", cli.rules_dir.display());
    }
    info!("{} detections loaded", detections.len());

    // ── Connect to RisingWave ───────────────────────────────────────────────
    let conn_str = format!(
        "host={} port={} user=root dbname=dev",
        cli.rw_host, cli.rw_port
    );
    let (db, conn) = tokio_postgres::connect(&conn_str, NoTls)
        .await
        .context("RisingWave connection failed")?;

    tokio::spawn(async move {
        if let Err(e) = conn.await {
            tracing::error!("RisingWave connection error: {e}");
        }
    });

    // ── Deploy detection views ──────────────────────────────────────────────
    let deployed = deployer::deploy_all(&db, &detections).await;
    info!("{deployed}/{} views deployed", detections.len());

    // ── Build Kafka producer ────────────────────────────────────────────────
    let producer = poller::build_producer(&cli.kafka_brokers)
        .context("Kafka producer creation failed")?;

    let detection_ids: Vec<String> = detections.iter().map(|d| d.id.clone()).collect();

    // ── Poll loop ───────────────────────────────────────────────────────────
    let mut interval = tokio::time::interval(Duration::from_secs(cli.poll_interval_secs));
    info!("polling every {}s for fired detections", cli.poll_interval_secs);

    loop {
        tokio::select! {
            _ = interval.tick() => {
                let n = poller::poll_and_emit(&db, &producer, &cli.alerts_topic, &detection_ids).await;
                if n > 0 {
                    info!("emitted {n} alerts this cycle");
                }
            }
            _ = tokio::signal::ctrl_c() => {
                info!("shutdown signal received");
                break;
            }
        }
    }

    Ok(())
}
