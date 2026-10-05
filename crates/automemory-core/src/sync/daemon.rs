use anyhow::Result;
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::Notify;
use tracing::{info, warn};

use crate::db::Database;
use crate::sync::git::GitSync;
use crate::sync::vault::VaultManager;

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

    /// Run immediate sync without debouncing
    pub fn sync_now(&self) -> Result<()> {
        let vault = VaultManager::new(&self.db, &self.vault_root);
        vault.export_to_vault()?;

        if GitSync::is_git_repo(&self.vault_root) {
            let committed =
                GitSync::commit_all(&self.vault_root, "AutoMemory: automated vault sync")?;
            if committed {
                info!("Committed vault updates to local git");
                if let Err(e) = GitSync::push(&self.vault_root, None, None) {
                    warn!(
                        "Git push failed (might be offline or remote not configured): {}",
                        e
                    );
                }
            }
        }
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
