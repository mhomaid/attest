//! RisingWave connection pool (Postgres-wire-protocol client).

use anyhow::Context;
use std::sync::Arc;
use tokio_postgres::{Client, NoTls};

pub type Db = Arc<Client>;

/// Connect to RisingWave (Postgres wire on port 4566) and return a shared client.
pub async fn connect(host: &str, port: u16) -> anyhow::Result<Db> {
    let config = format!("host={host} port={port} dbname=dev user=root password=''");

    let (client, connection) = tokio_postgres::connect(&config, NoTls)
        .await
        .with_context(|| format!("connecting to RisingWave at {host}:{port}"))?;

    // Drive the connection in a background task.
    tokio::spawn(async move {
        if let Err(e) = connection.await {
            tracing::error!("RisingWave connection error: {e}");
        }
    });

    Ok(Arc::new(client))
}

/// Apply Phase 1 DDL idempotently.  RisingWave accepts IF NOT EXISTS.
/// `kafka_brokers` is substituted at runtime so the same SQL file works both
/// locally (redpanda:9092) and on Railway (redpanda.railway.internal:9092).
pub async fn apply_phase1_ddl(db: &Client, kafka_brokers: &str) -> anyhow::Result<()> {
    let ddl = include_str!("../../../infra/risingwave/phase1_baseline.sql")
        .replace("redpanda:9092", kafka_brokers);

    // Split on `;` and execute each non-empty statement individually.
    // We strip leading comment lines only for the emptiness check — the full
    // chunk (including comments) is sent to the server, which handles them fine.
    for stmt in ddl.split(';').map(str::trim).filter(|s| {
        s.lines()
            .any(|l| !l.trim_start().starts_with("--") && !l.trim().is_empty())
    }) {
        tracing::debug!("applying DDL: {}…", &stmt[..stmt.len().min(60)]);
        db.simple_query(stmt)
            .await
            .with_context(|| format!("DDL failed: {stmt}"))?;
    }

    tracing::info!("Phase 1 DDL applied successfully");
    Ok(())
}
