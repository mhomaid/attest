//! Phase 2 E2E test — warm tier (Parquet on MinIO, queried through ClickHouse)
//!
//! Acceptance gate from `10_Build_Order.md`:
//!   1. Seed 10 000 CloudTrail events via the collector HTTP endpoint.
//!   2. Wait ≤ 90 s for attest-storage-iceberg to flush Parquet files to MinIO.
//!   3. Assert that `POST /v1/warm/query` with `SELECT count(*)` returns 10 000.
//!   4. Assert that a GROUP-BY aggregate query completes within 30 s.
//!
//! Guards:
//!   - Skipped unless `ATTEST_E2E=1` is set.
//!   - All services must be running via `make dev-up-all`.

use std::time::{Duration, Instant};

const WARM_GLOB: &str =
    "s3('http://minio:9000/attest-warm/cloudtrail/**/*.parquet', 'minioadmin', 'minioadmin', 'Parquet')";

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
        tokio::time::sleep(Duration::from_millis(500)).await;
    }
    None
}

fn collector_url() -> String {
    std::env::var("COLLECTOR_URL").unwrap_or_else(|_| "http://localhost:4000".into())
}

fn control_plane_url() -> String {
    std::env::var("CONTROL_PLANE_URL").unwrap_or_else(|_| "http://localhost:8080".into())
}

/// Seed `count` events in batches of `batch` via the collector /ingest endpoint.
async fn seed_events(count: usize, batch: usize) {
    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(30))
        .build()
        .unwrap();
    let url = format!("{}/ingest", collector_url());
    let mut seeded = 0usize;
    let seed_start = Instant::now();

    while seeded < count {
        let this_batch = (count - seeded).min(batch);
        let records: Vec<serde_json::Value> = (0..this_batch)
            .map(|i| {
                let user = format!("seed-user-{}@example.com", (seeded + i) % 50);
                let region =
                    ["us-east-1", "us-west-2", "eu-west-1", "ap-southeast-1"][(seeded + i) % 4];
                serde_json::json!({
                    "eventName": "ConsoleLogin",
                    "eventTime": chrono::Utc::now().to_rfc3339(),
                    "awsRegion": region,
                    "recipientAccountId": "111111111111",
                    "userIdentity": {
                        "type": "IAMUser",
                        "userName": user,
                        "arn": format!("arn:aws:iam::111111111111:user/{user}"),
                        "accountId": "111111111111"
                    }
                })
            })
            .collect();

        let payload = serde_json::json!({ "Records": records });
        let res = client
            .post(&url)
            .json(&payload)
            .send()
            .await
            .expect("collector /ingest unreachable");
        assert!(
            res.status().is_success(),
            "collector rejected batch at offset {seeded}: HTTP {}",
            res.status()
        );
        seeded += this_batch;
    }

    println!("Seeded {seeded} events in {:.1?}", seed_start.elapsed());
}

/// Call /v1/warm/query and return the first integer in the first row (or None).
async fn warm_count_query(sql: &str) -> Option<u64> {
    let res = reqwest::Client::new()
        .post(format!("{}/v1/warm/query", control_plane_url()))
        .json(&serde_json::json!({ "sql": sql }))
        .send()
        .await
        .ok()?;

    let status = res.status();
    let body_text = res.text().await.unwrap_or_default();
    if !status.is_success() {
        eprintln!(
            "warm query HTTP {status}: {}",
            body_text.chars().take(200).collect::<String>()
        );
        return None;
    }

    let body: serde_json::Value = serde_json::from_str(&body_text).ok()?;
    let cell = body["rows"].as_array()?.first()?.as_array()?.first()?;
    cell.as_u64().or_else(|| cell.as_str()?.parse().ok())
}

#[tokio::test]
async fn events_persisted_to_warm_tier_are_queryable_via_clickhouse() {
    e2e_tests::require_e2e!();

    const TARGET: usize = 10_000;

    println!("Seeding {TARGET} events …");
    seed_events(TARGET, 1000).await;

    println!("Waiting for Parquet flush to MinIO (≤ 90 s) …");
    let count_sql = format!("SELECT count(*) FROM {WARM_GLOB}");
    let count = poll_until(
        || async {
            warm_count_query(&count_sql)
                .await
                .filter(|&n| n >= TARGET as u64)
        },
        Duration::from_secs(90),
    )
    .await;

    assert!(
        count.is_some(),
        "expected {TARGET} events in MinIO Parquet within 90s, got {count:?}"
    );
    println!("✓ {TARGET} events confirmed in MinIO Parquet");

    println!("Running aggregate query (≤ 30 s) …");
    let agg_start = Instant::now();
    let agg_sql = format!(
        "SELECT count(*) FROM (SELECT cloud_region, count(*) FROM {WARM_GLOB} GROUP BY cloud_region)"
    );
    let agg_result = warm_count_query(&agg_sql).await;
    let elapsed = agg_start.elapsed();

    assert!(agg_result.is_some(), "aggregate query returned no results");
    assert!(
        elapsed < Duration::from_secs(30),
        "aggregate query took {elapsed:.1?}, expected < 30 s"
    );
    println!("✓ aggregate query completed in {elapsed:.1?}");
}
