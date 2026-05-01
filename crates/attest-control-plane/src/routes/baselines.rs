//! GET /v1/baselines/user/:name
//! Returns the 30-day baseline for a user from the `entity_baselines` mat. view.

use axum::{
    Json,
    extract::{Path, State},
    http::StatusCode,
    response::{IntoResponse, Response},
};
use serde::Serialize;
use utoipa::ToSchema;

use crate::state::AppState;

#[derive(Serialize, ToSchema)]
pub struct UserBaseline {
    pub tenant_id: String,
    pub actor_user_name: String,
    pub regions_seen_30d: Vec<String>,
    pub event_count_30d: i64,
    pub last_seen: String,
}

/// Return the 30-day behavioural baseline for a user from the `entity_baselines` materialized view.
#[utoipa::path(
    get,
    path = "/v1/baselines/user/{name}",
    params(
        ("name" = String, Path, description = "IAM user name to look up"),
    ),
    responses(
        (status = 200, description = "Baseline found", body = UserBaseline),
        (status = 404, description = "User not found in baseline"),
        (status = 503, description = "DB not ready"),
    ),
    tag = "baselines"
)]

pub async fn get_user_baseline(
    State(state): State<AppState>,
    Path(user_name): Path<String>,
) -> Response {
    let db = match state.get_db() {
        Some(db) => db,
        None => return (
            StatusCode::SERVICE_UNAVAILABLE,
            Json(serde_json::json!({"error": "service starting up"})),
        ).into_response(),
    };

    let rows = match db
        .query(
            "SELECT tenant_id, actor_user_name, \
             regions_seen_30d, event_count_30d, last_seen::TEXT \
             FROM entity_baselines WHERE actor_user_name = $1",
            &[&user_name],
        )
        .await
    {
        Ok(r) => r,
        Err(e) => {
            tracing::error!("DB error: {e}");
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(serde_json::json!({"error": e.to_string()})),
            ).into_response();
        }
    };

    if rows.is_empty() {
        return (
            StatusCode::NOT_FOUND,
            Json(serde_json::json!({"error": "user not found in baseline"})),
        ).into_response();
    }

    let row = &rows[0];
    let regions: Vec<String> = row.get(2);
    let baseline = UserBaseline {
        tenant_id:          row.get(0),
        actor_user_name:    row.get(1),
        regions_seen_30d:   regions,
        event_count_30d:    row.get(3),
        last_seen:          row.get(4),
    };

    (StatusCode::OK, Json(serde_json::to_value(baseline).unwrap())).into_response()
}
