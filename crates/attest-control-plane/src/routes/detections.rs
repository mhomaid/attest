//! GET  /v1/detections/fired  — returns all rows currently in every det_* view
//! GET  /v1/ws/alerts          — WebSocket: streams new fired rows in real time

use axum::{
    Json,
    extract::{State, WebSocketUpgrade},
    http::StatusCode,
    response::IntoResponse,
};
use axum::extract::ws::{Message, WebSocket};
use serde::Serialize;

use crate::state::AppState;

/// Shape of one fired detection row (mirrors the det_* view SELECT).
#[derive(Debug, Serialize, Clone)]
pub struct FiredAlert {
    pub detection_id:   String,
    pub event_id:       String,
    pub actor_user_name: String,
    pub cloud_region:   String,
    pub severity:       String,
    pub fired_at:       String,
}

/// Discover all `det_*` materialized views in RisingWave and return their rows.
pub async fn get_detections_fired(
    State(state): State<AppState>,
) -> impl IntoResponse {
    match fetch_all_fired(&state).await {
        Ok(alerts) => (StatusCode::OK, Json(serde_json::to_value(alerts).unwrap())).into_response(),
        Err(e) => {
            tracing::error!("detections/fired error: {e}");
            (StatusCode::INTERNAL_SERVER_ERROR,
             Json(serde_json::json!({"error": e.to_string()}))).into_response()
        }
    }
}

pub async fn fetch_all_fired(state: &AppState) -> anyhow::Result<Vec<FiredAlert>> {
    let db = match state.get_db() {
        Some(db) => db,
        None => return Ok(vec![]), // DB not ready yet — return empty list
    };

    // Discover det_* views dynamically from the RisingWave catalog.
    let view_rows = db
        .query(
            "SELECT name FROM rw_catalog.rw_materialized_views WHERE name LIKE 'det_%'",
            &[],
        )
        .await?;

    let mut alerts: Vec<FiredAlert> = Vec::new();

    for vr in &view_rows {
        let view_name: String = vr.get(0);
        match db
            .query(
                &format!(
                    "SELECT detection_id, event_id, actor_user_name, \
                     cloud_region, severity, fired_at \
                     FROM {view_name} ORDER BY fired_at DESC LIMIT 500"
                ),
                &[],
            )
            .await
        {
            Ok(rows) => {
                for row in rows {
                    alerts.push(FiredAlert {
                        detection_id:    row.get(0),
                        event_id:        row.get(1),
                        actor_user_name: row.try_get(2).unwrap_or_default(),
                        cloud_region:    row.try_get(3).unwrap_or_default(),
                        severity:        row.try_get(4).unwrap_or_else(|_| "medium".into()),
                        fired_at:        row.try_get(5).unwrap_or_default(),
                    });
                }
            }
            Err(e) => {
                tracing::warn!(view = %view_name, error = %e, "skipping view");
            }
        }
    }

    // Most-recent first.
    alerts.sort_by(|a, b| b.fired_at.cmp(&a.fired_at));
    Ok(alerts)
}

// ── WebSocket ─────────────────────────────────────────────────────────────────

/// Upgrade the connection to a WebSocket.  Each connected client receives
/// every new alert broadcast by the background polling task.
pub async fn ws_alerts(
    ws:             WebSocketUpgrade,
    State(state):   State<AppState>,
) -> impl IntoResponse {
    ws.on_upgrade(move |socket| handle_ws(socket, state))
}

async fn handle_ws(mut socket: WebSocket, state: AppState) {
    let mut rx = state.alert_tx.subscribe();
    loop {
        tokio::select! {
            msg = rx.recv() => {
                match msg {
                    Ok(json) => {
                        if socket.send(Message::Text(json.into())).await.is_err() {
                            break; // client disconnected
                        }
                    }
                    Err(tokio::sync::broadcast::error::RecvError::Lagged(n)) => {
                        tracing::warn!(skipped = n, "WS client lagged, skipping messages");
                    }
                    Err(_) => break,
                }
            }
            // Drain any ping/pong/close frames from the client.
            msg = socket.recv() => {
                match msg {
                    Some(Ok(Message::Close(_))) | None => break,
                    _ => {} // ignore pings
                }
            }
        }
    }
}
