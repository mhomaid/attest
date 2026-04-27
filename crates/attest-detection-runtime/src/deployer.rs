//! Deploy compiled HELIQL detections as RisingWave materialized views.

use anyhow::{Context, Result};
use attest_heliql::{Detection, compile_to_risingwave, drop_view_sql};
use tokio_postgres::Client;
use tracing::info;

/// (Re-)deploy a detection: drop the view if it exists, then recreate it.
pub async fn deploy(db: &Client, detection: &Detection) -> Result<()> {
    let drop_sql   = drop_view_sql(&detection.id);
    let create_sql = compile_to_risingwave(detection)
        .with_context(|| format!("compile failed for detection '{}'", detection.id))?;

    db.execute(&drop_sql, &[])
        .await
        .with_context(|| format!("DROP VIEW failed for '{}'", detection.id))?;

    db.execute(&create_sql, &[])
        .await
        .with_context(|| format!("CREATE VIEW failed for '{}'", detection.id))?;

    info!(detection_id = %detection.id, "deployed");
    Ok(())
}

/// Deploy all detections, skipping (and logging) any that fail.
pub async fn deploy_all(db: &Client, detections: &[Detection]) -> usize {
    let mut ok = 0usize;
    for d in detections {
        match deploy(db, d).await {
            Ok(_)  => ok += 1,
            Err(e) => tracing::warn!(detection_id = %d.id, error = %e, "deploy failed"),
        }
    }
    info!("{ok}/{} detections deployed successfully", detections.len());
    ok
}
