use std::sync::{Arc, OnceLock};
use crate::db::Db;

/// Shared application state injected into every route handler.
///
/// `db` starts as an empty `OnceLock` while the process boots and waits for
/// RisingWave.  All HTTP routes that need the database call `get_db()` and
/// return 503 if the lock has not been filled yet.  `/healthz` always returns
/// 200 so the Railway health-check passes immediately after the TCP listener
/// binds.
#[derive(Clone)]
pub struct AppState {
    /// Set exactly once by the background setup task when RisingWave is ready.
    pub db: Arc<OnceLock<Db>>,
    /// Broadcast channel: each message is a JSON-serialised `FiredAlert`.
    pub alert_tx: tokio::sync::broadcast::Sender<String>,
}

impl AppState {
    /// Returns the live DB client, or `None` if startup is still in progress.
    pub fn get_db(&self) -> Option<Db> {
        self.db.get().cloned()
    }
}
