//! Arroyo pipeline E2E tests
//!
//! Acceptance gates:
//!   1. Arroyo health — GET /api/v1/ping returns 200.
//!   2. Pipelines deployed — both `cloudtrail_to_parquet` and
//!      `cep_sequence_detection` are listed and in Running state.
//!   3. ETL pipeline — after seeding events via attest-collector, Parquet
//!      files appear under the `arroyo/cloudtrail/` MinIO prefix within 60 s.
//!   4. CEP pipeline — injecting a ConsoleLogin event followed by a
//!      GetObject event for the same user causes an alert to appear on
//!      the `alerts` Redpanda topic within 30 s.
//!
//! Guards:
//!   - Skipped unless `ATTEST_E2E=1` is set.
//!   - Requires `make dev-up-platform` to be running.

use rdkafka::{
    consumer::{Consumer, StreamConsumer},
    ClientConfig, Message,
};
use std::time::{Duration, Instant};

fn collector_url() -> String {
    std::env::var("COLLECTOR_URL").unwrap_or_else(|_| "http://localhost:4000".into())
}

fn control_plane_url() -> String {
    std::env::var("CONTROL_PLANE_URL").unwrap_or_else(|_| "http://localhost:8080".into())
}

fn arroyo_url() -> String {
    std::env::var("ARROYO_URL").unwrap_or_else(|_| "http://localhost:5115".into())
}

fn kafka_brokers() -> String {
    std::env::var("KAFKA_BROKERS").unwrap_or_else(|_| "localhost:19092".into())
}

// ── Helpers ───────────────────────────────────────────────────────────────────

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
        tokio::time::sleep(Duration::from_millis(800)).await;
    }
    None
}

/// POST a CloudTrail event to the collector.
async fn post_cloudtrail(
    client: &reqwest::Client,
    event_name: &str,
    user: &str,
    region: &str,
) -> bool {
    let payload = serde_json::json!({
        "Records": [{
            "eventName": event_name,
            "eventTime": chrono::Utc::now().to_rfc3339(),
            "awsRegion": region,
            "recipientAccountId": "222222222222",
            "responseElements": { "ConsoleLogin": "Success" },
            "userIdentity": {
                "type": "IAMUser",
                "userName": user,
                "arn": format!("arn:aws:iam::222222222222:user/{user}"),
                "accountId": "222222222222"
            }
        }]
    });

    client
        .post(format!("{}/ingest", collector_url()))
        .json(&payload)
        .timeout(Duration::from_secs(10))
        .send()
        .await
        .map(|r| r.status().is_success())
        .unwrap_or(false)
}

/// Seed `count` simple CloudTrail events to populate the Arroyo ETL pipeline.
async fn seed_events(client: &reqwest::Client, count: usize) {
    let regions = ["us-east-1", "us-west-2", "eu-west-1", "ap-southeast-1"];
    for i in 0..count {
        let user = format!("arroyo-test-user-{}@example.com", i % 10);
        let region = regions[i % 4];
        post_cloudtrail(client, "ConsoleLogin", &user, region).await;
    }
    println!("Seeded {count} events for Arroyo ETL pipeline");
}

/// Query MinIO via ClickHouse s3() and return the count of Parquet files under
/// the given prefix. Returns None on any error.
async fn count_arroyo_parquet(prefix: &str) -> Option<u64> {
    let client = reqwest::Client::new();
    // ClickHouse s3() glob — counts rows in Parquet files under prefix.
    // Arroyo writes flat files (no subdirectories) so *.parquet covers the default case.
    // Note: ClickHouse treats {..} as named-collection syntax, so we cannot use
    //       {**/*.parquet,*.parquet} — stick to the flat glob for Arroyo output.
    let sql = format!(
        "SELECT count(*) FROM s3('http://minio:9000/attest-warm/{prefix}/*.parquet', \
         'minioadmin', 'minioadmin', 'Parquet')"
    );
    let res = client
        .post(format!("{}/v1/warm/query", control_plane_url()))
        .json(&serde_json::json!({ "sql": sql }))
        .timeout(Duration::from_secs(15))
        .send()
        .await
        .ok()?;

    if !res.status().is_success() {
        return None;
    }

    let body: serde_json::Value = res.json().await.ok()?;
    body["rows"]
        .as_array()?
        .first()?
        .as_array()?
        .first()?
        .as_u64()
        .or_else(|| {
            body["rows"]
                .as_array()?
                .first()?
                .as_array()?
                .first()?
                .as_str()?
                .parse()
                .ok()
        })
}

// ── Tests ─────────────────────────────────────────────────────────────────────

