use axum::{Json, http::StatusCode, response::IntoResponse};

pub async fn healthz() -> impl IntoResponse {
    (StatusCode::OK, Json(serde_json::json!({"status": "ok"})))
}
