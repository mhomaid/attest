//! Phase 2 E2E test — Iceberg Warm Tier
//!
//! Acceptance gate from `10_Build_Order.md`:
//!   1. Seed 10 000 CloudTrail events via the collector HTTP endpoint.
//!   2. Wait ≤ 60 s for attest-storage-iceberg to flush Parquet files to MinIO.
//!   3. Assert that `POST /v1/warm/query` with `SELECT count(*)` returns 10 000.
//!   4. Assert that a GROUP-BY aggregate query completes within 30 s.
//!
//! Guards:
//!   - Skipped unless `ATTEST_E2E=1` is set.
//!   - All services must be running via `make dev-up-platform`.

use std::time::{Duration, Instant};

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

fn skip_if_no_e2e() -> bool {
    std::env::var("ATTEST_E2E").as_deref() != Ok("1")
}

/// Seed `count` events in batches of `batch` via the collector /ingest endpoint.
async fn seed_events(count: usize, batch: usize) {
    let client = reqwest::Client::new();
    let url = format!("{}/ingest", collector_url());
    let mut seeded = 0usize;

    while seeded < count {
        let this_batch = (count - seeded).min(batch);
        let records: Vec<serde_json::Value> = (0..this_batch)
            .map(|i| {
                let user = format!("seed-user-{}@example.com", (seeded + i) % 50);
                let region = ["us-east-1", "us-west-2", "eu-west-1", "ap-southeast-1"]
                    [(seeded + i) % 4];
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
        let _ = client.post(&url).json(&payload).send().await;
        seeded += this_batch;
    }

    println!("Seeded {seeded} events");
}

/// Call /v1/warm/query and return the first integer in the first row (or None).
async fn warm_count_query(sql: &str) -> Option<u64> {
    let client = reqwest::Client::new();
    let res = client
        .post(format!("{}/v1/warm/query", control_plane_url()))
        .json(&serde_json::json!({ "sql": sql }))
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

#[tokio::test]
async fn events_persisted_to_iceberg_are_queryable_via_clickhouse() {
    if skip_if_no_e2e() {
        println!("ATTEST_E2E not set — skipping Phase 2 E2E test");
        return;
    }

    const TARGET: usize = 10_000;

    println!("Seeding {TARGET} events …");
    seed_events(TARGET, 200).await;

    // Wait for storage-iceberg to flush Parquet to MinIO (≤ 60 s).
    println!("Waiting for Iceberg commit (≤ 60 s) …");
    let count = poll_until(
        || async {
            let sql = "SELECT count(*) FROM s3('http://minio:9000/attest-warm/cloudtrail/**/*.parquet', 'minioadmin', 'minioadmin', 'Parquet')";
            warm_count_query(sql).await.filter(|&n| n >= TARGET as u64)
        },
        Duration::from_secs(60),
    )
    .await;

    assert!(
        count.is_some(),
        "expected {TARGET} events in MinIO Parquet within 60s, got {count:?}"
    );
    println!("✓ {TARGET} events confirmed in Iceberg/MinIO");

    // Assert aggregate query completes within 30 s.
    println!("Running aggregate query (≤ 30 s) …");
    let agg_start = Instant::now();
    let sql = "SELECT cloud_region, count(*) FROM s3('http://minio:9000/attest-warm/cloudtrail/**/*.parquet', 'minioadmin', 'minioadmin', 'Parquet') GROUP BY cloud_region ORDER BY cloud_region";
    let agg_result = warm_count_query(&format!("SELECT count(*) FROM ({sql})")).await;
    let elapsed = agg_start.elapsed();

    assert!(
        agg_result.is_some(),
        "aggregate query returned no results"
    );
    assert!(
        elapsed < Duration::from_secs(30),
        "aggregate query took {elapsed:.1?}, expected < 30 s"
    );
    println!("✓ aggregate query completed in {elapsed:.1?}");
}
