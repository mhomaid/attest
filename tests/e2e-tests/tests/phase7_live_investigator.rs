//! Phase 7 **live** E2E — real `POST /triage` on a running orchestrator, optional workbench trace.
//!
//! This complements `crates/attest-orchestrator/tests/investigator_loop.rs`, which calls
//! `run_investigator_llm_loop` **in-process** with a scripted `ChatClient` and WireMock MCP
//! (no HTTP, no real MCP gateway, no shared attestation log).
//!
//! ## When to run
//!
//! - Set `ATTEST_E2E=1` and `ATTEST_PHASE7_LIVE=1`.
//! - Start orchestrator + LLM + MCP gateway (and control-plane for real warm queries).
//! - Start workbench-api with the **same** `ATTEST_LOG_PATH` as the orchestrator.
//!
//! ## Flakiness vs realism
//!
//! Triage only runs the investigator when the **final** verdict is `needs_investigation`.
//! That depends on the live model. This test retries several OOD-style alerts (separate
//! `case_id` per attempt). For a higher hit-rate, run the orchestrator with
//! `GUARDRAIL_MAX_RETRIES=0` so a no-tool first answer from the triager is more likely to
//! cap into `NeedsInvestigation` (still not guaranteed).
//!
//! - `ATTEST_PHASE7_LIVE_STRICT=1` — fail if no `needs_investigation` after all attempts.
//! - `PHASE7_REQUIRE_WARM_QUERY=1` — require `investigation.queried_warm_tier == true` (model must call MCP `query_warm_tier`).
//! - `PHASE7_MAX_TRIAGE_ATTEMPTS` — default `12`.
//!
//! ```bash
//! ATTEST_LOG_PATH=/tmp/attest-phase7.ndjson make run-orchestrator   # or compose
//! ATTEST_LOG_PATH=/tmp/attest-phase7.ndjson make run-workbench-api
//! make e2e-phase7-live
//! ```

use reqwest::Client;
use serde_json::{json, Value};
use std::time::{Duration, Instant};
use uuid::Uuid;

fn orchestrator_url() -> String {
    std::env::var("ORCHESTRATOR_URL").unwrap_or_else(|_| "http://localhost:4300".into())
}

fn workbench_url() -> String {
    std::env::var("WORKBENCH_API_URL").unwrap_or_else(|_| "http://localhost:4400".into())
}

fn phase7_live_enabled() -> bool {
    e2e_tests::e2e_enabled() && std::env::var("ATTEST_PHASE7_LIVE").is_ok()
}

fn live_strict() -> bool {
    std::env::var("ATTEST_PHASE7_LIVE_STRICT")
        .map(|v| v == "1" || v.eq_ignore_ascii_case("true"))
        .unwrap_or(false)
}

fn require_warm_query() -> bool {
    std::env::var("PHASE7_REQUIRE_WARM_QUERY")
        .map(|v| v == "1" || v.eq_ignore_ascii_case("true"))
        .unwrap_or(false)
}

fn max_triage_attempts() -> u32 {
    std::env::var("PHASE7_MAX_TRIAGE_ATTEMPTS")
        .ok()
        .and_then(|v| v.parse().ok())
        .filter(|&n| n > 0)
        .unwrap_or(12)
}

async fn wait_healthz(client: &Client, url: &str, path: &str, label: &str) {
    let deadline = Instant::now() + Duration::from_secs(45);
    loop {
        if Instant::now() >= deadline {
            panic!("{label} at {url}{path} did not become ready within 45s");
        }
        if let Ok(resp) = client.get(format!("{url}{path}")).send().await {
            if resp.status().is_success() {
                return;
            }
        }
        tokio::time::sleep(Duration::from_millis(300)).await;
    }
}

async fn post_triage(client: &Client, orch: &str, case_id: Uuid, alert: Value) -> Value {
    client
        .post(format!("{orch}/triage"))
        .json(&json!({ "case_id": case_id.to_string(), "alert": alert }))
        .send()
        .await
        .expect("triage POST failed")
        .error_for_status()
        .expect("triage HTTP error")
        .json::<Value>()
        .await
        .expect("triage JSON parse failed")
}

async fn get_trace(client: &Client, wb: &str, case_id: Uuid) -> Value {
    client
        .get(format!("{wb}/v1/cases/{case_id}/trace"))
        .send()
        .await
        .expect("trace GET failed")
        .error_for_status()
        .expect("trace HTTP error")
        .json::<Value>()
        .await
        .expect("trace JSON parse failed")
}

