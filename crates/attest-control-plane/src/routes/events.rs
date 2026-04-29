//! GET /v1/events/recent?id=<uuid>
//! Looks up a specific event by event_id from the `recent_events` materialized view.

use axum::{
    Json,
    extract::{Query, State},
    http::StatusCode,
    response::{IntoResponse, Response},
};
use serde::{Deserialize, Serialize};

use crate::state::AppState;

#[derive(Deserialize)]
pub struct EventQuery {
    pub id: String,
}

#[derive(Serialize)]
pub struct EventRow {
    pub event_id: String,
    pub class_uid: String,
    pub time: String,
    pub tenant_id: String,
    pub actor_user_name: String,
    pub actor_user_uid: String,
    pub cloud_region: String,
    pub cloud_account_uid: String,
    pub severity: String,
}

pub async fn get_recent_event(
    State(state): State<AppState>,
    Query(params): Query<EventQuery>,
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
            "SELECT event_id, class_uid, time::TEXT, tenant_id, \
             actor_user_name, actor_user_uid, cloud_region, cloud_account_uid, severity \
             FROM recent_events WHERE event_id = $1 LIMIT 1",
            &[&params.id],
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
            Json(serde_json::json!({"error": "event not found"})),
        ).into_response();
    }

    let row = &rows[0];
    let event = EventRow {
        event_id:         row.get(0),
        class_uid:        row.get(1),
        time:             row.get(2),
        tenant_id:        row.get(3),
        actor_user_name:  row.get(4),
        actor_user_uid:   row.get(5),
        cloud_region:     row.get(6),
        cloud_account_uid:row.get(7),
        severity:         row.get(8),
    };

    (StatusCode::OK, Json(serde_json::to_value(event).unwrap())).into_response()
}
