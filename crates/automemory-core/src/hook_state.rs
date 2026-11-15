//! Small per-hook state: the replay guard for duplicate hook registrations and the notes waiting for a session.
//! (File locks and write tracking now live in Tracewise: docs/specs/tracewise-companion.md.)

use crate::db::Database;
use chrono::Utc;
use rusqlite::params;

/// Identity of one hook invocation, for [`Database::replay_or_claim_hook`].
pub fn hook_key(agent: &str, event: &str, input: &str) -> String {
    use sha2::{Digest, Sha256};
    format!("{:x}", Sha256::digest(format!("{agent}\0{event}\0{input}").as_bytes()))
}

impl Database {
    /// Keep a message for the session's agent until a hook that can show it runs.
    pub fn push_note(&self, session_id: &str, text: &str) {
        let _ = self.conn().execute(
            "INSERT INTO session_notes (session_id, ts, text) VALUES (?1, ?2, ?3)",
            params![session_id, Utc::now().to_rfc3339(), text],
        );
    }

    /// The session's waiting notes, oldest first; each is delivered once.
    pub fn drain_notes(&self, session_id: &str) -> Vec<String> {
        let conn = self.conn();
        let notes: Vec<String> = conn
            .prepare("SELECT text FROM session_notes WHERE session_id = ?1 ORDER BY id")
            .and_then(|mut s| s.query_map(params![session_id], |r| r.get(0)).map(|rows| rows.flatten().collect()))
            .unwrap_or_default();
        let _ = conn.execute("DELETE FROM session_notes WHERE session_id = ?1", params![session_id]);
        notes
    }

    /// Atomically claim a hook invocation. `Some(output)` means an identical one (same agent,
    /// event and payload) ran within the last moments and this is its reply: print it and stop.
    /// `None` means this process owns the invocation and must call [`Database::finish_hook`].
    pub fn replay_or_claim_hook(&self, key: &str) -> Option<String> {
        const WINDOW_MS: i64 = 1500;
        let now = Utc::now().timestamp_millis();
        let claimed = {
            let conn = self.conn();
            if conn.execute_batch("BEGIN IMMEDIATE").is_err() {
                return None;
            }
            let prior: Option<i64> = conn
                .query_row("SELECT ts FROM hook_claims WHERE key = ?1", params![key], |r| r.get(0))
                .ok();
            let fresh = prior.is_none_or(|ts| now - ts > WINDOW_MS);
            if fresh {
                let _ = conn.execute(
                    "INSERT INTO hook_claims (key, ts, output) VALUES (?1, ?2, NULL)
                     ON CONFLICT(key) DO UPDATE SET ts = ?2, output = NULL",
                    params![key, now],
                );
                let _ = conn.execute("DELETE FROM hook_claims WHERE ts < ?1", params![now - 60_000]);
            }
            let _ = conn.execute_batch("COMMIT");
            fresh
        };
        if claimed {
            return None;
        }
        // A twin is (or was just) running: wait briefly for its reply.
        for _ in 0..150 {
            let out: Option<String> = self
                .conn()
                .query_row("SELECT output FROM hook_claims WHERE key = ?1", params![key], |r| r.get(0))
                .ok()
                .flatten();
            if out.is_some() {
                return out;
            }
            std::thread::sleep(std::time::Duration::from_millis(20));
        }
        None
    }

    pub fn finish_hook(&self, key: &str, output: &str) {
        let _ = self.conn().execute("UPDATE hook_claims SET output = ?2 WHERE key = ?1", params![key, output]);
    }
}
