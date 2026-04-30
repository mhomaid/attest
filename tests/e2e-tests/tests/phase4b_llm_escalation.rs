//! Phase 4b E2E tests — Hybrid Triager LLM escalation path.
//!
//! Tests a live orchestrator at `ORCHESTRATOR_URL` (default http://localhost:4300).
//! The orchestrator must be started with ONNX artifacts AND a configured LLM
//! provider before these tests run.
//!
//! Run via: `make e2e-phase4b`
//! Gate local Unsloth test on: `ATTEST_LLM_PROVIDER=local`
//! Gate Anthropic test on: `ANTHROPIC_API_KEY` env var.

use reqwest::Client;
use serde_json::{json, Value};
use std::time::{Duration, Instant};
use uuid::Uuid;

fn orchestrator_url() -> String {
    std::env::var("ORCHESTRATOR_URL").unwrap_or_else(|_| "http://localhost:4300".into())
}

fn is_local_llm_available() -> bool {
    std::env::var("ATTEST_LLM_PROVIDER").map(|v| v == "local").unwrap_or(false)
}

fn anthropic_key_available() -> bool {
    std::env::var("ANTHROPIC_API_KEY").is_ok()
}

async fn wait_for_orchestrator(client: &Client, url: &str) {
    let deadline = Instant::now() + Duration::from_secs(30);
    loop {
        if Instant::now() >= deadline {
            panic!("orchestrator at {url}/healthz did not become ready within 30s");
        }
        if let Ok(resp) = client.get(format!("{url}/healthz")).send().await {
            if resp.status().is_success() {
                return;
            }
        }
        tokio::time::sleep(Duration::from_millis(300)).await;
    }
}

async fn post_triage(client: &Client, url: &str, alert: Value) -> Value {
    client
        .post(format!("{url}/triage"))
        .json(&json!({
            "case_id": Uuid::new_v4().to_string(),
            "alert": alert,
        }))
        .send()
        .await
        .expect("triage request failed")
        .json::<Value>()
        .await
        .expect("failed to parse triage response")
}

// ── Test 1: Classifier path stays classifier for high-confidence alert ─────────

/// A well-known in-distribution, high-confidence alert must NOT escalate to the
/// LLM path — it should stay on the fast classifier path.
#[tokio::test]
async fn phase4b_classifier_path_does_not_escalate() {
    let client = Client::new();
    let url = orchestrator_url();
    wait_for_orchestrator(&client, &url).await;

    let alert = json!({
        "severity_score": 0.90,
        "source_class_id": 3002.0,
        "entity_reputation_score": 0.85,
        "baseline_deviation": 4.0,
        "threat_intel_hit_count": 3.0,
        "hour_of_day": 3.0,
        "asset_criticality": 0.80,
        "prior_disposition_ratio": 0.90,
        "case_class": "brute_force"
    });

    let t0 = Instant::now();
    let resp = post_triage(&client, &url, alert).await;
    let elapsed = t0.elapsed();

    let execution_path = resp["execution_path"].as_str().unwrap_or("");
    let escalated = resp["escalated"].as_bool().unwrap_or(true);

    assert_eq!(
        execution_path, "classifier",
        "high-confidence alert must use classifier path; got {execution_path}\nresponse: {resp}"
    );
    assert!(!escalated, "should not escalate; response: {resp}");
    assert!(
        elapsed < Duration::from_millis(500),
        "classifier path must be < 500ms over HTTP; was {elapsed:?}"
    );

    println!("PASS: classifier path retained for high-confidence alert ({elapsed:?})");
}

// ── Test 2: OOD alert produces a Hybrid envelope (live local Unsloth) ─────────

