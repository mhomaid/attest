use crate::db::Db;

/// Shared application state injected into every route handler.
#[derive(Clone)]
pub struct AppState {
    pub db: Db,
    /// Broadcast channel: each message is a JSON-serialised `FiredAlert`.
    /// Cloned into the background polling task and each WebSocket handler.
    pub alert_tx: tokio::sync::broadcast::Sender<String>,
}