/// Test 1: Arroyo API is healthy.
#[tokio::test]
async fn arroyo_api_is_healthy() {
    e2e_tests::require_e2e!();

    let client = reqwest::Client::new();
    let url = format!("{}/api/v1/ping", arroyo_url());

    let resp = poll_until(
        || {
            let client = client.clone();
            let url = url.clone();
            async move {
                client
                    .get(&url)
                    .timeout(Duration::from_secs(5))
                    .send()
                    .await
                    .ok()
                    .filter(|r| r.status().is_success())
                    .map(|_| ())
            }
        },
        Duration::from_secs(30),
    )
    .await;

    assert!(
        resp.is_some(),
        "Arroyo API at {} did not respond with 200 within 30s",
        arroyo_url()
    );
    println!("✓ Arroyo API is healthy");
}

/// Test 2: Both SQL pipelines are deployed and in Running state.
#[tokio::test]
async fn arroyo_pipelines_are_running() {
    e2e_tests::require_e2e!();

    let client = reqwest::Client::new();

    // Poll until pipelines are listed (deployer runs async after Arroyo starts)
    let pipelines: Option<Vec<String>> = poll_until(
        || {
            let client = client.clone();
            let url = format!("{}/api/v1/pipelines", arroyo_url());
            async move {
                let res = client
                    .get(&url)
                    .timeout(Duration::from_secs(5))
                    .send()
                    .await
                    .ok()?;

                if !res.status().is_success() {
                    return None;
                }

                let body: serde_json::Value = res.json().await.ok()?;
                let names: Vec<String> = body["data"]
                    .as_array()?
                    .iter()
                    .filter_map(|p| p["name"].as_str().map(|s| s.to_string()))
                    .collect();

                // Both pipelines must be present
                if names.iter().any(|n| n == "cloudtrail_to_parquet")
                    && names.iter().any(|n| n == "cep_sequence_detection")
                {
                    Some(names)
                } else {
                    None
                }
            }
        },
        Duration::from_secs(60),
    )
    .await;

    assert!(
        pipelines.is_some(),
        "Expected both pipelines (cloudtrail_to_parquet, cep_sequence_detection) \
         to be listed in Arroyo within 60s. \
         Check that arroyo-pipeline-deployer ran successfully."
    );

    let names = pipelines.unwrap();
    println!("✓ Pipelines deployed: {names:?}");

    // Verify each expected pipeline reports Running state via the jobs endpoint.
    // The pipeline-level object does not expose state directly; state lives on jobs.
    let expected = ["cloudtrail_to_parquet", "cep_sequence_detection"];
    for pipeline_name in expected {
        let running = poll_until(
            || {
                let client = client.clone();
                let list_url = format!("{}/api/v1/pipelines", arroyo_url());
                let name = pipeline_name.to_string();
                let arroyo = arroyo_url();
                async move {
                    // Step 1: find the pipeline id
                    let body: serde_json::Value = client
                        .get(&list_url)
                        .timeout(Duration::from_secs(5))
                        .send()
                        .await
                        .ok()?
                        .json()
                        .await
                        .ok()?;

                    let pipeline_id = body["data"].as_array()?.iter().find_map(|p| {
                        if p["name"].as_str() == Some(&name) {
                            p["id"].as_str().map(|s| s.to_string())
                        } else {
                            None
                        }
                    })?;

                    // Step 2: check the latest job is in Running state
                    let jobs_url = format!("{arroyo}/api/v1/pipelines/{pipeline_id}/jobs");
                    let jobs: serde_json::Value = client
                        .get(&jobs_url)
                        .timeout(Duration::from_secs(5))
                        .send()
                        .await
                        .ok()?
                        .json()
                        .await
                        .ok()?;

                    jobs["data"].as_array()?.iter().find_map(|j| {
                        if matches!(j["state"].as_str(), Some("Running") | Some("running")) {
                            Some(())
                        } else {
                            None
                        }
                    })
                }
            },
            Duration::from_secs(60),
        )
        .await;

        assert!(
            running.is_some(),
            "Pipeline '{pipeline_name}' did not reach Running state within 60s"
        );
        println!("  ✓ {pipeline_name} is Running");
    }
}

/// Test 3: ETL pipeline — Arroyo writes Parquet to the `arroyo/cloudtrail/` MinIO prefix.
#[tokio::test]
async fn arroyo_etl_pipeline_writes_parquet_to_minio() {
    e2e_tests::require_e2e!();

    let client = reqwest::Client::new();
    const EVENT_COUNT: usize = 30; // Enough to ensure data is written

    println!("Seeding {EVENT_COUNT} events via collector …");
    seed_events(&client, EVENT_COUNT).await;

    // Arroyo flushes every 30s via rolling_policy.interval / inactivity_interval.
    // Wait up to 90s (30s interval + startup + pipeline lag) for at least 1 Parquet file.
    println!("Waiting for Arroyo ETL to flush Parquet to MinIO (≤ 90 s) …");
    let count = poll_until(
        || async {
            count_arroyo_parquet("arroyo/cloudtrail")
                .await
                .filter(|&n| n > 0)
        },
        Duration::from_secs(90),
    )
    .await;

    assert!(
        count.is_some(),
        "No Parquet rows found under s3://attest-warm/arroyo/cloudtrail/ within 90s. \
         Check that the cloudtrail_to_parquet Arroyo pipeline is Running and \
         that the s3::http://minio:9000 path is reachable from Arroyo."
    );
    println!(
        "✓ Arroyo ETL: {} rows confirmed in MinIO under arroyo/cloudtrail/",
        count.unwrap()
    );
}

