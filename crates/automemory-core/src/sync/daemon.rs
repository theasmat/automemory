use anyhow::Result;
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::Notify;
use tracing::{info, warn};

use crate::db::Database;

pub struct AutoSyncDaemon {
    db: Arc<Database>,
    vault_root: PathBuf,
    debounce_duration: Duration,
    pending: Arc<AtomicBool>,
    notifier: Arc<Notify>,
}

impl AutoSyncDaemon {
    pub fn new(db: Arc<Database>, vault_root: PathBuf, debounce_duration: Duration) -> Self {
        Self {
            db,
            vault_root,
            debounce_duration,
            pending: Arc::new(AtomicBool::new(false)),
            notifier: Arc::new(Notify::new()),
        }
    }

    /// Trigger a debounced sync pass (e.g. after event/memory activity)
    pub fn trigger(&self) {
        self.pending.store(true, Ordering::SeqCst);
        self.notifier.notify_one();
    }

    /// Run immediate sync without debouncing: pull, apply other devices' changes, write ours, push.
    pub fn sync_now(&self) -> Result<()> {
        let remote = crate::config::GlobalConfig::load()
            .ok()
            .and_then(|g| g.vault.remote_url)
            .filter(|u| !u.trim().is_empty());
        let key = super::auto::resolve_key(remote.is_some());
        let report = super::auto::run(&self.db, &self.vault_root, remote.as_deref(), true, true, key.as_ref())?;
        info!("vault sync: {}", report.summary());
        Ok(())
    }

    /// Continuous background loop with debouncing
    pub async fn run_loop(self: Arc<Self>) {
        loop {
            self.notifier.notified().await;

            // Wait for debounce period while new triggers might arrive
            tokio::time::sleep(self.debounce_duration).await;

            if self.pending.swap(false, Ordering::SeqCst) {
                info!("Executing debounced automated vault sync");
                if let Err(e) = self.sync_now() {
                    warn!("Automated vault sync error: {}", e);
                }
            }
        }
    }
}
