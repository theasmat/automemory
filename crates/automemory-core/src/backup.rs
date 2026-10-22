//! Database backups: create (keeping the newest few), list, check that one opens, and restore one.
//! Restore keeps the current database as a `before-restore-*` copy, so it can itself be undone.
use crate::db::Database;
use anyhow::{bail, Context, Result};
use chrono::{DateTime, Utc};
use std::path::{Path, PathBuf};

const KEEP: usize = 10;

pub fn dir(home: &Path) -> PathBuf {
    home.join("backups")
}

/// Saves a copy as `automemory-<time>.db` and deletes all but the newest 10 such copies.
pub fn create(db: &Database, home: &Path) -> Result<PathBuf> {
    let dir = dir(home);
    std::fs::create_dir_all(&dir)?;
    let dest = dir.join(format!("automemory-{}.db", Utc::now().format("%Y%m%d-%H%M%S")));
    db.backup_to(&dest)?;
    let mut all = named(&dir, "automemory-");
    all.sort();
    for old in all.iter().rev().skip(KEEP) {
        let _ = std::fs::remove_file(old);
    }
    Ok(dest)
}

fn named(dir: &Path, prefix: &str) -> Vec<PathBuf> {
    std::fs::read_dir(dir).into_iter().flatten().flatten().map(|e| e.path())
        .filter(|p| p.extension().is_some_and(|x| x == "db") && p.file_name().is_some_and(|n| n.to_string_lossy().starts_with(prefix)))
        .collect()
}

/// Newest first: (file, size in bytes, modified time).
pub fn list(home: &Path) -> Vec<(PathBuf, u64, DateTime<Utc>)> {
    let mut v: Vec<(PathBuf, u64, DateTime<Utc>)> = std::fs::read_dir(dir(home)).into_iter().flatten().flatten()
        .filter(|e| e.path().extension().is_some_and(|x| x == "db"))
        .filter_map(|e| {
            let m = e.metadata().ok()?;
            Some((e.path(), m.len(), m.modified().ok()?.into()))
        })
        .collect();
    v.sort_by_key(|e| std::cmp::Reverse(e.2));
    v
}

/// Opens the file read-only and returns how many active memories it holds; errors if it is not a healthy database.
pub fn check(file: &Path) -> Result<i64> {
    let conn = rusqlite::Connection::open_with_flags(file, rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY)
        .with_context(|| format!("cannot open {}", file.display()))?;
    let verdict: String = conn.query_row("PRAGMA quick_check", [], |r| r.get(0))?;
    if verdict != "ok" {
        bail!("{} is damaged: {verdict}", file.display());
    }
    Ok(conn.query_row("SELECT COUNT(*) FROM memories WHERE status IN ('active','pinned')", [], |r| r.get(0))?)
}

/// Replaces the database at `db_path` with `backup`. Refuses while another process (the daemon, an agent's MCP
/// server) has it open, and saves the current database first. Returns where that copy went.
pub fn restore(db_path: &Path, backup: &Path, home: &Path) -> Result<PathBuf> {
    check(backup)?;
    let saved = dir(home).join(format!("before-restore-{}.db", Utc::now().format("%Y%m%d-%H%M%S")));
    if db_path.exists() {
        let current = Database::open(db_path)?;
        current.conn().execute_batch("BEGIN EXCLUSIVE; ROLLBACK;")
            .context("the database is in use: stop `automemory serve` and close your agents first")?;
        current.backup_to(&saved)?;
        drop(current);
    }
    let tmp = db_path.with_extension("restoring");
    std::fs::copy(backup, &tmp)?;
    for ext in ["db-wal", "db-shm", "sqlite-wal", "sqlite-shm"] {
        let _ = std::fs::remove_file(db_path.with_extension(ext));
    }
    let _ = std::fs::remove_file(PathBuf::from(format!("{}-wal", db_path.display())));
    let _ = std::fs::remove_file(PathBuf::from(format!("{}-shm", db_path.display())));
    std::fs::rename(&tmp, db_path)?;
    Ok(saved)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::*;

    fn note(db: &Database, id: &str) {
        db.save_memory(&Memory {
            id: id.into(), scope: MemoryScope::Global, project_id: None, memory_type: MemoryType::Fact,
            content: id.into(), summary: id.into(), confidence: 0.9, importance: 0.5, freshness: 1.0,
            status: MemoryStatus::Active, source_kind: MemorySourceKind::User, version: Some(1),
            created_at: Utc::now(), updated_at: Utc::now(), last_recalled_at: None, tags: vec![], entities: vec![],
            metadata: serde_json::json!({}),
        })
        .unwrap();
    }

    #[test]
    fn restore_round_trip_keeps_the_old_database_and_refuses_bad_files() {
        let home = std::env::temp_dir().join(format!("am-backup-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&home);
        std::fs::create_dir_all(&home).unwrap();
        let path = home.join("automemory.db");
        let db = Database::open(&path).unwrap();
        note(&db, "one");
        let backup = create(&db, &home).unwrap();
        note(&db, "two");
        drop(db);
        assert_eq!(check(&backup).unwrap(), 1);

        let junk = home.join("junk.db");
        std::fs::write(&junk, "not a database").unwrap();
        assert!(restore(&path, &junk, &home).is_err());
        assert_eq!(check(&path).unwrap(), 2, "a refused restore changes nothing");

        let saved = restore(&path, &backup, &home).unwrap();
        assert_eq!(check(&path).unwrap(), 1, "back to the backup");
        assert_eq!(check(&saved).unwrap(), 2, "the replaced database was kept");
        assert!(list(&home).len() >= 2);
        let _ = std::fs::remove_dir_all(&home);
    }
}
