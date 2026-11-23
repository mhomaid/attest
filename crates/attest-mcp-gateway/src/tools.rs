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
        "ip_asn_lookup" => ip_asn_lookup(args).await,
        "get_asset_context" => get_asset_context(args).await,
        "get_user_baseline" => get_user_baseline(args).await,
        "entity_lookup" => entity_lookup(args).await,
        "analyze_code_snippet" => analyze_code_snippet(args).await,
        "sandbox_detonate" => sandbox_detonate(args).await,
        "propose_detection_pr" => propose_detection_pr(args).await,
        "request_human_review" => request_human_review(args).await,
        "idp_revoke_session" => planned_action("idp_revoke_session", args).await,
        "edr_isolate_host" => planned_action("edr_isolate_host", args).await,
        "firewall_block_ioc" => planned_action("firewall_block_ioc", args).await,
        "graylog_search" => graylog_search(args).await,
        other => anyhow::bail!("unknown tool: {other}"),
    }
}

async fn graylog_search(args: &Value) -> Result<Value> {
    let query = crate::graylog_search::SearchQuery {
        query: args["query"].as_str().unwrap_or("").to_string(),
        from_unix: args["from"].as_i64().unwrap_or(0),
        to_unix: args["to"].as_i64().unwrap_or(0),
        limit: args["limit"].as_u64().unwrap_or(20),
    };
    crate::graylog_search::validate(&query).map_err(|e| anyhow::anyhow!(e))?;
    let url = std::env::var("ATTEST_GRAYLOG_URL")
        .map_err(|_| anyhow::anyhow!("graylog search is not configured"))?;
    let token = std::env::var("ATTEST_GRAYLOG_TOKEN").unwrap_or_default();
    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(5))
        .build()
        .map_err(|e| anyhow::anyhow!(e))?;
    let response = client
        .get(format!(
            "{url}/api/search/universal/absolute?query={}&limit={}",
            urlencoding_query(&query.query),
            query.limit
        ))
        .header("X-Requested-By", "attest")
        .header("Authorization", format!("Bearer {token}"))
        .send()
        .await
        .map_err(|e| anyhow::anyhow!("graylog search failed: {e}"))?;
    let status = response.status().as_u16();
    let body = response.text().await.unwrap_or_default();
    crate::graylog_search::search(
        &query,
        crate::graylog_search::UpstreamReply { status, body },
    )
    .map_err(|e| anyhow::anyhow!(e))
}

fn urlencoding_query(query: &str) -> String {
    let mut out = String::new();
    for b in query.bytes() {
        match b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b':' => {
                out.push(b as char)
            }
            _ => out.push_str(&format!("%{b:02X}")),
        }
    }
    out
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
    if let Some(value) =
        attest_service_auth::authorization_from_env(attest_service_auth::AUDIENCE_CONTROL_PLANE)
    {
        req = req.header(http::header::AUTHORIZATION, value);
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
    let indicator = args
        .get("indicator")
        .and_then(|v| v.as_str())
        .ok_or_else(|| anyhow::anyhow!("lookup_threat_intel requires string 'indicator'"))?;
    let indicator_type = args.get("indicator_type").and_then(|v| v.as_str());
    let path = env_path("ATTEST_THREAT_INTEL_FEED");
    crate::threat_intel::respond(
        indicator,
        indicator_type,
        path.as_deref(),
        crate::demo::enabled(),
    )
    .map_err(|e| anyhow::anyhow!(e))
}

async fn ip_asn_lookup(args: &Value) -> Result<Value> {
    let ip = args
        .get("ip")
        .and_then(|v| v.as_str())
        .ok_or_else(|| anyhow::anyhow!("ip_asn_lookup requires string 'ip'"))?;
    let path = env_path("ATTEST_ASN_DB");
    crate::asn_lookup::respond(ip, path.as_deref(), crate::demo::enabled())
        .map_err(|e| anyhow::anyhow!(e))
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
    let principal = args
        .get("principal")
        .and_then(|v| v.as_str())
        .ok_or_else(|| anyhow::anyhow!("get_user_baseline requires string 'principal'"))?;
    let path = env_path("ATTEST_USER_BASELINES");
    crate::user_baseline::respond(principal, path.as_deref(), crate::demo::enabled())
        .map_err(|e| anyhow::anyhow!(e))
}

