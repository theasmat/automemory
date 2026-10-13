use anyhow::{Context, Result};
use rusqlite::Connection;
use std::path::Path;
use std::sync::{Arc, Mutex};

const SCHEMA_SQL: &str = include_str!("schema.sql");

#[derive(Clone)]
pub struct Database {
    conn: Arc<Mutex<Connection>>,
}

impl Database {
    pub fn open<P: AsRef<Path>>(path: P) -> Result<Self> {
        let path_ref = path.as_ref();
        if let Some(parent) = path_ref.parent() {
            std::fs::create_dir_all(parent)
                .with_context(|| format!("Failed to create parent directory {:?}", parent))?;
        }

        let conn = Connection::open(path_ref)
            .with_context(|| format!("Failed to open SQLite database at {:?}", path_ref))?;

        // Configure WAL mode and robust pragmas
        conn.busy_timeout(std::time::Duration::from_millis(5000))
            .context("Failed to set busy timeout")?;
        conn.pragma_update(None, "foreign_keys", "ON")
            .context("Failed to enable foreign keys")?;
        let _: String = conn
            .query_row("PRAGMA journal_mode = WAL", [], |r| r.get(0))
            .context("Failed to set WAL mode")?;
        conn.pragma_update(None, "synchronous", "NORMAL")
            .context("Failed to set synchronous mode")?;

        // Initialize schema
        conn.execute_batch(SCHEMA_SQL)
            .context("Failed to execute database schema")?;
        run_migrations(&conn);

        Ok(Self {
            conn: Arc::new(Mutex::new(conn)),
        })
    }

    pub fn open_in_memory() -> Result<Self> {
        let conn = Connection::open_in_memory().context("Failed to open in-memory SQLite")?;
        conn.busy_timeout(std::time::Duration::from_millis(5000))?;
        conn.pragma_update(None, "foreign_keys", "ON")?;
        conn.execute_batch(SCHEMA_SQL)
            .context("Failed to execute schema in memory")?;
        run_migrations(&conn);

        Ok(Self {
            conn: Arc::new(Mutex::new(conn)),
        })
    }

    pub fn conn(&self) -> std::sync::MutexGuard<'_, Connection> {
        self.conn.lock().expect("SQLite mutex poisoned")
    }

    /// Snapshot the live database to `dest` with `VACUUM INTO` (compact + defragmented copy).
    pub fn backup_to<P: AsRef<Path>>(&self, dest: P) -> Result<()> {
        let dest_ref = dest.as_ref();
        if let Some(parent) = dest_ref.parent() {
            std::fs::create_dir_all(parent)
                .with_context(|| format!("Failed to create backup directory {:?}", parent))?;
        }
        // VACUUM INTO requires a fresh target file.
        let _ = std::fs::remove_file(dest_ref);
        let dest_str = dest_ref.to_string_lossy().to_string();
        self.conn()
            .execute("VACUUM INTO ?1", [dest_str])
            .with_context(|| format!("Failed to back up database to {:?}", dest_ref))?;
        Ok(())
    }
}

