//! attest-load-gen
//!
//! Two modes:
//!   Server mode (default): binds HTTP control API on :9100, waits for POST /run
//!   Run mode (--rate / --duration provided): starts the benchmark immediately,
//!     prints live stats every 5s, exits when done — no HTTP server started.
//!
//! Examples:
//!   # Server (UI-driven)
//!   attest-load-gen --brokers redpanda:9092
//!
//!   # One-shot CLI benchmark
//!   attest-load-gen --rate 100000 --duration 60 --scenario attack --tenants 5
//!
//!   # Smoke test (10k/sec, 30s, baseline pre-seed)
//!   attest-load-gen --rate 10000 --duration 30 --seed-baselines

use anyhow::Context;
use clap::Parser;
use std::sync::Arc;
use tokio::sync::RwLock;
use tracing_subscriber::{fmt, EnvFilter};

mod control;
mod generator;
mod scenarios;

use generator::{RunConfig, Scenario};

#[derive(Parser, Debug)]
#[command(
    name = "attest-load-gen",
    about = "Direct-to-Kafka load generator for the Attest platform",
    long_about = "Run without --rate to start the HTTP control API (server mode).\n\
                  Pass --rate to start a benchmark immediately and exit when done."
)]
pub struct Cli {
    /// Kafka broker list
    #[arg(long, env = "KAFKA_BROKERS", default_value = "redpanda:9092")]
    pub brokers: String,

    /// HTTP control API port (server mode only)
    #[arg(long, env = "LOAD_GEN_PORT", default_value = "9100")]
    pub port: u16,

    // ── Run-mode flags (all optional — when --rate is set, run immediately) ──

    /// Events per second (enables run mode when set)
    #[arg(long)]
    pub rate: Option<u64>,

    /// Run duration in seconds (0 = until Ctrl-C)
    #[arg(long, default_value = "30")]
    pub duration: u64,

    /// Scenario: mixed | attack | benign
    #[arg(long, default_value = "mixed")]
    pub scenario: String,

    /// Number of simulated tenants
    #[arg(long, default_value = "3")]
    pub tenants: usize,

    /// Pre-seed home-region baselines before the run
    #[arg(long, default_value = "true")]
    pub seed_baselines: bool,
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    fmt()
        .with_env_filter(
            EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info")),
        )
        .init();

    let cli = Cli::parse();

    // ── Run mode: start benchmark immediately, print stats, exit ─────────────
    if let Some(rate) = cli.rate {
        let scenario = match cli.scenario.as_str() {
            "attack" => Scenario::Attack,
            "benign" => Scenario::Benign,
            _ => Scenario::Mixed,
        };
        let cfg = RunConfig {
            rate,
            duration_secs: cli.duration,
            scenario,
            tenants: cli.tenants,
            seed_baselines: cli.seed_baselines,
        };

        tracing::info!(
            rate, duration = cli.duration, scenario = %cli.scenario, tenants = cli.tenants,
            "starting run-mode benchmark"
        );

        let handle = generator::start_run(cli.brokers, cfg)
            .await
            .context("failed to start run")?;

        // Print live stats every 5 seconds
        let mut print_interval = tokio::time::interval(std::time::Duration::from_secs(5));
        print_interval.tick().await; // consume immediate tick

        loop {
            print_interval.tick().await;
            let s = handle.status();
            println!(
                "  sent={:>9}  rate={:>7.0}/s  errors={:>4}  p50={:.1}ms  p95={:.1}ms  p99={:.1}ms  elapsed={:.1}s",
                s.sent, s.rate_actual, s.errors, s.p50_ms, s.p95_ms, s.p99_ms, s.elapsed_secs,
            );
            if !s.running {
                println!(
                    "\n✔ Run complete — {} events in {:.1}s ({:.0}/s avg)",
                    s.sent, s.elapsed_secs, s.rate_actual,
                );
                break;
            }
        }

        return Ok(());
    }

    // ── Server mode: bind HTTP control API and idle ───────────────────────────
    let state: control::SharedState =
        Arc::new(RwLock::new(control::AppState::new(cli.brokers.clone())));

    let app = control::build_router(state);
    let addr = format!("0.0.0.0:{}", cli.port);
    tracing::info!("attest-load-gen listening on {addr}  (POST /run to start a benchmark)");
    let listener = tokio::net::TcpListener::bind(&addr)
        .await
        .context("bind failed")?;

    axum::serve(listener, app).await.context("server error")?;
    Ok(())
}
