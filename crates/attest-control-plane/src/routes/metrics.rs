//! WebSocket endpoint for streaming live `MetricsSnapshot` to the workbench.
//!
//! `GET /v1/metrics/stream` — upgrades to WebSocket, then sends one JSON frame
//! per second as the sampler broadcasts.  Pings every 15 s for proxy keep-alive.

use axum::{
    extract::{
        ws::{Message, WebSocket},
        State, WebSocketUpgrade,
    },
    response::IntoResponse,
};
use std::time::Duration;
use tracing::{debug, warn};

use crate::state::AppState;

pub async fn ws_metrics(ws: WebSocketUpgrade, State(state): State<AppState>) -> impl IntoResponse {
    ws.on_upgrade(move |socket| handle_socket(socket, state))
}

async fn handle_socket(mut socket: WebSocket, state: AppState) {
    let mut rx = state.metrics_tx.subscribe();
    let mut ping_interval = tokio::time::interval(Duration::from_secs(15));
    // skip the immediate first tick
    ping_interval.tick().await;

    loop {
        tokio::select! {
            snap = rx.recv() => {
                match snap {
                    Ok(s) => {
                        match serde_json::to_string(&s) {
                            Ok(json) => {
                                if socket.send(Message::Text(json.into())).await.is_err() {
                                    debug!("metrics WS: client disconnected");
                                    break;
                                }
                            }
                            Err(e) => warn!("metrics serialize error: {e}"),
                        }
                    }
                    Err(tokio::sync::broadcast::error::RecvError::Lagged(n)) => {
                        warn!("metrics WS lagged {n} frames");
                    }
                    Err(_) => break,
                }
            }
            _ = ping_interval.tick() => {
                if socket.send(Message::Ping(vec![].into())).await.is_err() {
                    break;
                }
            }
        }
    }
}
