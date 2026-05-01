//! attest-load-gen — Direct-to-Kafka load generator.
//!
//! Two modes:
//!   Server mode (default): HTTP control API on :9100 — UI-driven.
//!   Run mode (--rate set):  one-shot benchmark, live stats, exit when done.
//!
//! Sampled triage: set --triage-pct N (or TRIAGE_PCT env) to also route N% of
//! events through the orchestrator POST /triage, measuring ML latency under load.

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
    about = "Direct-to-Kafka load generator for the Attest platform"
)]
pub struct Cli {
    #[arg(long, env = "KAFKA_BROKERS", default_value = "redpanda:9092")]
    pub brokers: String,

    #[arg(long, env = "LOAD_GEN_PORT", default_value = "9100")]
    pub port: u16,

    /// Orchestrator URL for sampled triage (e.g. http://orchestrator:4300).
    #[arg(long, env = "ORCHESTRATOR_URL")]
    pub orchestrator_url: Option<String>,

    // ── Run-mode flags ────────────────────────────────────────────────────────
    #[arg(long)]
    pub rate: Option<u64>,

    #[arg(long, default_value = "30")]
    pub duration: u64,

    #[arg(long, default_value = "mixed")]
    pub scenario: String,

    #[arg(long, default_value = "3")]
    pub tenants: usize,

    #[arg(long, default_value = "true")]
    pub seed_baselines: bool,

    /// Percentage of events to also route through the orchestrator for triage (0–100).
    #[arg(long, env = "TRIAGE_PCT", default_value = "0")]
    pub triage_pct: u8,
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    fmt()
        .with_env_filter(
            EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info")),
        )
        .init();

    let cli = Cli::parse();

    // ── Run mode ──────────────────────────────────────────────────────────────
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
            sampled_triage_pct: cli.triage_pct,
        };

        tracing::info!(
            rate,
            duration = cli.duration,
            triage_pct = cli.triage_pct,
            "starting run-mode benchmark"
        );

        let handle = generator::start_run(cli.brokers, cfg, cli.orchestrator_url)
            .await
            .context("failed to start run")?;

        let mut tick = tokio::time::interval(std::time::Duration::from_secs(5));
        tick.tick().await; // consume immediate tick

        loop {
            tick.tick().await;
            let s = handle.status();
            println!(
                "  sent={:>9}  rate={:>7.0}/s  errors={:>4}  p95={:.1}ms  triage_p95={:.1}ms  elapsed={:.1}s",
                s.sent, s.rate_actual, s.errors, s.p95_ms, s.triage_p95_ms, s.elapsed_secs,
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

    // ── Server mode ───────────────────────────────────────────────────────────
    let state: control::SharedState = Arc::new(RwLock::new(control::AppState::new(
        cli.brokers.clone(),
        cli.orchestrator_url,
    )));

    let app = control::build_router(state);
    let addr = format!("0.0.0.0:{}", cli.port);
    tracing::info!("attest-load-gen listening on {addr}");
    let listener = tokio::net::TcpListener::bind(&addr)
        .await
        .context("bind failed")?;

    axum::serve(listener, app).await.context("server error")?;
    Ok(())
}
