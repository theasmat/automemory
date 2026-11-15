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
    // the copy, not only the source, must be healthy before it replaces anything
    if let Err(e) = check(&tmp) {
        let _ = std::fs::remove_file(&tmp);
        return Err(e.context("the copy made for the restore is damaged; nothing was replaced"));
    }
    for ext in ["db-wal", "db-shm", "sqlite-wal", "sqlite-shm"] {
        let _ = std::fs::remove_file(db_path.with_extension(ext));
    }
    let _ = std::fs::remove_file(PathBuf::from(format!("{}-wal", db_path.display())));
    let _ = std::fs::remove_file(PathBuf::from(format!("{}-shm", db_path.display())));
    std::fs::rename(&tmp, db_path)?;
    Ok(saved)
}

/// What a restore drill found in the newest backup.
#[derive(Debug, Clone, serde::Serialize)]
pub struct DrillReport {
    pub file: PathBuf,
    pub notes: i64,
    pub decisions: i64,
    pub events: i64,
    pub schema_version: i64,
}

fn count(conn: &rusqlite::Connection, sql: &str) -> i64 {
    conn.query_row(sql, [], |r| r.get(0)).unwrap_or(0)
}

/// A restore drill: copy the newest backup to a scratch file, open it the way a restore would (integrity check, then a normal
/// open that applies any migration), and count what is in it. Nothing live is touched. Records the time in `drill.json`.
pub fn drill(home: &Path) -> Result<DrillReport> {
    let Some((file, ..)) = list(home).into_iter().find(|(f, ..)| f.file_name().is_some_and(|n| n.to_string_lossy().starts_with("automemory-"))) else {
        bail!("no backup to test yet: run `automemory backup` first");
    };
    check(&file)?;
    let scratch = home.join(format!("drill-{}.db", std::process::id()));
    let result = (|| -> Result<DrillReport> {
        std::fs::copy(&file, &scratch)?;
        let db = Database::open(&scratch).context("the backup does not open as a database of this version")?;
        let conn = db.conn();
        let report = DrillReport {
            file: file.clone(),
            notes: count(&conn, "SELECT COUNT(*) FROM memories WHERE status IN ('active','pinned')"),
            decisions: count(&conn, "SELECT COUNT(*) FROM decisions"),
            events: count(&conn, "SELECT COUNT(*) FROM tool_events"),
            schema_version: count(&conn, "PRAGMA user_version"),
        };
        Ok(report)
    })();
    for ext in ["", "-wal", "-shm"] {
        let _ = std::fs::remove_file(PathBuf::from(format!("{}{ext}", scratch.display())));
    }
    let report = result?;
    std::fs::create_dir_all(dir(home))?;
    std::fs::write(dir(home).join("drill.json"), serde_json::json!({ "at": Utc::now().to_rfc3339(), "file": report.file, "notes": report.notes }).to_string())?;
    Ok(report)
}

/// Days since the last successful drill, or None when there never was one.
pub fn days_since_drill(home: &Path) -> Option<i64> {
    let text = std::fs::read_to_string(dir(home).join("drill.json")).ok()?;
    let at = serde_json::from_str::<serde_json::Value>(&text).ok()?["at"].as_str()?.parse::<DateTime<Utc>>().ok()?;
    Some((Utc::now() - at).num_days())
}

/// For the daemon: saves a backup when the newest is over 20 hours old, and runs a drill when the last one is over 7 days old.
/// Returns what it did, for the log.
pub fn scheduled(db: &Database, home: &Path) -> Vec<String> {
    let mut did = Vec::new();
    let newest_age = list(home).into_iter().find(|(f, ..)| f.file_name().is_some_and(|n| n.to_string_lossy().starts_with("automemory-")))
        .map(|(_, _, when)| (Utc::now() - when).num_hours());
    if newest_age.is_none_or(|h| h >= 20) {
        match create(db, home) {
            Ok(f) => did.push(format!("backup saved: {}", f.display())),
            Err(e) => did.push(format!("backup failed: {e}")),
        }
    }
    if days_since_drill(home).is_none_or(|d| d >= 7) {
        match drill(home) {
            Ok(r) => did.push(format!("restore drill passed: {} notes, {} decisions, {} events", r.notes, r.decisions, r.events)),
            Err(e) => did.push(format!("restore drill FAILED: {e}")),
        }
    }
    did
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

    #[test]
    fn a_drill_reads_the_newest_backup_and_records_when() {
        let home = std::env::temp_dir().join(format!("am-drill-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&home);
        std::fs::create_dir_all(&home).unwrap();
        assert!(drill(&home).is_err(), "no backup yet");
        let db = Database::open(home.join("automemory.db")).unwrap();
        note(&db, "one");
        note(&db, "two");
        assert_eq!(days_since_drill(&home), None);
        let did = scheduled(&db, &home);
        assert!(did.iter().any(|d| d.starts_with("backup saved")) && did.iter().any(|d| d.starts_with("restore drill passed: 2 notes")), "{did:?}");
        assert_eq!(days_since_drill(&home), Some(0));
        assert!(scheduled(&db, &home).is_empty(), "nothing due right after");
        assert!(!home.join(format!("drill-{}.db", std::process::id())).exists(), "the scratch copy is removed");
        let _ = std::fs::remove_dir_all(&home);
    }
}
