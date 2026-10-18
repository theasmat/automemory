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

        // Hooks open the database on every tool call, so the ~50 CREATE IF NOT EXISTS
        // statements only run while the schema is behind (new or upgraded databases).
        // A schema.sql change therefore must bump LATEST_SCHEMA_VERSION with a migration.
        if schema_version(&conn) < LATEST_SCHEMA_VERSION {
            conn.execute_batch(SCHEMA_SQL)
                .context("Failed to execute database schema")?;
            run_migrations(&conn).context("Failed to migrate database schema")?;
        }

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
        run_migrations(&conn).context("Failed to migrate database schema")?;

        Ok(Self {
            conn: Arc::new(Mutex::new(conn)),
        })
    }

    pub fn conn(&self) -> std::sync::MutexGuard<'_, Connection> {
        // A panic in another thread poisons the mutex but leaves the SQLite handle
        // intact; keep serving instead of cascading the panic through the daemon.
        self.conn
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
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

fn schema_version(conn: &Connection) -> i32 {
    conn.query_row("PRAGMA user_version", [], |r| r.get(0)).unwrap_or(0)
}

/// Newest schema version; bump with each new `if version < N` step below.
const LATEST_SCHEMA_VERSION: i32 = 12;

/// Runs pending migrations atomically. Hooks are separate processes that may all
/// open the database on first use after an upgrade, so the work happens under
/// `BEGIN IMMEDIATE` and the version is bumped only if every step succeeded.
fn run_migrations(conn: &Connection) -> rusqlite::Result<()> {
    let version: i32 = conn.query_row("PRAGMA user_version", [], |r| r.get(0))?;
    if version >= LATEST_SCHEMA_VERSION {
        return Ok(());
    }
    conn.execute_batch("BEGIN IMMEDIATE")?;
    let result = apply_migrations(conn);
    conn.execute_batch(if result.is_ok() { "COMMIT" } else { "ROLLBACK" })?;
    result
}

/// One migration statement. A repeated `ADD COLUMN` is fine (the step already ran);
/// any other failure aborts the whole migration instead of being swallowed.
fn step(conn: &Connection, sql: &str, _no_params: [i32; 0]) -> rusqlite::Result<()> {
    match conn.execute_batch(sql) {
        Ok(()) => Ok(()),
        Err(e) if e.to_string().contains("duplicate column name") => Ok(()),
        Err(e) => Err(e),
    }
}