fn run_migrations(conn: &Connection) {
    let version: i32 = conn
        .query_row("PRAGMA user_version", [], |r| r.get(0))
        .unwrap_or(0);

    if version < 2 {
        let _ = conn.execute(
            "ALTER TABLE tracked_documents ADD COLUMN category TEXT DEFAULT 'general'",
            [],
        );
        let _ = conn.execute(
            "ALTER TABLE tracked_documents ADD COLUMN title TEXT DEFAULT ''",
            [],
        );
        let _ = conn.execute(
            "ALTER TABLE tracked_documents ADD COLUMN remote_synced_hash TEXT",
            [],
        );
        let _ = conn.execute(
            "ALTER TABLE tracked_documents ADD COLUMN l0_abstract TEXT",
            [],
        );
        let _ = conn.execute(
            "ALTER TABLE tracked_documents ADD COLUMN l1_overview TEXT",
            [],
        );
        let _ = conn.execute(
            "CREATE INDEX IF NOT EXISTS idx_tool_events_timestamp ON tool_events(timestamp)",
            [],
        );
        let _ = conn.execute(
            "CREATE INDEX IF NOT EXISTS idx_tool_events_proj_time ON tool_events(project_id, timestamp)",
            [],
        );
        let _ = conn.execute(
            "CREATE INDEX IF NOT EXISTS idx_memories_scope_summary ON memories(scope, summary, project_id)",
            [],
        );
        // Deduplicate existing redundant active memories in the same scope
        let _ = conn.execute(
            r#"
            DELETE FROM memories WHERE id NOT IN (
                SELECT id FROM (
                    SELECT id, ROW_NUMBER() OVER (
                        PARTITION BY COALESCE(project_id, ''), scope, summary
                        ORDER BY updated_at DESC, created_at DESC
                    ) as rn
                    FROM memories
                    WHERE status = 'active'
                ) WHERE rn = 1
            ) AND status = 'active'
            "#,
            [],
        );
        let _ = conn.execute("PRAGMA user_version = 2", []);
    }

    if version < 3 {
        let _ = conn.execute(
            r#"
            CREATE TABLE IF NOT EXISTS leases (
                id TEXT PRIMARY KEY,
                project_id TEXT,
                agent TEXT NOT NULL,
                session_id TEXT NOT NULL,
                scope TEXT NOT NULL,
                scope_type TEXT NOT NULL DEFAULT 'file',
                goal TEXT NOT NULL,
                ttl_seconds INTEGER NOT NULL DEFAULT 300,
                acquired_at TEXT NOT NULL,
                heartbeat_at TEXT NOT NULL,
                expires_at TEXT NOT NULL
            );
            CREATE INDEX IF NOT EXISTS idx_leases_scope ON leases(project_id, scope);
            CREATE INDEX IF NOT EXISTS idx_leases_expires ON leases(expires_at);
            CREATE INDEX IF NOT EXISTS idx_leases_session ON leases(session_id);
            "#,
            [],
        );
        let _ = conn.execute("PRAGMA user_version = 3", []);
    }

    if version < 4 {
        let _ = conn.execute(
            r#"
            CREATE TABLE IF NOT EXISTS decisions (
                id TEXT PRIMARY KEY,
                project_id TEXT,
                topic TEXT NOT NULL,
                decision TEXT NOT NULL,
                reason TEXT NOT NULL,
                alternatives TEXT NOT NULL DEFAULT '[]',
                dissent TEXT,
                agent TEXT NOT NULL,
                symbols TEXT NOT NULL DEFAULT '[]',
                evidence TEXT NOT NULL DEFAULT '[]',
                status TEXT NOT NULL DEFAULT 'active',
                superseded_by TEXT,
                created_at TEXT NOT NULL,
                updated_at TEXT NOT NULL
            );
            CREATE INDEX IF NOT EXISTS idx_decisions_topic ON decisions(project_id, topic);
            CREATE INDEX IF NOT EXISTS idx_decisions_status ON decisions(status);
            "#,
            [],
        );
        let _ = conn.execute("PRAGMA user_version = 4", []);
    }

    if version < 5 {
        let _ = conn.execute(
            r#"
            CREATE TABLE IF NOT EXISTS batons (
                id TEXT PRIMARY KEY,
                project_id TEXT,
                from_session TEXT NOT NULL,
                to_session TEXT,
                agent TEXT NOT NULL,
                packet TEXT NOT NULL,
                created_at TEXT NOT NULL
            );
            CREATE INDEX IF NOT EXISTS idx_batons_pickup ON batons(project_id, to_session, created_at);
            "#,
            [],
        );
        let _ = conn.execute("PRAGMA user_version = 5", []);
    }

    if version < 6 {
        for ddl in [
            "ALTER TABLE tasks ADD COLUMN spec TEXT NOT NULL DEFAULT ''",
            "ALTER TABLE tasks ADD COLUMN symbols TEXT NOT NULL DEFAULT '[]'",
            "ALTER TABLE tasks ADD COLUMN owner_session TEXT",
            "ALTER TABLE tasks ADD COLUMN result TEXT",
            "CREATE INDEX IF NOT EXISTS idx_tasks_status_created ON tasks(status, created_at)",
            "CREATE INDEX IF NOT EXISTS idx_tasks_owner ON tasks(owner_session)",
        ] {
            let _ = conn.execute(ddl, []);
        }
        let _ = conn.execute("PRAGMA user_version = 6", []);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_backup_to_creates_snapshot() {
        let db = Database::open_in_memory().expect("in-memory db open failed");
        let dest =
            std::env::temp_dir().join(format!("automemory-test-backup-{}.db", std::process::id()));
        let _ = std::fs::remove_file(&dest);
        db.backup_to(&dest).expect("backup_to failed");
        assert!(dest.exists(), "backup file must exist");
        let size = dest.metadata().expect("backup metadata").len();
        assert!(size > 0, "backup must be non-empty");
        let _ = std::fs::remove_file(&dest);
    }
}
