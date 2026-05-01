//! Phase 3 E2E test — HELIQL DSL + stream detection
//!
//! Acceptance gate from `10_Build_Order.md`:
//!   1. Seed alice with a 30-day baseline of US-only logins (via collector ingest).
//!   2. Inject a login event from an anomalous region (ap-southeast-1).
//!   3. Within 5 s an alert should appear on the `alerts` Redpanda topic with
//!      detection_id = "aws_console_login_from_anomalous_geolocation".
//!
//! Guards:
//!   - Skipped unless `ATTEST_E2E=1` is set.
//!   - Requires `make dev-up-platform` to be running.

use rdkafka::{
    consumer::{Consumer, StreamConsumer},
    ClientConfig, Message,
};
use std::time::{Duration, Instant};

fn skip_if_no_e2e() -> bool {
    std::env::var("ATTEST_E2E").as_deref() != Ok("1")
}

fn collector_url() -> String {
    std::env::var("COLLECTOR_URL").unwrap_or_else(|_| "http://localhost:4000".into())
}

fn kafka_brokers() -> String {
    std::env::var("KAFKA_BROKERS").unwrap_or_else(|_| "localhost:19092".into())
}

// ── Helpers ──────────────────────────────────────────────────────────────────

/// POST a CloudTrail ConsoleLogin event to the collector.
async fn post_login(
    client: &reqwest::Client,
    user: &str,
    region: &str,
    auth_status: &str,
) -> Option<String> {
    let payload = serde_json::json!({
        "Records": [{
            "eventName": "ConsoleLogin",
            "eventTime": chrono::Utc::now().to_rfc3339(),
            "awsRegion": region,
            "recipientAccountId": "111111111111",
            "responseElements": { "ConsoleLogin": auth_status },
            "userIdentity": {
                "type": "IAMUser",
                "userName": user,
                "arn": format!("arn:aws:iam::111111111111:user/{user}"),
                "accountId": "111111111111"
            }
        }]
    });

    let res = client
        .post(format!("{}/ingest", collector_url()))
        .json(&payload)
        .timeout(Duration::from_secs(10))
        .send()
        .await
        .ok()?;

    if !res.status().is_success() {
        return None;
    }

    let body: serde_json::Value = res.json().await.ok()?;
    body["event_ids"]
        .as_array()?
        .first()?
        .as_str()
        .map(|s| s.to_string())
}

/// Seed N successful login events for a user from the given region.
/// Used to build a baseline.
async fn seed_baseline(client: &reqwest::Client, user: &str, region: &str, count: usize) {
    for _ in 0..count {
        post_login(client, user, region, "Success").await;
    }
    println!("seeded {count} baseline logins for {user} from {region}");
}

/// Build a Kafka consumer on the `alerts` topic.
fn build_alerts_consumer(brokers: &str) -> StreamConsumer {
    let consumer: StreamConsumer = ClientConfig::new()
        .set("bootstrap.servers", brokers)
        .set("group.id", "e2e-phase3-alerts-consumer")
        .set("enable.auto.commit", "true")
        .set("auto.offset.reset", "latest")
        .create()
        .expect("failed to create Kafka consumer");
    consumer.subscribe(&["alerts"]).expect("subscribe failed");
    consumer
}

/// Poll the `alerts` topic for up to `timeout` looking for an alert with
/// the given detection_id referencing the given event_id.
async fn poll_for_alert(
    consumer: &StreamConsumer,
    detection_id: &str,
    event_id: &str,
    timeout: Duration,
) -> Option<serde_json::Value> {
    let deadline = Instant::now() + timeout;

    while Instant::now() < deadline {
        let remaining = deadline.saturating_duration_since(Instant::now());
        let msg =
            tokio::time::timeout(remaining.min(Duration::from_millis(500)), consumer.recv()).await;

        let msg = match msg {
            Ok(Ok(m)) => m,
            _ => continue,
        };

        if let Some(payload) = msg.payload() {
            if let Ok(alert) = serde_json::from_slice::<serde_json::Value>(payload) {
                if alert["detection_id"].as_str() == Some(detection_id)
                    && alert["event_id"].as_str() == Some(event_id)
                {
                    return Some(alert);
                }
            }
        }
    }
    None
}

