use axum::{Json, http::StatusCode, response::IntoResponse};
use utoipa::ToSchema;

#[derive(serde::Serialize, ToSchema)]
pub struct HealthResponse {
    pub status: String,
}

/// Liveness probe.
#[utoipa::path(
    get,
    path = "/healthz",
    responses(
        (status = 200, description = "Service healthy", body = HealthResponse),
    ),
    tag = "ops"
)]
pub async fn healthz() -> impl IntoResponse {
    (StatusCode::OK, Json(serde_json::json!({"status": "ok"})))
}