fn apply_migrations(conn: &Connection) -> rusqlite::Result<()> {
    let version: i32 = conn
        .query_row("PRAGMA user_version", [], |r| r.get(0))
        .unwrap_or(0);

    if version < 2 {
        step(
            conn,
            "ALTER TABLE tracked_documents ADD COLUMN category TEXT DEFAULT 'general'",
            [],
        )?;
        step(
            conn,
            "ALTER TABLE tracked_documents ADD COLUMN title TEXT DEFAULT ''",
            [],
        )?;
        step(
            conn,
            "ALTER TABLE tracked_documents ADD COLUMN remote_synced_hash TEXT",
            [],
        )?;
        step(
            conn,
            "ALTER TABLE tracked_documents ADD COLUMN l0_abstract TEXT",
            [],
        )?;
        step(
            conn,
            "ALTER TABLE tracked_documents ADD COLUMN l1_overview TEXT",
            [],
        )?;
        step(
            conn,
            "CREATE INDEX IF NOT EXISTS idx_tool_events_timestamp ON tool_events(timestamp)",
            [],
        )?;
        step(conn, 
            "CREATE INDEX IF NOT EXISTS idx_tool_events_proj_time ON tool_events(project_id, timestamp)",
            [],
        )?;
        step(conn, 
            "CREATE INDEX IF NOT EXISTS idx_memories_scope_summary ON memories(scope, summary, project_id)",
            [],
        )?;
        // Deduplicate existing redundant active memories in the same scope
        step(
            conn,
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
        )?;
        step(conn, "PRAGMA user_version = 2", [])?;
    }

    if version < 3 {
        step(
            conn,
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
        )?;
        step(conn, "PRAGMA user_version = 3", [])?;
    }

    if version < 4 {
        step(
            conn,
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
        )?;
        step(conn, "PRAGMA user_version = 4", [])?;
    }

    if version < 5 {
        step(
            conn,
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
        )?;
        step(conn, "PRAGMA user_version = 5", [])?;
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
            step(conn, ddl, [])?;
        }
        step(conn, "PRAGMA user_version = 6", [])?;
    }

    if version < 7 {
        step(
            conn,
            r#"
            CREATE TABLE IF NOT EXISTS reviews (
                id TEXT PRIMARY KEY,
                project_id TEXT,
                task_id TEXT,
                author_session TEXT NOT NULL,
                title TEXT NOT NULL,
                diff TEXT NOT NULL,
                test_command TEXT,
                status TEXT NOT NULL DEFAULT 'pending',
                reviewer_session TEXT,
                tests_passed INTEGER,
                verdict TEXT,
                created_at TEXT NOT NULL,
                updated_at TEXT NOT NULL
            );
            CREATE INDEX IF NOT EXISTS idx_reviews_queue ON reviews(project_id, status, created_at);
            "#,
            [],
        )?;
        step(conn, "PRAGMA user_version = 7", [])?;
    }

    if version < 8 {
        step(
            conn,
            r#"
            CREATE TABLE IF NOT EXISTS rooms (
                id TEXT PRIMARY KEY,
                project_id TEXT,
                topic TEXT NOT NULL,
                state TEXT NOT NULL DEFAULT 'open',
                members TEXT NOT NULL DEFAULT '[]',
                turn_holder TEXT,
                round INTEGER NOT NULL DEFAULT 0,
                created_at TEXT NOT NULL,
                updated_at TEXT NOT NULL
            );
            CREATE TABLE IF NOT EXISTS room_posts (
                id TEXT PRIMARY KEY,
                room_id TEXT NOT NULL,
                author_session TEXT NOT NULL,
                kind TEXT NOT NULL,
                body TEXT NOT NULL,
                evidence TEXT NOT NULL DEFAULT '[]',
                created_at TEXT NOT NULL,
                FOREIGN KEY(room_id) REFERENCES rooms(id) ON DELETE CASCADE
            );
            CREATE INDEX IF NOT EXISTS idx_rooms_turn ON rooms(project_id, state, turn_holder);
            CREATE INDEX IF NOT EXISTS idx_room_posts_room ON room_posts(room_id, created_at);
            "#,
            [],
        )?;
        step(conn, "PRAGMA user_version = 8", [])?;
    }

    if version < 9 {
        step(
            conn,
            r#"
            CREATE TABLE IF NOT EXISTS sync_kv (
                key TEXT PRIMARY KEY,
                value TEXT NOT NULL
            );
            "#,
            [],
        )?;
        step(conn, "PRAGMA user_version = 9", [])?;
    }

    if version < 10 {
        step(
            conn,
            r#"
            CREATE TABLE IF NOT EXISTS workspace_snapshots (
                session_id TEXT PRIMARY KEY,
                project_id TEXT,
                taken_at TEXT NOT NULL,
                in_flight INTEGER NOT NULL DEFAULT 1,
                data TEXT NOT NULL
            );
            CREATE TABLE IF NOT EXISTS lock_coverage (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                ts TEXT NOT NULL,
                project_id TEXT,
                session_id TEXT NOT NULL,
                path TEXT NOT NULL,
                source TEXT NOT NULL
            );
            CREATE INDEX IF NOT EXISTS idx_lock_coverage_ts ON lock_coverage(ts);
            CREATE TABLE IF NOT EXISTS lock_alerts (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                ts TEXT NOT NULL,
                project_id TEXT,
                path TEXT NOT NULL,
                holder_agent TEXT NOT NULL,
                holder_session TEXT NOT NULL,
                message TEXT NOT NULL
            );
            "#,
            [],
        )?;
        step(conn, "PRAGMA user_version = 10", [])?;
    }

    if version < 11 {
        step(
            conn,
            r#"
            CREATE TABLE IF NOT EXISTS session_notes (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                session_id TEXT NOT NULL,
                ts TEXT NOT NULL,
                text TEXT NOT NULL
            );
            CREATE INDEX IF NOT EXISTS idx_session_notes_session ON session_notes(session_id);
            "#,
            [],
        )?;
        step(conn, "PRAGMA user_version = 11", [])?;
    }

    if version < 12 {
        step(
            conn,
            "CREATE TABLE IF NOT EXISTS hook_claims (key TEXT PRIMARY KEY, ts INTEGER NOT NULL, output TEXT);",
            [],
        )?;
        step(conn, "PRAGMA user_version = 12", [])?;
    }
    Ok(())
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

    #[test]
    fn migration_step_tolerates_repeat_add_column_but_not_other_errors() {
        let conn = Connection::open_in_memory().unwrap();
        conn.execute_batch("CREATE TABLE t (a INTEGER)").unwrap();
        step(&conn, "ALTER TABLE t ADD COLUMN b TEXT", []).unwrap();
        step(&conn, "ALTER TABLE t ADD COLUMN b TEXT", []).unwrap(); // already applied
        assert!(step(&conn, "ALTER TABLE missing ADD COLUMN c TEXT", []).is_err());
    }

    #[test]
    fn a_fresh_database_reports_the_latest_schema_version() {
        // If this fails, schema.sql or a migration changed without bumping LATEST_SCHEMA_VERSION:
        // existing databases would then skip the new schema.
        let db = Database::open_in_memory().unwrap();
        assert_eq!(schema_version(&db.conn()), LATEST_SCHEMA_VERSION);
    }
}
