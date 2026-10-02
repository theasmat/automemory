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
}

fn run_migrations(conn: &Connection) {
    let _ = conn.execute("ALTER TABLE tracked_documents ADD COLUMN category TEXT DEFAULT 'general'", []);
    let _ = conn.execute("ALTER TABLE tracked_documents ADD COLUMN title TEXT DEFAULT ''", []);
    let _ = conn.execute("ALTER TABLE tracked_documents ADD COLUMN remote_synced_hash TEXT", []);
    let _ = conn.execute("ALTER TABLE tracked_documents ADD COLUMN l0_abstract TEXT", []);
    let _ = conn.execute("ALTER TABLE tracked_documents ADD COLUMN l1_overview TEXT", []);
}
