//! GET /v1/baselines/user/:name
//! Returns the 30-day baseline for a user from the `entity_baselines` mat. view.

use axum::{
    Json,
    extract::{Path, State},
    http::StatusCode,
    response::IntoResponse,
};
use serde::Serialize;

use crate::state::AppState;

#[derive(Serialize)]
pub struct UserBaseline {
    pub tenant_id: String,
    pub actor_user_name: String,
    pub regions_seen_30d: Vec<String>,
    pub event_count_30d: i64,
    pub last_seen: String,
}

pub async fn get_user_baseline(
    State(state): State<AppState>,
    Path(user_name): Path<String>,
) -> impl IntoResponse {
    let rows = match state
        .db
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
            );
        }
    };

    if rows.is_empty() {
        return (
            StatusCode::NOT_FOUND,
            Json(serde_json::json!({"error": "user not found in baseline"})),
        );
    }

    let row = &rows[0];
    let regions: Vec<String> = row.get(2);
    let baseline = UserBaseline {
        tenant_id: row.get(0),
        actor_user_name: row.get(1),
        regions_seen_30d: regions,
        event_count_30d: row.get(3),
        last_seen: row.get(4),
    };

    (StatusCode::OK, Json(serde_json::to_value(baseline).unwrap()))
}
