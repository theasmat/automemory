use automemory_core::db::Database;
use automemory_core::sync::AutoSyncDaemon;
use serde::Deserialize;
use std::sync::Arc;

#[derive(Clone)]
pub struct AppState {
    pub db: Arc<Database>,
    pub sync_daemon: Arc<AutoSyncDaemon>,
    pub broadcast_tx: tokio::sync::broadcast::Sender<String>,
}

#[derive(Deserialize)]
pub struct SearchParams {
    pub q: Option<String>,
    pub project_id: Option<String>,
    pub scope: Option<String>,
}
