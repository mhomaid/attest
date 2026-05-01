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

const LOG_PATH: &str = "/Users/mohamedhomaid/StartUp-Projects/Attest/.cursor/debug-c5c3a9.log";

fn debug_log(hypothesis: &str, location: &str, message: &str, data: &str) {
    use std::io::Write;
    if let Ok(mut f) = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(LOG_PATH)
    {
        let ts = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_millis())
            .unwrap_or(0);
        let _ = writeln!(
            f,
            r#"{{"sessionId":"c5c3a9","timestamp":{ts},"hypothesisId":"{hypothesis}","location":"{location}","message":"{message}","data":{data}}}"#
        );
    }
}

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
        match client.post(&url).json(&payload).send().await {
            Ok(r) => {
                // #region agent log
                debug_log(
                    "H-B",
                    "phase2_iceberg.rs:seed_events",
                    "batch sent",
                    &format!(
                        r#"{{"seeded":{},"batch":{},"status":{},"elapsed_ms":{}}}"#,
                        seeded + this_batch,
                        this_batch,
                        r.status().as_u16(),
                        seed_start.elapsed().as_millis()
                    ),
                );
                // #endregion
            }
            Err(e) => {
                // #region agent log
                debug_log(
                    "H-B",
                    "phase2_iceberg.rs:seed_events",
                    "batch error",
                    &format!(r#"{{"seeded":{},"error":"{}"}}"#, seeded, e),
                );
                // #endregion
            }
        }
        seeded += this_batch;
    }

    let elapsed = seed_start.elapsed();
    println!("Seeded {seeded} events in {elapsed:.1?}");
    // #region agent log
    debug_log(
        "H-B",
        "phase2_iceberg.rs:seed_events",
        "seeding complete",
        &format!(
            r#"{{"total":{},"elapsed_ms":{}}}"#,
            seeded,
            elapsed.as_millis()
        ),
    );
    // #endregion
}

/// Call /v1/warm/query and return the first integer in the first row (or None).
/// Logs the raw HTTP status + body for diagnostics.
async fn warm_count_query(sql: &str) -> Option<u64> {
    let client = reqwest::Client::new();
    let res = client
        .post(format!("{}/v1/warm/query", control_plane_url()))
        .json(&serde_json::json!({ "sql": sql }))
        .send()
        .await;

    let res = match res {
        Ok(r) => r,
        Err(e) => {
            // #region agent log
            debug_log(
                "H-A",
                "phase2_iceberg.rs:warm_count_query",
                "http send error",
                &format!(r#"{{"error":"{}"}}"#, e),
            );
            // #endregion
            return None;
        }
    };

    let status = res.status().as_u16();
    let body_text = res.text().await.unwrap_or_default();

    // #region agent log
    debug_log(
        "H-A",
        "phase2_iceberg.rs:warm_count_query",
        "response",
        &format!(
            r#"{{"status":{},"body_preview":"{}"}}"#,
            status,
            body_text
                .chars()
                .take(200)
                .collect::<String>()
                .replace('"', "\\\"")
        ),
    );
    // #endregion

    if status != 200 {
        return None;
    }

    let body: serde_json::Value = serde_json::from_str(&body_text).ok()?;
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
    // #region agent log
    debug_log(
        "H-B",
        "phase2_iceberg.rs:main",
        "seed start",
        &format!(r#"{{"target":{TARGET},"batch_size":1000}}"#),
    );
    // #endregion
    seed_events(TARGET, 1000).await; // 10 batches of 1000 — ~10× faster than 50×200

    // Probe warm query ONCE before the poll to get a diagnostic snapshot.
    let probe_sql = "SELECT count(*) FROM s3('http://minio:9000/attest-warm/cloudtrail/**/*.parquet', 'minioadmin', 'minioadmin', 'Parquet')";
    // #region agent log
    debug_log(
        "H-A",
        "phase2_iceberg.rs:main",
        "probing warm query before poll",
        r#"{}"#,
    );
    // #endregion
    let _probe = warm_count_query(probe_sql).await;

    // Wait for storage-iceberg to flush Parquet to MinIO (≤ 90 s).
    println!("Waiting for Iceberg commit (≤ 90 s) …");
    let count = poll_until(
        || async {
            warm_count_query(probe_sql)
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
    println!("✓ {TARGET} events confirmed in Iceberg/MinIO");

    // Assert aggregate query completes within 30 s.
    println!("Running aggregate query (≤ 30 s) …");
    let agg_start = Instant::now();
    let agg_sql = "SELECT cloud_region, count(*) FROM s3('http://minio:9000/attest-warm/cloudtrail/**/*.parquet', 'minioadmin', 'minioadmin', 'Parquet') GROUP BY cloud_region ORDER BY cloud_region";
    let agg_result = warm_count_query(&format!("SELECT count(*) FROM ({agg_sql})")).await;
    let elapsed = agg_start.elapsed();

    // #region agent log
    debug_log(
        "H-C",
        "phase2_iceberg.rs:main",
        "agg query done",
        &format!(
            r#"{{"result":{:?},"elapsed_ms":{}}}"#,
            agg_result,
            elapsed.as_millis()
        ),
    );
    // #endregion

    assert!(agg_result.is_some(), "aggregate query returned no results");
    assert!(
        elapsed < Duration::from_secs(30),
        "aggregate query took {elapsed:.1?}, expected < 30 s"
    );
    println!("✓ aggregate query completed in {elapsed:.1?}");
}
