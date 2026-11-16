//! `automemory reset`: forget everything AutoMemory has stored, so a project (or a test) can start from nothing.
//! Removes the database and its journal files and the local vault copy. Keeps `config.toml` (settings and the human password), the device id,
//! the agent registrations `connect` wrote, and, unless asked, the backups. Nothing outside AutoMemory's own folder is touched.

use crate::db::Database;
use anyhow::{Context, Result};
use std::path::{Path, PathBuf};

/// What a reset would remove, for the confirmation screen.
#[derive(Debug, Default, PartialEq, Eq)]
pub struct Inventory {
    pub notes: i64,
    pub decisions: i64,
    pub projects: i64,
    pub backups: usize,
}

pub fn inventory(db_path: &Path, backups_dir: &Path) -> Inventory {
    let count = |sql: &str| -> i64 {
        rusqlite::Connection::open_with_flags(db_path, rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY)
            .ok()
            .and_then(|c| c.query_row(sql, [], |r| r.get(0)).ok())
            .unwrap_or(0)
    };
    Inventory {
        notes: count("SELECT COUNT(*) FROM memories WHERE status IN ('active','pinned','candidate')"),
        decisions: count("SELECT COUNT(*) FROM decisions"),
        projects: count("SELECT COUNT(*) FROM projects WHERE id != 'demo'"),
        backups: std::fs::read_dir(backups_dir).map(|d| d.flatten().count()).unwrap_or(0),
    }
}

fn remove(path: &Path, removed: &mut Vec<String>) -> Result<()> {
    if !path.exists() {
        return Ok(());
    }
    if path.is_dir() {
        std::fs::remove_dir_all(path)?;
    } else {
        std::fs::remove_file(path)?;
    }
    removed.push(path.display().to_string());
    Ok(())
}

/// Deletes the data. Refuses when another process (the daemon, an agent's MCP server) has the database open, and removes nothing then.
pub fn wipe(db_path: &Path, vault_dir: &Path, backups_dir: &Path, include_backups: bool) -> Result<Vec<String>> {
    if db_path.exists() {
        let db = Database::open(db_path)?;
        db.conn()
            .execute_batch("BEGIN EXCLUSIVE; ROLLBACK;")
            .context("the database is in use: stop `automemory serve` and close your agents first")?;
    }
    let mut removed = Vec::new();
    for suffix in ["", "-wal", "-shm"] {
        remove(&PathBuf::from(format!("{}{suffix}", db_path.display())), &mut removed)?;
    }
    // The vault holds synced copies of the same records: left behind, the next sync would bring everything back.
    if vault_dir.exists() {
        for entry in std::fs::read_dir(vault_dir)?.flatten() {
            remove(&entry.path(), &mut removed)?;
        }
    }
    if include_backups {
        remove(backups_dir, &mut removed)?;
    }
    Ok(removed)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::*;
    use chrono::Utc;

    fn note(db: &Database, id: &str) {
        db.save_memory(&Memory {
            id: id.into(), scope: MemoryScope::Global, project_id: None, memory_type: MemoryType::Fact, content: id.into(), summary: id.into(),
            confidence: 0.9, importance: 0.5, freshness: 1.0, status: MemoryStatus::Active, source_kind: MemorySourceKind::User, version: Some(1),
            created_at: Utc::now(), updated_at: Utc::now(), last_recalled_at: None, tags: vec![], entities: vec![], metadata: serde_json::json!({}),
        })
        .unwrap();
    }

    #[test]
    fn a_reset_removes_data_and_vault_and_keeps_config_and_backups() {
        let home = std::env::temp_dir().join(format!("am-reset-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&home);
        std::fs::create_dir_all(home.join("vault/deltas")).unwrap();
        std::fs::write(home.join("vault/deltas/x"), "secret").unwrap();
        std::fs::write(home.join("config.toml"), "keep me").unwrap();
        let (db_path, backups) = (home.join("automemory.db"), home.join("backups"));
        let db = Database::open(&db_path).unwrap();
        note(&db, "one");
        note(&db, "two");
        crate::backup::create(&db, &home).unwrap();
        drop(db);

        let inv = inventory(&db_path, &backups);
        assert_eq!((inv.notes, inv.backups), (2, 1));

        let removed = wipe(&db_path, &home.join("vault"), &backups, false).unwrap();
        assert!(removed.iter().any(|r| r.ends_with("automemory.db")) && removed.iter().any(|r| r.contains("vault")));
        assert!(!db_path.exists() && !home.join("vault/deltas").exists());
        assert_eq!(std::fs::read_to_string(home.join("config.toml")).unwrap(), "keep me", "settings and the password stay");
        assert!(backups.exists(), "backups stay unless asked");
        assert_eq!(inventory(&db_path, &backups).notes, 0);

        wipe(&db_path, &home.join("vault"), &backups, true).unwrap();
        assert!(!backups.exists(), "--include-backups removes them too");
        let _ = std::fs::remove_dir_all(&home);
    }

    #[test]
    fn a_database_in_use_is_not_wiped() {
        let home = std::env::temp_dir().join(format!("am-reset-busy-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&home);
        std::fs::create_dir_all(&home).unwrap();
        let db_path = home.join("automemory.db");
        let holder = Database::open(&db_path).unwrap();
        note(&holder, "one");
        holder.conn().execute_batch("BEGIN IMMEDIATE").unwrap();
        let err = wipe(&db_path, &home.join("vault"), &home.join("backups"), false);
        assert!(err.is_err() && db_path.exists(), "nothing is removed while another connection holds the database");
        let _ = std::fs::remove_dir_all(&home);
    }
}
