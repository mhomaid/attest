//! Phase 4a E2E tests — Hybrid Triager classifier path.
//!
//! These tests call a live orchestrator running at `ORCHESTRATOR_URL`
//! (default http://localhost:4300). The orchestrator must be started with
//! trained ONNX artifacts before these tests run.
//!
//! Run via: `make e2e-phase4a`

// attest_attestation types used in future assertion extensions (Phase 4b)
use reqwest::Client;
use serde_json::{json, Value};
use std::time::{Duration, Instant};
use uuid::Uuid;

fn orchestrator_url() -> String {
    std::env::var("ORCHESTRATOR_URL").unwrap_or_else(|_| "http://localhost:4300".into())
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

// ─── Test 1 ──────────────────────────────────────────────────────────────────

/// A well-known brute-force pattern that is in-distribution and high-confidence
/// must be handled by the Classifier path in under 500ms (network budget; the
/// pure classifier is <50ms but over HTTP we allow 500ms for CI).
#[tokio::test]
async fn triager_classifier_path_produces_attested_verdict() {
    e2e_tests::require_e2e!();
    let client = Client::new();
    let url = orchestrator_url();
    wait_for_orchestrator(&client, &url).await;

    // Known brute-force pattern: high severity, high entity reputation, high baseline deviation
    let alert = json!({
        "severity_score": 0.80,
        "source_class_id": 3002.0,
        "entity_reputation_score": 0.70,
        "baseline_deviation": 3.5,
        "threat_intel_hit_count": 2.0,
        "hour_of_day": 3.0,
        "asset_criticality": 0.50,
        "prior_disposition_ratio": 0.85,
        "case_class": "brute_force"
    });

    let t0 = Instant::now();
    let resp = post_triage(&client, &url, alert).await;
    let elapsed = t0.elapsed();

    // 1. Must respond within 500ms over HTTP
    assert!(
        elapsed < Duration::from_millis(500),
        "triage over HTTP took {:?} — must be < 500ms",
        elapsed
    );

    // 2. Must be handled by the classifier path (not escalated)
    let execution_path = resp["execution_path"].as_str().unwrap_or("");
    assert_eq!(
        execution_path, "classifier",
        "expected classifier path for known pattern; got execution_path={execution_path}\nfull response: {resp}"
    );

    // 3. Must not be marked as escalated
    assert!(
        !resp["escalated"].as_bool().unwrap_or(true),
        "known pattern should not escalate; response: {resp}"
    );

    // 4. Calibrated confidence must be a valid 0–1 float
    let confidence = resp["calibrated_confidence"].as_f64().unwrap_or(-1.0);
    assert!(
        (0.0..=1.0).contains(&confidence),
        "calibrated_confidence out of range: {confidence}"
    );

    // 5. action_id must be a valid UUID
    let action_id = resp["action_id"].as_str().unwrap_or("");
    assert!(
        Uuid::parse_str(action_id).is_ok(),
        "action_id is not a valid UUID: {action_id}"
    );

    println!(
        "PASS: classifier path, confidence={:.3}, latency={:?}",
        confidence, elapsed
    );
}

// ─── Test 2 ──────────────────────────────────────────────────────────────────

/// A deliberately out-of-distribution alert (unknown source class, midpoint features)
/// must be escalated to the LLM stub path (EscalatedStub / Hybrid).
#[tokio::test]
async fn triager_classifier_escalates_when_out_of_distribution() {
    e2e_tests::require_e2e!();
    let client = Client::new();
    let url = orchestrator_url();
    wait_for_orchestrator(&client, &url).await;

    // OOD pattern: unknown source class, all features near midpoint — novel to the classifier
    let alert = json!({
        "severity_score": 0.50,
        "source_class_id": 9999.0,   // unknown class — highly OOD
        "entity_reputation_score": 0.50,
        "baseline_deviation": 0.30,
        "threat_intel_hit_count": 0.0,
        "hour_of_day": 12.0,
        "asset_criticality": 0.50,
        "prior_disposition_ratio": 0.50,
        "case_class": "unknown"
    });

    let resp = post_triage(&client, &url, alert).await;

    // OOD cases must be escalated (either "hybrid" path or explicit escalated=true)
    let execution_path = resp["execution_path"].as_str().unwrap_or("");
    let escalated = resp["escalated"].as_bool().unwrap_or(false);

    assert!(
        execution_path == "hybrid" || escalated,
        "OOD alert should escalate; got execution_path={execution_path}, escalated={escalated}\nfull response: {resp}"
    );

    // Verdict must be EscalatedStub for Phase 4a
    let verdict = resp["verdict"].as_str().unwrap_or("");
    assert_eq!(
        verdict, "escalated_stub",
        "OOD alert in Phase 4a must produce escalated_stub verdict; got {verdict}"
    );

    // Novelty score must be present and > 0
    let novelty = resp["novelty_score"].as_f64().unwrap_or(0.0);
    assert!(
        novelty > 0.0,
        "OOD alert should have a positive novelty score; got {novelty}"
    );

    println!(
        "PASS: OOD alert escalated, novelty_score={:.3}, verdict={verdict}",
        novelty
    );
}

// ─── Test 3 ──────────────────────────────────────────────────────────────────

/// Latency test: 10 sequential known-pattern requests, P99 must be < 200ms over HTTP.
#[tokio::test]
async fn triager_classifier_p99_latency_under_200ms() {
    e2e_tests::require_e2e!();
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

    let n = 10;
    let mut latencies_ms = Vec::with_capacity(n);

    for _ in 0..n {
        let t0 = Instant::now();
        let resp = post_triage(&client, &url, alert.clone()).await;
        let lat = t0.elapsed().as_millis() as u64;
        // Only classifier-path responses count toward the latency budget
        if resp["execution_path"].as_str() == Some("classifier") {
            latencies_ms.push(lat);
        }
    }

    assert!(
        !latencies_ms.is_empty(),
        "no classifier-path responses out of {n}; the benign alert should not escalate"
    );
    latencies_ms.sort_unstable();
    let m = latencies_ms.len();
    let p99 = latencies_ms[((m as f64 * 0.99) as usize).min(m - 1)];
    println!("Classifier P99 latency over HTTP: {p99}ms (n={m})");

    assert!(
        p99 < 200,
        "classifier P99 latency {p99}ms > 200ms budget (HTTP overhead included)"
    );
}
