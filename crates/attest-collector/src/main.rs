//! attest-collector — CloudTrail → OCSF → Redpanda edge collector.
//!
//! Usage:
//!   attest-collector serve          # Start HTTP ingest server on PORT (default 4000)
//!   attest-collector ingest-file <path>   # One-shot file ingest

mod error;
mod http;
mod normalizer;
mod producer;

use std::sync::Arc;

use anyhow::Context;
use axum::{Router, middleware::from_fn, routing::{get, post}};
use clap::{Parser, Subcommand};
use utoipa::OpenApi;
use utoipa_scalar::{Scalar, Servable as _};

use crate::{
    http::{AppState, ErrorResponse, HealthResponse, IngestResponse, healthz, ingest},
    normalizer::normalize_cloudtrail,
    producer::EventProducer,
};

#[derive(OpenApi)]
#[openapi(
    paths(http::healthz, http::ingest),
    components(schemas(IngestResponse, ErrorResponse, HealthResponse)),
    info(
        title = "Attest Collector",
        version = "0.1.0",
        description = "CloudTrail → OCSF → Redpanda edge collector. \
            POST /ingest to send events; they are normalised and produced to the `cloudtrail` Kafka topic."
    )
)]
struct ApiDoc;

#[derive(Parser)]
#[command(name = "attest-collector", about = "Attest edge collector")]
struct Cli {
    /// Redpanda / Kafka broker list.
    #[arg(long, env = "KAFKA_BROKERS", default_value = "localhost:9092")]
    kafka_brokers: String,

    /// Tenant ID stamped on every produced event.
    #[arg(long, env = "TENANT_ID", default_value = "default")]
    tenant_id: String,

    /// HTTP server port.
    #[arg(long, env = "PORT", default_value = "4000")]
    port: u16,

    #[command(subcommand)]
    command: Option<Command>,
}

#[derive(Subcommand)]
enum Command {
    /// Start the HTTP ingest server (default when no subcommand given).
    Serve,
    /// Ingest a local CloudTrail JSON file.
    IngestFile {
        /// Path to a CloudTrail JSON file.
        path: String,
    },
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let service_name =
        std::env::var("OTEL_SERVICE_NAME").unwrap_or_else(|_| "attest-collector".into());
    let _otel = attest_telemetry::init_subscriber_with_otel(&service_name)?;

    let cli = Cli::parse();

    let producer = Arc::new(
        EventProducer::new(&cli.kafka_brokers)
            .context("failed to create Kafka producer")?,
    );

    match cli.command.unwrap_or(Command::Serve) {
        Command::Serve => serve(producer, &cli.tenant_id, cli.port).await?,
        Command::IngestFile { path } => {
            ingest_file(producer, &cli.tenant_id, &path).await?;
        }
    }

    Ok(())
}

async fn serve(
    producer: Arc<EventProducer>,
    tenant_id: &str,
    port: u16,
) -> anyhow::Result<()> {
    let state = AppState {
        producer,
        tenant_id: tenant_id.to_string(),
    };

    let api = Router::new()
        .route("/healthz", get(healthz))
        .route("/ingest", post(ingest))
        .with_state(state);

    let app = Router::new()
        .merge(api)
        .merge(Scalar::with_url("/docs", ApiDoc::openapi()))
        .layer(from_fn(attest_telemetry::axum_trace_propagation));

    let addr = format!("0.0.0.0:{port}");
    tracing::info!("attest-collector listening on {addr}");

    let listener = tokio::net::TcpListener::bind(&addr)
        .await
        .context("failed to bind")?;

    axum::serve(listener, app)
        .await
        .context("server error")?;

    Ok(())
}

async fn ingest_file(
    producer: Arc<EventProducer>,
    tenant_id: &str,
    path: &str,
) -> anyhow::Result<()> {
    let content = tokio::fs::read_to_string(path)
        .await
        .with_context(|| format!("failed to read {path}"))?;

    let raw: serde_json::Value = serde_json::from_str(&content)
        .context("invalid JSON")?;

    let dummy_state = crate::http::AppState {
        producer,
        tenant_id: tenant_id.to_string(),
    };

    let events = normalize_cloudtrail(&raw, tenant_id)?;
    let mut count = 0usize;
    for event in &events {
        dummy_state.producer.produce(event).await?;
        count += 1;
    }

    tracing::info!(count, "file ingest complete");
    Ok(())
}
