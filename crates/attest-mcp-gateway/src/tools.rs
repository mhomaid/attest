//! Internal tool implementations dispatched by the gateway.
//!
//! Phase 4a stubs: each tool returns a realistic but synthetic response so
//! the orchestrator integration tests pass without live ClickHouse / RisingWave.
//! Phase 4b will replace stubs with real queries once the Railway infra is stable.

use anyhow::Result;
use serde_json::{json, Value};

/// Dispatch a tool call to the appropriate internal implementation.
pub async fn dispatch(tool_id: &str, args: &Value) -> Result<Value> {
    match tool_id {
        "query_hot_tier" => query_hot_tier(args).await,
        "query_warm_tier" => query_warm_tier(args).await,
        "lookup_threat_intel" => lookup_threat_intel(args).await,
        "get_asset_context" => get_asset_context(args).await,
        "get_user_baseline" => get_user_baseline(args).await,
        other => anyhow::bail!("unknown tool: {other}"),
    }
}

async fn query_hot_tier(args: &Value) -> Result<Value> {
    let sql = args["sql"].as_str().unwrap_or("(no sql)");
    tracing::debug!(sql, "query_hot_tier stub");
    Ok(json!({
        "rows": [],
        "total": 0,
        "source": "clickhouse_hot_tier_stub",
        "query": sql
    }))
}

async fn query_warm_tier(args: &Value) -> Result<Value> {
    let sql = args["sql"].as_str().unwrap_or("(no sql)");
    tracing::debug!(sql, "query_warm_tier stub");
    Ok(json!({
        "rows": [],
        "total": 0,
        "source": "iceberg_warm_tier_stub",
        "query": sql
    }))
}

async fn lookup_threat_intel(args: &Value) -> Result<Value> {
    let indicator = args["indicator"].as_str().unwrap_or("(unknown)");
    Ok(json!({
        "indicator": indicator,
        "malicious": false,
        "confidence": 0.0,
        "source": "threat_intel_stub",
        "tags": []
    }))
}

async fn get_asset_context(args: &Value) -> Result<Value> {
    let asset = args["asset"].as_str().unwrap_or("(unknown)");
    Ok(json!({
        "asset": asset,
        "criticality": 0.5,
        "owner": "unknown",
        "environment": "production",
        "source": "asset_context_stub"
    }))
}

async fn get_user_baseline(args: &Value) -> Result<Value> {
    let principal = args["principal"].as_str().unwrap_or("(unknown)");
    Ok(json!({
        "principal": principal,
        "avg_daily_logins": 3.2,
        "typical_geo_countries": ["US"],
        "typical_login_hours": [8, 9, 10, 11, 12, 13, 14, 15, 16, 17],
        "anomaly_score": 0.0,
        "source": "risingwave_baseline_stub"
    }))
}