async fn entity_lookup(args: &Value) -> Result<Value> {
    if args.get("sql").is_some() || args.get("statement").is_some() {
        anyhow::bail!("entity_lookup does not accept free-form sql");
    }
    let Some(path) = env_path("ATTEST_ENTITY_QUERIES") else {
        if crate::demo::enabled() {
            return Ok(json!({"rows": [], "row_count": 0, "source": "entity_lookup_stub"}));
        }
        anyhow::bail!("entity queries are not configured");
    };
    let text = std::fs::read_to_string(&path)
        .map_err(|e| anyhow::anyhow!("entity queries unreadable: {e}"))?;
    let catalog = crate::entity_lookup::Catalog::parse(&text).map_err(|e| anyhow::anyhow!(e))?;
    let Some(url) = std::env::var("ATTEST_ENTITY_DATABASE_URL")
        .ok()
        .filter(|v| !v.trim().is_empty())
    else {
        if crate::demo::enabled() {
            return Ok(json!({"rows": [], "row_count": 0, "source": "entity_lookup_stub"}));
        }
        anyhow::bail!("entity database is not configured");
    };
    match catalog.dialect() {
        crate::entity_lookup::Dialect::Postgres => {
            crate::entity_lookup::execute(&catalog, args, |bound| {
                crate::entity_lookup::query_postgres(url.clone(), bound)
            })
            .await
            .map_err(|e| anyhow::anyhow!(e))
        }
        crate::entity_lookup::Dialect::Mysql => {
            let bound = catalog.bind(args).map_err(|e| anyhow::anyhow!(e))?;
            let name = bound.name.clone();
            let columns = bound.columns.clone();
            let row_cap = bound.row_cap;
            match crate::entity_lookup::query_mysql(url, bound).await {
                Ok(rows) => Ok(crate::entity_lookup::project(
                    &name, &columns, row_cap, rows,
                )),
                Err(e) => Err(anyhow::anyhow!(e)),
            }
        }
    }
}

fn env_path(key: &str) -> Option<std::path::PathBuf> {
    std::env::var(key)
        .ok()
        .filter(|v| !v.trim().is_empty())
        .map(std::path::PathBuf::from)
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

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::{json, Value};

    static ENV_LOCK: std::sync::LazyLock<tokio::sync::Mutex<()>> =
        std::sync::LazyLock::new(|| tokio::sync::Mutex::new(()));

    fn assert_not_stub(value: &Value) {
        let source = value["source"].as_str().expect("source");
        assert!(
            !crate::demo::is_stub_source(source),
            "source {source} ends with _stub"
        );
    }

    #[tokio::test]
    async fn production_mode_enrichment_sources_are_not_stubs() {
        let _guard = ENV_LOCK.lock().await;
        let _clear = ClearEnrichEnv;
        let dir = std::env::temp_dir().join(format!("attest-enrich-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let asn = dir.join("asn.json");
        let feed = dir.join("feed.json");
        let baselines = dir.join("baselines.json");
        std::fs::write(
            &asn,
            r#"{"networks":[{"cidr":"203.0.113.0/24","asn":64500,"organization":"Example Net"}]}"#,
        )
        .unwrap();
        std::fs::write(
            &feed,
            r#"{"synced_at":"2026-01-15T00:00:00Z","threatfox":{"data":[{"ioc":"203.0.113.10","ioc_type":"ip","malware":"synthetic-loader","confidence_level":50}]}}"#,
        )
        .unwrap();
        std::fs::write(
            &baselines,
            r#"[{"principal":"user-1001","country":"DE","hour":22,"logins":3}]"#,
        )
        .unwrap();
        std::env::remove_var("ATTEST_DEMO_MODE");
        std::env::set_var("ATTEST_ASN_DB", &asn);
        std::env::set_var("ATTEST_THREAT_INTEL_FEED", &feed);
        std::env::set_var("ATTEST_USER_BASELINES", &baselines);
        std::env::remove_var("ATTEST_ENTITY_QUERIES");
        std::env::remove_var("ATTEST_ENTITY_DATABASE_URL");

        let limiter = crate::warm_limit::WarmTierLimiter::from_env();
        let asn_hit = dispatch(
            "ip_asn_lookup",
            &json!({"ip": "203.0.113.10", "path": "/etc/passwd"}),
            &limiter,
        )
        .await
        .unwrap();
        assert_eq!(asn_hit["asn"], 64500);
        assert_eq!(asn_hit["organization"], "Example Net");
        assert_not_stub(&asn_hit);

        let intel = dispatch(
            "lookup_threat_intel",
            &json!({"indicator": "203.0.113.10", "indicator_type": "ip"}),
            &limiter,
        )
        .await
        .unwrap();
        assert_eq!(intel["last_sync"], "2026-01-15T00:00:00Z");
        assert_eq!(intel["malicious"], true);
        assert_not_stub(&intel);

        let baseline = dispatch(
            "get_user_baseline",
            &json!({"principal": "user-1001", "typical_geo_countries": ["US"]}),
            &limiter,
        )
        .await
        .unwrap();
        assert_eq!(baseline["typical_geo_countries"], json!(["DE"]));
        assert_eq!(baseline["typical_login_hours"], json!([22]));
        assert_not_stub(&baseline);

        let missing = dispatch("entity_lookup", &json!({"query": "store_owner"}), &limiter)
            .await
            .unwrap_err()
            .to_string();
        assert!(!missing.contains("_stub"), "{missing}");
    }

    struct ClearEnrichEnv;

    impl Drop for ClearEnrichEnv {
        fn drop(&mut self) {
            for key in [
                "ATTEST_DEMO_MODE",
                "ATTEST_ASN_DB",
                "ATTEST_THREAT_INTEL_FEED",
                "ATTEST_USER_BASELINES",
                "ATTEST_ENTITY_QUERIES",
                "ATTEST_ENTITY_DATABASE_URL",
            ] {
                std::env::remove_var(key);
            }
        }
    }
}