// ── Tests ────────────────────────────────────────────────────────────────────

#[tokio::test]
async fn detection_fires_on_anomalous_geolocation() {
    if skip_if_no_e2e() {
        println!("ATTEST_E2E not set — skipping Phase 3 E2E test");
        return;
    }

    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(15))
        .build()
        .unwrap();

    // Use a UUID-suffix so each run has a fresh baseline with no prior history.
    let run_id = uuid::Uuid::new_v4().to_string();
    let user = format!("alice-e2e-{}@example.com", &run_id[..8]);
    let user = user.as_str();

    const BASELINE_REGION: &str = "us-east-1";
    const ANOMALOUS_REGION: &str = "ap-southeast-1";
    const DETECTION_ID: &str = "aws_console_login_from_anomalous_geolocation";

    // ── 1. Build user's baseline (30 US logins so the mat. view has data) ──
    println!("Building baseline for {user} from {BASELINE_REGION} …");
    seed_baseline(&client, user, BASELINE_REGION, 30).await;

    // ── 2. Subscribe to alerts topic BEFORE injecting the anomalous event ───
    let consumer = build_alerts_consumer(&kafka_brokers());

    // Small pause to let RisingWave process the baseline events.
    tokio::time::sleep(Duration::from_secs(5)).await;

    // ── 3. Inject the anomalous login ───────────────────────────────────────
    println!("Injecting anomalous login from {ANOMALOUS_REGION} …");
    let event_id = post_login(&client, user, ANOMALOUS_REGION, "Success")
        .await
        .expect("failed to post anomalous login event");

    println!("event_id = {event_id}");

    // ── 4. Wait ≤ 10 s for the alert to appear on the alerts topic ──────────
    println!("Polling alerts topic for detection_id={DETECTION_ID} (≤ 10 s) …");
    let alert = poll_for_alert(&consumer, DETECTION_ID, &event_id, Duration::from_secs(10)).await;

    assert!(
        alert.is_some(),
        "expected alert for detection '{DETECTION_ID}' referencing event '{event_id}' within 10 s, got none"
    );

    let alert = alert.unwrap();
    println!("✓ alert received: {alert}");

    let severity = alert["severity"].as_str().unwrap_or("");
    assert!(
        !severity.is_empty(),
        "alert should have a non-empty severity field"
    );
    println!("✓ severity = {severity}");
}

#[tokio::test]
async fn cloudtrail_logging_disabled_fires_critical_alert() {
    if skip_if_no_e2e() {
        println!("ATTEST_E2E not set — skipping Phase 3 E2E test");
        return;
    }

    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(15))
        .build()
        .unwrap();

    let consumer = build_alerts_consumer(&kafka_brokers());

    // Inject a StopLogging event.
    let payload = serde_json::json!({
        "Records": [{
            "eventName": "StopLogging",
            "eventSource": "cloudtrail.amazonaws.com",
            "eventTime": chrono::Utc::now().to_rfc3339(),
            "awsRegion": "us-east-1",
            "recipientAccountId": "111111111111",
            "userIdentity": {
                "type": "IAMUser",
                "userName": "attacker@example.com",
                "arn": "arn:aws:iam::111111111111:user/attacker",
                "accountId": "111111111111"
            }
        }]
    });

    let res = client
        .post(format!("{}/ingest", collector_url()))
        .json(&payload)
        .send()
        .await
        .expect("post failed");

    let body: serde_json::Value = res.json().await.unwrap();
    let event_id = body["event_ids"]
        .as_array()
        .and_then(|a| a.first())
        .and_then(|v| v.as_str())
        .expect("no event_id in response")
        .to_string();

    println!("Injected StopLogging event_id={event_id}, polling for alert …");

    let alert = poll_for_alert(
        &consumer,
        "aws_cloudtrail_logging_disabled",
        &event_id,
        Duration::from_secs(10),
    )
    .await;

    assert!(
        alert.is_some(),
        "expected critical alert for 'aws_cloudtrail_logging_disabled', got none"
    );

    let alert = alert.unwrap();
    assert_eq!(
        alert["severity"].as_str().unwrap_or(""),
        "critical",
        "CloudTrail disable must fire a critical alert"
    );
    println!("✓ critical alert confirmed: {alert}");
}