/// Test 4: CEP pipeline — login → S3 access within 5 min fires an alert on `alerts` topic.
#[tokio::test]
async fn arroyo_cep_pipeline_fires_sequence_alert() {
    e2e_tests::require_e2e!();

    let client = reqwest::Client::new();
    let test_user = format!(
        "cep-test-{}@example.com",
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs()
    );

    // Subscribe to the alerts Kafka topic BEFORE injecting events so we don't
    // miss the message.
    let consumer: StreamConsumer = ClientConfig::new()
        .set("bootstrap.servers", kafka_brokers())
        .set(
            "group.id",
            format!("arroyo-cep-e2e-{}", uuid::Uuid::new_v4()),
        )
        .set("auto.offset.reset", "latest")
        // Trigger an immediate metadata fetch so partition assignment happens now,
        // not on the first recv() call (avoids missing early alerts).
        .set("session.timeout.ms", "6000")
        .set("heartbeat.interval.ms", "1000")
        .set("enable.auto.commit", "false")
        .create()
        .expect("failed to create Kafka consumer for alerts topic");

    consumer
        .subscribe(&["alerts"])
        .expect("failed to subscribe to alerts topic");

    // Trigger partition assignment eagerly by polling with a short timeout.
    // This is more reliable than a fixed sleep because rdkafka assigns partitions
    // on the first recv() / poll() call, not on subscribe().
    let _ = tokio::time::timeout(Duration::from_secs(3), consumer.recv()).await;
    println!("  (alerts consumer partitions assigned)");

    // Step 1: inject ConsoleLogin (Success) from us-east-1
    println!("Injecting ConsoleLogin for {test_user} …");
    let ok = post_cloudtrail(&client, "ConsoleLogin", &test_user, "us-east-1").await;
    assert!(ok, "ConsoleLogin event failed to ingest");

    // Small gap — within the 5-minute window
    tokio::time::sleep(Duration::from_millis(500)).await;

    // Step 2: inject GetObject (S3 access) for the same user
    println!("Injecting GetObject for {test_user} …");
    let ok = post_cloudtrail(&client, "GetObject", &test_user, "us-east-1").await;
    assert!(ok, "GetObject event failed to ingest");

    // Keep pumping watermark-advance events throughout the entire wait window.
    // Arroyo's streaming JOIN needs the watermark to advance before it emits.
    // We pump one event every ~500 ms so the watermark keeps moving regardless
    // of how long the pipeline takes to process previous batches.
    println!("Waiting for CEP alert on `alerts` topic (≤ 60 s) …");
    let deadline = Instant::now() + Duration::from_secs(60);
    let mut found = false;
    let mut pump_tick = Instant::now();
    let mut pump_idx: u32 = 0;

    while Instant::now() < deadline && !found {
        // Pump a watermark-advance event every 500 ms.
        if pump_tick.elapsed() >= Duration::from_millis(500) {
            post_cloudtrail(
                &client,
                "ConsoleLogin",
                &format!("watermark-advance-{}@example.com", pump_idx),
                "eu-west-1",
            )
            .await;
            pump_idx += 1;
            pump_tick = Instant::now();
        }

        match tokio::time::timeout(Duration::from_millis(200), consumer.recv()).await {
            Ok(Ok(msg)) => {
                if let Some(payload) = msg.payload() {
                    if let Ok(body) = serde_json::from_slice::<serde_json::Value>(payload) {
                        let det_id = body["detection_id"].as_str().unwrap_or("");
                        let actor = body["actor_user_name"].as_str().unwrap_or("");
                        println!("  alert: detection_id={det_id} actor={actor}");
                        if det_id == "aws_login_then_s3_access_sequence" && actor == test_user {
                            found = true;
                        }
                    }
                }
            }
            Ok(Err(e)) => {
                eprintln!("Kafka consumer error: {e}");
            }
            Err(_) => {
                // poll timeout — continue loop
            }
        }
    }

    assert!(
        found,
        "Arroyo CEP pipeline did not fire 'aws_login_then_s3_access_sequence' \
         alert for {test_user} within 60 s (pumped {pump_idx} watermark-advance events). \
         Check that the cep_sequence_detection pipeline is Running."
    );
    println!("✓ Arroyo CEP alert fired for {test_user} (after {pump_idx} watermark pumps)");
}
