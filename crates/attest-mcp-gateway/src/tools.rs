//! Internal tool implementations dispatched by the gateway.
//!
//! Phase 4a stubs: each tool returns a realistic but synthetic response so
//! the orchestrator integration tests pass without live ClickHouse / RisingWave.
//! Phase 4b will replace stubs with real queries once the Railway infra is stable.
//!
//! Phase 7: `query_warm_tier` forwards read-only SQL to the control-plane
//! `/v1/warm/query` endpoint (ClickHouse / Iceberg).

use crate::warm_limit::WarmTierLimiter;
use anyhow::Result;
use serde_json::{json, Value};

/// Dispatch a tool call to the appropriate internal implementation.
pub async fn dispatch(
    tool_id: &str,
    args: &Value,
    warm_limiter: &WarmTierLimiter,
) -> Result<Value> {
    match tool_id {
        "query_hot_tier" => query_hot_tier(args).await,
        "query_warm_tier" => query_warm_tier(args, warm_limiter).await,
        "lookup_threat_intel" => lookup_threat_intel(args).await,
        "get_asset_context" => get_asset_context(args).await,
        "get_user_baseline" => get_user_baseline(args).await,
        "analyze_code_snippet" => analyze_code_snippet(args).await,
        "sandbox_detonate" => sandbox_detonate(args).await,
        "propose_detection_pr" => propose_detection_pr(args).await,
        "request_human_review" => request_human_review(args).await,
        "idp_revoke_session" => planned_action("idp_revoke_session", args).await,
        "edr_isolate_host" => planned_action("edr_isolate_host", args).await,
        "firewall_block_ioc" => planned_action("firewall_block_ioc", args).await,
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

async fn query_warm_tier(args: &Value, warm_limiter: &WarmTierLimiter) -> Result<Value> {
    warm_limiter.acquire()?;

    let sql = args["sql"]
        .as_str()
        .ok_or_else(|| anyhow::anyhow!("query_warm_tier requires string 'sql'"))?;
    let max_rows: usize = args["limit"].as_u64().unwrap_or(500).min(10_000) as usize;

    let base =
        std::env::var("CONTROL_PLANE_URL").unwrap_or_else(|_| "http://localhost:8080".into());
    let url = format!("{}/v1/warm/query", base.trim_end_matches('/'));

    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(60))
        .build()?;

    let mut req = client.post(&url).json(&json!({ "sql": sql }));
    if std::env::var("OTEL_EXPORTER_OTLP_ENDPOINT")
        .map(|v| !v.trim().is_empty())
        .unwrap_or(false)
    {
        let mut headers = http::HeaderMap::new();
        attest_telemetry::inject_trace_headers(&mut headers);
        req = req.headers(headers);
    }

    let resp = req
        .send()
        .await
        .map_err(|e| anyhow::anyhow!("warm query HTTP error: {e}"))?;

    if !resp.status().is_success() {
        let status = resp.status();
        let body = resp.text().await.unwrap_or_default();
        anyhow::bail!("control-plane warm query {status}: {body}");
    }

    let mut v: Value = resp
        .json()
        .await
        .map_err(|e| anyhow::anyhow!("warm query JSON error: {e}"))?;

    // Enforce row cap (control-plane returns `rows` / `row_count`).
    if let Some(rows) = v.get_mut("rows").and_then(|r| r.as_array_mut()) {
        if rows.len() > max_rows {
            rows.truncate(max_rows);
        }
        v["row_count"] = json!(rows.len());
        v["truncated_to_limit"] = json!(max_rows);
    }

    v["source"] = json!("control_plane_warm_tier");
    Ok(v)
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
    let asset = args["asset"]
        .as_str()
        .or_else(|| args["asset_id"].as_str())
        .unwrap_or("(unknown)");
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

async fn analyze_code_snippet(args: &Value) -> Result<Value> {
    let snippet_id = args["snippet_id"].as_str().unwrap_or("unknown");
    Ok(json!({
        "snippet_id": snippet_id,
        "language": "unknown",
        "findings": [],
        "risk_score": 0.0,
        "source": "code_analyzer_stub"
    }))
}

async fn sandbox_detonate(args: &Value) -> Result<Value> {
    let artifact = args["artifact_hash"].as_str().unwrap_or("unknown");
    Ok(json!({
        "artifact_hash": artifact,
        "behaviour_summary": "stub: no detonation in MVP",
        "ioc_observed": [],
        "source": "sandbox_detonate_stub"
    }))
}

async fn propose_detection_pr(args: &Value) -> Result<Value> {
    Ok(json!({
        "status": "draft",
        "title": args["title"],
        "heliql": args["heliql"],
        "rationale": args["rationale"],
        "merged": false,
        "source": "detection_pr_stub"
    }))
}

async fn request_human_review(args: &Value) -> Result<Value> {
    Ok(json!({
        "queued": true,
        "reason": args["reason"],
        "source": "human_review_queue_stub"
    }))
}

/// Containment tools record a planned action; they do not call a real IdP/EDR.
async fn planned_action(tool_id: &str, args: &Value) -> Result<Value> {
    Ok(json!({
        "tool_id": tool_id,
        "planned": true,
        "executed": false,
        "args": args,
        "source": "responder_action_stub"
    }))
}
