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
///
/// For `det_*` materialized views we drop-and-recreate so that schema changes
/// (column renames, new columns) are always applied cleanly.
pub async fn apply_phase1_ddl(db: &Client, kafka_brokers: &str) -> anyhow::Result<()> {
    let ddl = include_str!("../../../infra/risingwave/phase1_baseline.sql")
        .replace("redpanda:9092", kafka_brokers);

    // Drop stale det_* views first so IF NOT EXISTS always creates fresh ones.
    let det_views = [
        "det_aws_root_account_use",
        "det_aws_cloudtrail_logging_disabled",
        "det_aws_s3_bucket_policy_made_public",
        "det_aws_console_login_from_anomalous_geolocation",
        "det_aws_new_iam_user_then_access_keys_sequence",
        "det_aws_iam_user_excessive_privilege",
        "det_okta_mfa_bypass_attempt",
        "det_okta_brute_force_authentication",
        "det_m365_mass_external_sharing",
        "det_m365_inbox_rule_auto_forward_external",
    ];
    for view in &det_views {
        let drop = format!("DROP MATERIALIZED VIEW IF EXISTS {view}");
        if let Err(e) = db.simple_query(&drop).await {
            tracing::warn!("could not drop {view}: {e} (continuing)");
        }
    }

    // Drop `--` comment lines first. A semicolon inside a comment (common in
    // prose) would otherwise split the next CREATE statement and prepend the
    // leftover comment text as invalid SQL.
    let ddl: String = ddl
        .lines()
        .filter(|line| !line.trim_start().starts_with("--"))
        .collect::<Vec<_>>()
        .join("\n");

    for stmt in ddl.split(';').map(str::trim).filter(|s| !s.is_empty()) {
        tracing::debug!("applying DDL: {}…", &stmt[..stmt.len().min(60)]);
        db.simple_query(stmt)
            .await
            .with_context(|| format!("DDL failed: {stmt}"))?;
    }

    tracing::info!("Phase 1 DDL applied successfully");
    Ok(())
}
