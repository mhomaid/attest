use axum::{Json, extract::State, http::StatusCode, response::IntoResponse};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use tracing::error;

/// Shared ClickHouse HTTP base URL, held in an `Arc<String>` so it's cheap to clone.
pub type ChUrl = std::sync::Arc<String>;

#[derive(Debug, Deserialize)]
pub struct WarmQueryRequest {
    pub sql: String,
}

#[derive(Debug, Serialize)]
pub struct WarmQueryResponse {
    pub rows: Vec<Vec<Value>>,
    pub row_count: usize,
}

pub async fn post_warm_query(
    State(ch_url): State<ChUrl>,
    Json(req): Json<WarmQueryRequest>,
) -> impl IntoResponse {
    let trimmed = req.sql.trim().to_lowercase();
    if !trimmed.starts_with("select") {
        return (
            StatusCode::BAD_REQUEST,
            Json(json!({"error": "only SELECT statements are permitted"})),
        )
            .into_response();
    }

    match run_query(&ch_url, &req.sql).await {
        Ok(resp) => (StatusCode::OK, Json(json!(resp))).into_response(),
        Err(e) => {
            error!("warm query error: {e}");
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({"error": e.to_string()})),
            )
                .into_response()
        }
    }
}

async fn run_query(ch_url: &str, sql: &str) -> anyhow::Result<WarmQueryResponse> {
    // ClickHouse HTTP API: POST the SQL with FORMAT JSON appended.
    let query = format!("{sql} FORMAT JSONCompact");
    let client = reqwest::Client::new();
    let resp = client
        .post(ch_url)
        .body(query)
        .send()
        .await
        .map_err(|e| anyhow::anyhow!("HTTP request to ClickHouse failed: {e}"))?;

    if !resp.status().is_success() {
        let status = resp.status();
        let body = resp.text().await.unwrap_or_default();
        return Err(anyhow::anyhow!("ClickHouse error {status}: {body}"));
    }

    // ClickHouse JSONCompact format: { "data": [[v1,v2,...], ...], ... }
    let json: Value = resp
        .json()
        .await
        .map_err(|e| anyhow::anyhow!("failed to parse ClickHouse response: {e}"))?;

    let rows: Vec<Vec<Value>> = json
        .get("data")
        .and_then(|d| d.as_array())
        .map(|rows| {
            rows.iter()
                .filter_map(|r| r.as_array().cloned())
                .collect()
        })
        .unwrap_or_default();

    let row_count = rows.len();
    Ok(WarmQueryResponse { rows, row_count })
}
