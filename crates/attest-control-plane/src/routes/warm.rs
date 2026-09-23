use std::sync::Arc;

use attest_storage_clickhouse::{ClickHouseClient, QueryError};
use axum::{extract::State, http::StatusCode, response::IntoResponse, Json};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use tracing::{error, warn};
use utoipa::ToSchema;

pub type WarmClient = Arc<ClickHouseClient>;

#[derive(Debug, Deserialize, ToSchema)]
pub struct WarmQueryRequest {
    /// A read-only SELECT statement to run against ClickHouse.
    pub sql: String,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct WarmQueryResponse {
    pub rows: Vec<Vec<Value>>,
    pub row_count: usize,
}

/// Run a read-only SELECT against the ClickHouse warm tier.
///
/// Queries are validated before they reach ClickHouse: a single SELECT/WITH statement, no
/// network or file table functions, `s3()` only against the warm bucket, no `SETTINGS`
/// overrides. ClickHouse additionally enforces `readonly`, a 30 s execution limit and a
/// 10 000-row result limit.
#[utoipa::path(
    post,
    path = "/v1/warm/query",
    request_body(content = WarmQueryRequest, description = "SQL SELECT to execute"),
    responses(
        (status = 200, description = "Query results", body = WarmQueryResponse),
        (status = 400, description = "Query rejected by the warm-tier guard"),
        (status = 502, description = "ClickHouse error"),
    ),
    tag = "warm"
)]
pub async fn post_warm_query(
    State(client): State<WarmClient>,
    Json(req): Json<WarmQueryRequest>,
) -> impl IntoResponse {
    match client.query(&req.sql).await {
        Ok(rows) => {
            let row_count = rows.len();
            (
                StatusCode::OK,
                Json(json!(WarmQueryResponse { rows, row_count })),
            )
                .into_response()
        }
        Err(QueryError::Rejected(reason)) => {
            warn!(%reason, "warm query rejected");
            (
                StatusCode::BAD_REQUEST,
                Json(json!({ "error": reason.to_string() })),
            )
                .into_response()
        }
        Err(e) => {
            error!("warm query error: {e}");
            (
                StatusCode::BAD_GATEWAY,
                Json(json!({ "error": e.to_string() })),
            )
                .into_response()
        }
    }
}