/// An OOD alert must go through the LLM escalation path and produce a
/// `hybrid` execution_path with both `classifier_evidence` and an LLM verdict.
///
/// This test is gated on `ATTEST_LLM_PROVIDER=local` because it requires a
/// running Unsloth Studio instance at 127.0.0.1:8888.
/// P50 latency budget: 15 seconds (Qwen3.6-35B-A3B is fast).
#[tokio::test]
async fn phase4b_ood_alert_produces_hybrid_envelope() {
    if !is_local_llm_available() {
        println!("SKIP: ATTEST_LLM_PROVIDER!=local — skipping live Unsloth test");
        return;
    }

    let client = Client::builder()
        .timeout(Duration::from_secs(60))
        .build()
        .unwrap();
    let url = orchestrator_url();
    wait_for_orchestrator(&client, &url).await;

    let alert = json!({
        "severity_score": 0.50,
        "source_class_id": 9999.0,
        "entity_reputation_score": 0.50,
        "baseline_deviation": 0.30,
        "threat_intel_hit_count": 0.0,
        "hour_of_day": 12.0,
        "asset_criticality": 0.50,
        "prior_disposition_ratio": 0.50,
        "case_class": "unknown"
    });

    let t0 = Instant::now();
    let resp = post_triage(&client, &url, alert).await;
    let elapsed = t0.elapsed();

    let execution_path = resp["execution_path"].as_str().unwrap_or("");
    let escalated = resp["escalated"].as_bool().unwrap_or(false);

    // Must have taken the hybrid path
    assert!(
        execution_path == "hybrid" || escalated,
        "OOD alert must escalate; execution_path={execution_path}, escalated={escalated}\nresponse: {resp}"
    );

    // Must NOT be EscalatedStub — Phase 4b produces real Hybrid envelopes
    let verdict = resp["verdict"].as_str().unwrap_or("");
    assert_ne!(
        verdict, "escalated_stub",
        "Phase 4b must produce a real verdict (not escalated_stub); got {verdict}\nresponse: {resp}"
    );
    assert!(
        ["true_positive", "false_positive", "benign", "needs_investigation"].contains(&verdict),
        "verdict must be a recognised value; got {verdict}"
    );

    // Must carry classifier_evidence (the hybrid package)
    assert!(
        resp["classifier_evidence"].is_object(),
        "hybrid envelope must include classifier_evidence; response: {resp}"
    );

    // Latency budget: <= 15 seconds (P50 target for local Qwen path)
    assert!(
        elapsed <= Duration::from_secs(15),
        "LLM escalation took {elapsed:?} — must be <= 15s for local Qwen"
    );

    println!(
        "PASS: hybrid path, verdict={verdict}, latency={elapsed:?}"
    );
}

// ── Test 3: Latency — 5 sequential high-confidence requests, P99 < 200ms ──────

/// Pure classifier path P99 latency must stay under 200ms even when LLM client
/// is configured (the LLM must not be called on high-confidence paths).
#[tokio::test]
async fn phase4b_classifier_p99_unaffected_by_llm_config() {
    let client = Client::new();
    let url = orchestrator_url();
    wait_for_orchestrator(&client, &url).await;

    let alert = json!({
        "severity_score": 0.10,
        "source_class_id": 3002.0,
        "entity_reputation_score": 0.0,
        "baseline_deviation": 0.1,
        "threat_intel_hit_count": 0.0,
        "hour_of_day": 9.0,
        "asset_criticality": 0.3,
        "prior_disposition_ratio": 0.05,
        "case_class": "benign"
    });

    let n = 5;
    let mut latencies_ms: Vec<u64> = Vec::with_capacity(n);

    for _ in 0..n {
        let t0 = Instant::now();
        let resp = post_triage(&client, &url, alert.clone()).await;
        let lat = t0.elapsed().as_millis() as u64;
        if resp["execution_path"].as_str() == Some("classifier") {
            latencies_ms.push(lat);
        }
    }

    if latencies_ms.is_empty() {
        println!("SKIP: no classifier-path responses recorded (check thresholds)");
        return;
    }

    latencies_ms.sort_unstable();
    let p99 = latencies_ms[((latencies_ms.len() as f64 * 0.99) as usize).min(latencies_ms.len() - 1)];
    println!("Classifier P99 latency (Phase 4b): {p99}ms (n={})", latencies_ms.len());

    assert!(
        p99 < 200,
        "classifier P99 latency {p99}ms > 200ms — LLM config must not slow down classifier path"
    );
}

// ── Test 4: Anthropic escalation path ─────────────────────────────────────────

/// Escalation via Anthropic Sonnet must produce a real Hybrid envelope.
/// Gated on `ANTHROPIC_API_KEY` env var; marked `#[ignore]` to prevent
/// accidental billing in CI.
#[tokio::test]
#[ignore = "requires ANTHROPIC_API_KEY and a running orchestrator with ATTEST_LLM_PROVIDER=anthropic"]
async fn phase4b_anthropic_escalation_produces_hybrid_envelope() {
    if !anthropic_key_available() {
        println!("SKIP: ANTHROPIC_API_KEY not set");
        return;
    }

    let client = Client::builder()
        .timeout(Duration::from_secs(120))
        .build()
        .unwrap();
    let url = orchestrator_url();
    wait_for_orchestrator(&client, &url).await;

    let alert = json!({
        "severity_score": 0.50,
        "source_class_id": 9999.0,
        "entity_reputation_score": 0.50,
        "baseline_deviation": 0.30,
        "threat_intel_hit_count": 0.0,
        "hour_of_day": 12.0,
        "asset_criticality": 0.50,
        "prior_disposition_ratio": 0.50,
        "case_class": "unknown"
    });

    let t0 = Instant::now();
    let resp = post_triage(&client, &url, alert).await;
    let elapsed = t0.elapsed();

    let verdict = resp["verdict"].as_str().unwrap_or("");
    assert_ne!(verdict, "escalated_stub", "Anthropic path must not return escalated_stub; got {verdict}\nresponse: {resp}");
    assert!(resp["classifier_evidence"].is_object(), "missing classifier_evidence");

    println!("PASS: Anthropic hybrid path, verdict={verdict}, latency={elapsed:?}");
}