fn alert_variants() -> Vec<Value> {
    vec![
        json!({
            "event_type": "login",
            "principal": "phase7-live-a@corp.example",
            "source_ip": "185.220.101.5",
            "severity_score": 0.95,
            "entity_reputation_score": 0.9,
            "baseline_deviation": 8.0,
            "is_weekend": true,
            "hour_of_day": 3,
            "failed_attempts_last_hour": 20,
            "unique_ips_last_hour": 5,
            "bytes_sent": 50000,
            "bytes_recv": 200,
        }),
        json!({
            "event_type": "data_exfiltration",
            "principal": "phase7-live-b@corp.example",
            "source_ip": "185.220.101.100",
            "severity_score": 0.99,
            "entity_reputation_score": 0.99,
            "baseline_deviation": 12.0,
            "is_weekend": true,
            "hour_of_day": 3,
            "failed_attempts_last_hour": 50,
            "unique_ips_last_hour": 10,
            "bytes_sent": 500000,
            "bytes_recv": 100,
        }),
        json!({
            "event_type": "credential_access",
            "principal": "phase7-live-c@corp.example",
            "source_ip": "203.0.113.99",
            "severity_score": 0.88,
            "entity_reputation_score": 0.8,
            "baseline_deviation": 6.5,
            "is_weekend": false,
            "hour_of_day": 2,
            "failed_attempts_last_hour": 15,
            "unique_ips_last_hour": 4,
            "bytes_sent": 30000,
            "bytes_recv": 500,
        }),
    ]
}

async fn poll_trace_has_investigator(
    client: &Client,
    wb: &str,
    case_id: Uuid,
    timeout: Duration,
) -> Value {
    let deadline = Instant::now() + timeout;
    loop {
        let trace = get_trace(client, wb, case_id).await;
        let steps = trace["steps"].as_array().cloned().unwrap_or_default();
        let has_inv = steps
            .iter()
            .any(|s| s["kind"].as_str() == Some("investigator"));
        if has_inv {
            return trace;
        }
        if Instant::now() >= deadline {
            panic!(
                "trace never showed investigator step within {:?} — last trace: {trace}",
                timeout
            );
        }
        tokio::time::sleep(Duration::from_millis(400)).await;
    }
}

#[tokio::test]
async fn phase7_live_triage_investigator_and_trace() {
    if !phase7_live_enabled() {
        println!("SKIP: set ATTEST_E2E=1 and ATTEST_PHASE7_LIVE=1 with a running stack");
        return;
    }

    let client = Client::builder()
        .timeout(Duration::from_secs(180))
        .build()
        .unwrap();
    let orch = orchestrator_url();
    let wb = workbench_url();

    wait_healthz(&client, &orch, "/healthz", "orchestrator").await;
    wait_healthz(&client, &wb, "/healthz", "workbench-api").await;

    let attempts = max_triage_attempts();
    let alerts = alert_variants();
    let mut last_verdict = String::new();
    let mut last_body: Option<Value> = None;

    for i in 0..attempts {
        let case_id = Uuid::new_v4();
        let alert = alerts[i as usize % alerts.len()].clone();
        let resp = post_triage(&client, &orch, case_id, alert).await;
        last_body = Some(resp.clone());

        let verdict = resp["verdict"].as_str().unwrap_or("").to_string();
        last_verdict = verdict.clone();

        if verdict != "needs_investigation" {
            eprintln!(
                "phase7-live attempt {}/{}: verdict={} (want needs_investigation)",
                i + 1,
                attempts,
                verdict
            );
            continue;
        }

        let inv = resp.get("investigation").filter(|v| !v.is_null()).expect(
            "needs_investigation must include investigation summary when LLM is configured",
        );

        if let Some(err) = inv["error"].as_str() {
            panic!("investigator error: {err} — full response: {resp}");
        }

        if require_warm_query() {
            assert_eq!(
                inv["queried_warm_tier"].as_bool(),
                Some(true),
                "PHASE7_REQUIRE_WARM_QUERY=1 but investigator did not call query_warm_tier — {resp}"
            );
        }

        let trace =
            poll_trace_has_investigator(&client, &wb, case_id, Duration::from_secs(60)).await;
        let steps = trace["steps"].as_array().expect("steps array");
        let inv_steps: Vec<_> = steps
            .iter()
            .filter(|s| s["kind"].as_str() == Some("investigator"))
            .collect();
        assert!(
            !inv_steps.is_empty(),
            "trace steps missing investigator kind: {trace}"
        );
        println!(
            "PASS phase7-live: case_id={} investigation={} trace_steps={}",
            case_id,
            serde_json::to_string(inv).unwrap(),
            steps.len()
        );
        return;
    }

    let tail = last_body
        .as_ref()
        .map(|b| serde_json::to_string(b).unwrap_or_else(|_| "<json>".into()))
        .unwrap_or_else(|| "<none>".into());
    let msg = format!(
        "never observed needs_investigation after {attempts} attempts (last_verdict={last_verdict}). \
         Try GUARDRAIL_MAX_RETRIES=0 on the orchestrator or ATTEST_PHASE7_LIVE_STRICT=0 to soft-skip. \
         Last body: {tail}"
    );
    if live_strict() {
        panic!("{msg}");
    }
    eprintln!("SKIP phase7-live: {msg}");
}
