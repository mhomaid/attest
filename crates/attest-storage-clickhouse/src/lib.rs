//! ClickHouse warm-tier client.
//!
//! Every query is statically validated (see [`guard`]) and sent with server-side limits:
//! `readonly=2` (no writes; table functions allowed so `s3()` works), a maximum execution
//! time and a maximum result size. Results use `JSONCompact`, so rows are positional arrays.

pub mod guard;

use std::time::Duration;

pub use guard::{validate_read_query, Rejection};
use serde_json::Value;

pub const DEFAULT_URL: &str = "http://localhost:8123";
pub const DEFAULT_S3_PREFIX: &str = "http://minio:9000/attest-warm/";

#[derive(Debug, thiserror::Error)]
pub enum QueryError {
    #[error("query rejected: {0}")]
    Rejected(#[from] Rejection),
    #[error("ClickHouse request failed: {0}")]
    Transport(#[from] reqwest::Error),
    #[error("ClickHouse returned HTTP {status}: {body}")]
    Upstream { status: u16, body: String },
    #[error("unexpected ClickHouse response: {0}")]
    Decode(String),
}

#[derive(Debug, Clone, Copy)]
pub struct QueryLimits {
    pub max_execution_secs: u32,
    pub max_result_rows: u64,
}

impl Default for QueryLimits {
    fn default() -> Self {
        Self {
            max_execution_secs: 30,
            max_result_rows: 10_000,
        }
    }
}

#[derive(Debug, Clone)]
pub struct ClickHouseClient {
    base_url: String,
    s3_prefix: String,
    limits: QueryLimits,
    http: reqwest::Client,
}

impl ClickHouseClient {
    pub fn new(base_url: impl Into<String>) -> Self {
        Self {
            base_url: base_url.into().trim_end_matches('/').to_string(),
            s3_prefix: DEFAULT_S3_PREFIX.to_string(),
            limits: QueryLimits::default(),
            http: reqwest::Client::new(),
        }
    }

    /// `CLICKHOUSE_URL` (default `http://localhost:8123`) and `WARM_S3_PREFIX`
    /// (default `http://minio:9000/attest-warm/`).
    pub fn from_env() -> Self {
        let client =
            Self::new(std::env::var("CLICKHOUSE_URL").unwrap_or_else(|_| DEFAULT_URL.to_string()));
        match std::env::var("WARM_S3_PREFIX") {
            Ok(prefix) => client.with_s3_prefix(prefix),
            Err(_) => client,
        }
    }

    pub fn with_s3_prefix(mut self, prefix: impl Into<String>) -> Self {
        self.s3_prefix = prefix.into();
        self
    }

    pub fn with_limits(mut self, limits: QueryLimits) -> Self {
        self.limits = limits;
        self
    }

    /// Client-side request timeout, on top of ClickHouse's own `max_execution_time`.
    pub fn with_timeout(mut self, timeout: Duration) -> Self {
        self.http = reqwest::Client::builder()
            .timeout(timeout)
            .build()
            .expect("reqwest client with timeout");
        self
    }

    pub fn base_url(&self) -> &str {
        &self.base_url
    }

    /// Validate and run a read-only query, returning positional rows.
    pub async fn query(&self, sql: &str) -> Result<Vec<Vec<Value>>, QueryError> {
        let sql = validate_read_query(sql, &self.s3_prefix)?;
        let body = self.post(&format!("{sql}\nFORMAT JSONCompact")).await?;
        let json: Value =
            serde_json::from_str(&body).map_err(|e| QueryError::Decode(e.to_string()))?;
        let rows = json
            .get("data")
            .and_then(Value::as_array)
            .ok_or_else(|| QueryError::Decode("missing `data` array".into()))?;
        Ok(rows.iter().filter_map(|r| r.as_array().cloned()).collect())
    }

    /// `SELECT count() FROM <table>`. `table` must be a plain (optionally db-qualified) name.
    pub async fn count_rows(&self, table: &str) -> Result<u64, QueryError> {
        let valid = !table.is_empty()
            && table
                .chars()
                .all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '.');
        if !valid {
            return Err(QueryError::Rejected(Rejection::NotSelect));
        }
        let rows = self.query(&format!("SELECT count() FROM {table}")).await?;
        let cell = rows
            .first()
            .and_then(|r| r.first())
            .ok_or_else(|| QueryError::Decode("empty count result".into()))?;
        cell.as_u64()
            .or_else(|| cell.as_str()?.parse().ok())
            .ok_or_else(|| QueryError::Decode(format!("non-integer count: {cell}")))
    }

    async fn post(&self, body: &str) -> Result<String, QueryError> {
        let resp = self
            .http
            .post(&self.base_url)
            .query(&[
                ("readonly", "2".to_string()),
                (
                    "max_execution_time",
                    self.limits.max_execution_secs.to_string(),
                ),
                ("max_result_rows", self.limits.max_result_rows.to_string()),
                ("result_overflow_mode", "throw".to_string()),
            ])
            .body(body.to_string())
            .send()
            .await?;
        let status = resp.status();
        let text = resp.text().await?;
        if !status.is_success() {
            return Err(QueryError::Upstream {
                status: status.as_u16(),
                body: text.chars().take(500).collect(),
            });
        }
        Ok(text)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::{extract::RawQuery, routing::post, Router};
    use std::sync::{Arc, Mutex};

    type Seen = Arc<Mutex<Vec<(String, String)>>>;

    /// Minimal stand-in for ClickHouse's HTTP interface: records (query string, body) and
    /// answers with a fixed JSONCompact payload.
    async fn fake_clickhouse(response: &'static str) -> (String, Seen) {
        let seen: Seen = Arc::default();
        let recorder = seen.clone();
        let app = Router::new().route(
            "/",
            post(move |RawQuery(q): RawQuery, body: String| {
                let recorder = recorder.clone();
                async move {
                    recorder.lock().unwrap().push((q.unwrap_or_default(), body));
                    response
                }
            }),
        );
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        tokio::spawn(async move { axum::serve(listener, app).await.unwrap() });
        (format!("http://{addr}"), seen)
    }

    #[tokio::test]
    async fn query_sends_limits_and_parses_rows() {
        let (url, seen) =
            fake_clickhouse(r#"{"meta":[],"data":[["us-east-1","42"]],"rows":1}"#).await;
        let rows = ClickHouseClient::new(url)
            .query("SELECT cloud_region, count() FROM cloudtrail_events GROUP BY 1;")
            .await
            .unwrap();
        assert_eq!(
            rows,
            vec![vec![Value::from("us-east-1"), Value::from("42")]]
        );

        let (params, body) = seen.lock().unwrap()[0].clone();
        for expected in [
            "readonly=2",
            "max_execution_time=30",
            "max_result_rows=10000",
            "result_overflow_mode=throw",
        ] {
            assert!(params.contains(expected), "missing {expected} in {params}");
        }
        assert_eq!(
            body,
            "SELECT cloud_region, count() FROM cloudtrail_events GROUP BY 1\nFORMAT JSONCompact"
        );
    }

    #[tokio::test]
    async fn rejected_queries_never_reach_clickhouse() {
        let (url, seen) = fake_clickhouse(r#"{"data":[]}"#).await;
        let err = ClickHouseClient::new(url)
            .query("SELECT * FROM url('http://169.254.169.254/', 'LineAsString')")
            .await
            .unwrap_err();
        assert!(matches!(
            err,
            QueryError::Rejected(Rejection::ForbiddenFunction(_))
        ));
        assert!(seen.lock().unwrap().is_empty());
    }

    #[tokio::test]
    async fn count_rows_parses_quoted_uint64() {
        let (url, _) = fake_clickhouse(r#"{"data":[["10000"]]}"#).await;
        let n = ClickHouseClient::new(url)
            .count_rows("cloudtrail_events")
            .await
            .unwrap();
        assert_eq!(n, 10_000);
        assert!(ClickHouseClient::new("http://unused")
            .count_rows("t; DROP")
            .await
            .is_err());
    }
}
