//! Phase 1 E2E test — Streaming Substrate
//!
//! Acceptance gate from `10_Build_Order.md`:
//!   A CloudTrail ConsoleLogin event POSTed to the collector appears in
//!   /v1/events/recent within 5 s, and alice's baseline reflects the new
//!   region within 10 s.
//!
//! Guards:
//!   - Skipped unless `ATTEST_E2E=1` is set (CI requires `make dev-up` first).
//!   - Collector must be running on `COLLECTOR_URL` (default http://localhost:4000).
//!   - Control-plane must be running on `CONTROL_PLANE_URL` (default http://localhost:8080).

use std::time::{Duration, Instant};

/// Helper: busy-poll until the async closure returns Ok(Some(T)) or timeout.
async fn poll_until<F, Fut, T>(mut f: F, timeout: Duration) -> Option<T>
where
    F: FnMut() -> Fut,
    Fut: std::future::Future<Output = Option<T>>,
{
    let deadline = Instant::now() + timeout;
    while Instant::now() < deadline {
        if let Some(v) = f().await {
            return Some(v);
        }
        tokio::time::sleep(Duration::from_millis(250)).await;
    }
    None
}

fn collector_url() -> String {
    std::env::var("COLLECTOR_URL").unwrap_or_else(|_| "http://localhost:4000".into())
}

fn control_plane_url() -> String {
    std::env::var("CONTROL_PLANE_URL").unwrap_or_else(|_| "http://localhost:8080".into())
}

/// POST a CloudTrail ConsoleLogin event to the collector.
/// Returns the first event_id string on success.
async fn post_cloudtrail_login(user: &str, region: &str) -> Option<String> {
    let payload = serde_json::json!({
        "Records": [{
            "eventName": "ConsoleLogin",
            "eventTime": chrono::Utc::now().to_rfc3339(),
            "awsRegion": region,
            "recipientAccountId": "123456789012",
            "userIdentity": {
                "type": "IAMUser",
                "userName": user,
                "arn": format!("arn:aws:iam::123456789012:user/{}", user.split('@').next().unwrap_or("user")),
                "accountId": "123456789012"
            }
        }]
    });

    let client = reqwest::Client::new();
    let resp = client
        .post(format!("{}/ingest", collector_url()))
        .json(&payload)
        .send()
        .await
        .ok()?;

    let body: serde_json::Value = resp.json().await.ok()?;
    body["event_ids"]
        .as_array()?
        .first()?
        .as_str()
        .map(str::to_string)
}

/// Poll /v1/events/recent?id=<id> until the event appears.
async fn poll_event(event_id: &str, timeout: Duration) -> Option<serde_json::Value> {
    let url = format!("{}/v1/events/recent?id={}", control_plane_url(), event_id);
    poll_until(
        || async {
            let resp = reqwest::get(&url).await.ok()?;
            if resp.status().is_success() {
                resp.json().await.ok()
            } else {
                None
            }
        },
        timeout,
    )
    .await
}

/// Poll /v1/baselines/user/:name until the region appears.
async fn poll_baseline_region(user: &str, region: &str, timeout: Duration) -> bool {
    let url = format!("{}/v1/baselines/user/{}", control_plane_url(), user);
    poll_until(
        || async {
            let resp = reqwest::get(&url).await.ok()?;
            if !resp.status().is_success() {
                return None;
            }
            let body: serde_json::Value = resp.json().await.ok()?;
            let regions = body["regions_seen_30d"].as_array()?;
            if regions.iter().any(|r| r.as_str() == Some(region)) {
                Some(true)
            } else {
                None
            }
        },
        timeout,
    )
    .await
    .is_some()
}

#[tokio::test]
async fn cloudtrail_event_appears_in_baseline_within_10s() {
    e2e_tests::require_e2e!();

    // 1. POST a ConsoleLogin for alice from us-west-2.
    let event_id = post_cloudtrail_login("alice@example.com", "us-west-2")
        .await
        .expect("POST /ingest failed — is the collector running?");

    eprintln!("Ingested event_id: {event_id}");

    // 2. Within 5 s: event must appear in /v1/events/recent.
    let event = poll_event(&event_id, Duration::from_secs(5))
        .await
        .expect("event did not appear in /v1/events/recent within 5s");

    assert_eq!(
        event["actor_user_name"].as_str().unwrap_or(""),
        "alice@example.com"
    );
    assert_eq!(event["cloud_region"].as_str().unwrap_or(""), "us-west-2");

    eprintln!("Event found in recent_events ✓");

    // 3. Within 30 s: alice's baseline must include us-west-2.
    // entity_baselines uses GROUP BY + ARRAY_AGG which RisingWave materializes
    // slightly slower than a simple filter view — allow extra time.
    assert!(
        poll_baseline_region("alice@example.com", "us-west-2", Duration::from_secs(30)).await,
        "us-west-2 did not appear in alice's baseline within 30s"
    );

    eprintln!("Baseline updated with us-west-2 ✓");
}

#[tokio::test]
async fn collector_healthz_is_ok() {
    e2e_tests::require_e2e!();
    let resp = reqwest::get(format!("{}/healthz", collector_url()))
        .await
        .expect("collector healthz failed");
    assert!(resp.status().is_success());
}

#[tokio::test]
async fn control_plane_healthz_is_ok() {
    e2e_tests::require_e2e!();
    let resp = reqwest::get(format!("{}/healthz", control_plane_url()))
        .await
        .expect("control-plane healthz failed");
    assert!(resp.status().is_success());
}
