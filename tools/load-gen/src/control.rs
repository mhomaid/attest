//! Axum HTTP control API on :9100.
//!
//! POST /run    — start a benchmark run
//! POST /stop   — cancel the active run
//! GET  /status — current metrics snapshot

use axum::{
    extract::State,
    http::StatusCode,
    response::IntoResponse,
    routing::{get, post},
    Json, Router,
};
use serde_json::json;
use tokio::sync::RwLock;
use std::sync::Arc;

use crate::generator::{start_run, RunConfig, RunHandle, RunStatus};

pub struct AppState {
    pub brokers: String,
    pub run: Option<RunHandle>,
}

impl AppState {
    pub fn new(brokers: String) -> Self {
        AppState { brokers, run: None }
    }
}

pub type SharedState = Arc<RwLock<AppState>>;

pub fn build_router(state: SharedState) -> Router {
    Router::new()
        .route("/healthz", get(healthz))
        .route("/run", post(run_handler))
        .route("/stop", post(stop_handler))
        .route("/status", get(status_handler))
        .with_state(state)
}

async fn healthz() -> impl IntoResponse {
    Json(json!({ "ok": true }))
}

async fn run_handler(
    State(state): State<SharedState>,
    Json(cfg): Json<RunConfig>,
) -> impl IntoResponse {
    let s = state.write().await;
    if let Some(ref handle) = s.run {
        if handle.running.load(std::sync::atomic::Ordering::Relaxed) {
            return (
                StatusCode::CONFLICT,
                Json(json!({ "error": "a run is already active — POST /stop first" })),
            );
        }
    }

    let brokers = s.brokers.clone();
    drop(s); // Release the write lock before awaiting

    match start_run(brokers, cfg).await {
        Ok(handle) => {
            let mut s = state.write().await;
            s.run = Some(handle);
            (StatusCode::ACCEPTED, Json(json!({ "started": true })))
        }
        Err(e) => {
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({ "error": e.to_string() })),
            )
        }
    }
}

async fn stop_handler(State(state): State<SharedState>) -> impl IntoResponse {
    let s = state.read().await;
    match s.run.as_ref() {
        Some(handle) => {
            handle.stop();
            (StatusCode::OK, Json(json!({ "stopped": true })))
        }
        None => (
            StatusCode::NOT_FOUND,
            Json(json!({ "error": "no active run" })),
        ),
    }
}

async fn status_handler(State(state): State<SharedState>) -> impl IntoResponse {
    let s = state.read().await;
    match s.run.as_ref() {
        Some(handle) => {
            let status: RunStatus = handle.status();
            (StatusCode::OK, Json(serde_json::to_value(status).unwrap()))
        }
        None => (
            StatusCode::OK,
            Json(json!({ "running": false, "sent": 0, "errors": 0,
                          "elapsed_secs": 0.0, "rate_actual": 0.0,
                          "p50_ms": 0.0, "p95_ms": 0.0, "p99_ms": 0.0 })),
        ),
    }
}
