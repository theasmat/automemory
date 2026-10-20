//! The chat archive: redacted messages of imported sessions, searchable with FTS5. Never injected whole; agents reach it
//! through `chat.search` and `chat.get`.
use crate::db::Database;
use anyhow::Result;
use chrono::{DateTime, Utc};
use rusqlite::params;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize)]
pub struct ChatSession {
    pub id: String,
    pub agent: String,
    pub native_id: String,
    pub project_id: Option<String>,
    pub title: Option<String>,
    pub messages: i64,
    pub started_at: Option<String>,
    pub last_at: Option<String>,
    /// Opaque reader position: only what is new is read next time.
    pub cursor: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChatMessage {
    pub seq: i64,
    pub role: String,
    pub ts: Option<String>,
    pub text: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct ChatHit {
    pub session: String,
    pub agent: String,
    pub title: Option<String>,
    pub seq: i64,
    pub role: String,
    pub ts: Option<String>,
    pub snippet: String,
}

pub fn session_id(agent: &str, native_id: &str) -> String {
    format!("{agent}:{native_id}")
}

/// FTS5 query from free text: every word as a prefix term (same rule as note search).
fn fts_query(q: &str) -> String {
    q.split(|c: char| !(c.is_alphanumeric() || c == '_')).filter(|w| !w.is_empty()).map(|w| format!("\"{w}\"*")).collect::<Vec<_>>().join(" ")
}

impl Database {
    pub fn chat_session(&self, id: &str) -> Result<Option<ChatSession>> {
        let conn = self.conn();
        let mut stmt = conn.prepare("SELECT id, agent, native_id, project_id, title, messages, started_at, last_at, cursor FROM chat_sessions WHERE id = ?1")?;
        let mut rows = stmt.query_map([id], |r| Ok(ChatSession { id: r.get(0)?, agent: r.get(1)?, native_id: r.get(2)?, project_id: r.get(3)?, title: r.get(4)?, messages: r.get(5)?, started_at: r.get(6)?, last_at: r.get(7)?, cursor: r.get(8)? }))?;
        Ok(rows.next().transpose()?)
    }

    pub fn chat_sessions(&self, project_id: Option<&str>, agent: Option<&str>) -> Result<Vec<ChatSession>> {
        let conn = self.conn();
        let mut stmt = conn.prepare(
            "SELECT id, agent, native_id, project_id, title, messages, started_at, last_at, cursor FROM chat_sessions
             WHERE (?1 IS NULL OR project_id = ?1) AND (?2 IS NULL OR agent = ?2) ORDER BY last_at DESC",
        )?;
        let rows = stmt.query_map(params![project_id, agent], |r| Ok(ChatSession { id: r.get(0)?, agent: r.get(1)?, native_id: r.get(2)?, project_id: r.get(3)?, title: r.get(4)?, messages: r.get(5)?, started_at: r.get(6)?, last_at: r.get(7)?, cursor: r.get(8)? }))?
            .collect::<std::result::Result<Vec<_>, _>>()?;
        Ok(rows)
    }

    /// Appends already-redacted messages to a session (creating it) and moves its cursor, all or nothing.
    pub fn chat_append(&self, agent: &str, native_id: &str, project_id: Option<&str>, title: Option<&str>, cursor: &str, msgs: &[(String, Option<DateTime<Utc>>, String)]) -> Result<usize> {
        let id = session_id(agent, native_id);
        let mut conn = self.conn();
        let tx = conn.transaction()?;
        tx.execute(
            "INSERT INTO chat_sessions (id, agent, native_id, project_id, title, messages, started_at, last_at, cursor, imported_at)
             VALUES (?1, ?2, ?3, ?4, ?5, 0, NULL, NULL, ?6, ?7)
             ON CONFLICT(id) DO UPDATE SET project_id = COALESCE(excluded.project_id, project_id), title = COALESCE(excluded.title, title), cursor = excluded.cursor, imported_at = excluded.imported_at",
            params![id, agent, native_id, project_id, title, cursor, Utc::now().to_rfc3339()],
        )?;
        let mut seq: i64 = tx.query_row("SELECT COALESCE(MAX(seq), -1) FROM chat_messages WHERE session_id = ?1", [&id], |r| r.get(0))?;
        for (role, ts, text) in msgs {
            seq += 1;
            tx.execute("INSERT INTO chat_messages (session_id, seq, role, ts, text) VALUES (?1, ?2, ?3, ?4, ?5)", params![id, seq, role, ts.map(|t| t.to_rfc3339()), text])?;
            let rowid = tx.last_insert_rowid();
            tx.execute("INSERT INTO chat_fts (rowid, text) VALUES (?1, ?2)", params![rowid, text])?;
        }
        if let (Some(first), Some(last)) = (msgs.iter().find_map(|m| m.1), msgs.iter().rev().find_map(|m| m.1)) {
            tx.execute(
                "UPDATE chat_sessions SET started_at = COALESCE(started_at, ?2), last_at = MAX(COALESCE(last_at, ''), ?3) WHERE id = ?1",
                params![id, first.to_rfc3339(), last.to_rfc3339()],
            )?;
        }
        tx.execute("UPDATE chat_sessions SET messages = (SELECT COUNT(*) FROM chat_messages WHERE session_id = ?1) WHERE id = ?1", [&id])?;
        tx.commit()?;
        Ok(msgs.len())
    }

    /// Words of the conversation itself: the one-line notes of tools an agent ran are kept for context (chat.get) but not searched.
    pub fn chat_search(&self, query: &str, project_id: Option<&str>, agent: Option<&str>, limit: usize) -> Result<Vec<ChatHit>> {
        let q = fts_query(query);
        if q.is_empty() {
            return Ok(vec![]);
        }
        let conn = self.conn();
        let mut stmt = conn.prepare(
            "SELECT s.id, s.agent, s.title, m.seq, m.role, m.ts, snippet(chat_fts, 0, '[', ']', '…', 24)
             FROM chat_fts JOIN chat_messages m ON m.id = chat_fts.rowid JOIN chat_sessions s ON s.id = m.session_id
             WHERE chat_fts MATCH ?1 AND m.role != 'tool' AND (?2 IS NULL OR s.project_id = ?2) AND (?3 IS NULL OR s.agent = ?3)
             ORDER BY rank LIMIT ?4",
        )?;
        let rows = stmt.query_map(params![q, project_id, agent, limit as i64], |r| Ok(ChatHit { session: r.get(0)?, agent: r.get(1)?, title: r.get(2)?, seq: r.get(3)?, role: r.get(4)?, ts: r.get(5)?, snippet: r.get(6)? }))?
            .collect::<std::result::Result<Vec<_>, _>>()?;
        Ok(rows)
    }

    /// Messages `from_seq..` of one session (at most `limit`), in order.
    pub fn chat_get(&self, session: &str, from_seq: i64, limit: usize) -> Result<Vec<ChatMessage>> {
        let conn = self.conn();
        let mut stmt = conn.prepare("SELECT seq, role, ts, text FROM chat_messages WHERE session_id = ?1 AND seq >= ?2 ORDER BY seq LIMIT ?3")?;
        let rows = stmt.query_map(params![session, from_seq, limit as i64], |r| Ok(ChatMessage { seq: r.get(0)?, role: r.get(1)?, ts: r.get(2)?, text: r.get(3)? }))?
            .collect::<std::result::Result<Vec<_>, _>>()?;
        Ok(rows)
    }

    /// Deletes archived chat (all of it, one agent's, or one project's). Returns the messages removed.
    pub fn chat_forget(&self, project_id: Option<&str>, agent: Option<&str>) -> Result<usize> {
        let mut conn = self.conn();
        let tx = conn.transaction()?;
        let ids: Vec<String> = {
            let mut stmt = tx.prepare("SELECT id FROM chat_sessions WHERE (?1 IS NULL OR project_id = ?1) AND (?2 IS NULL OR agent = ?2)")?;
            let rows = stmt.query_map(params![project_id, agent], |r| r.get(0))?.collect::<std::result::Result<Vec<String>, _>>()?;
            rows
        };
        let mut removed = 0;
        for id in &ids {
            tx.execute("DELETE FROM chat_fts WHERE rowid IN (SELECT id FROM chat_messages WHERE session_id = ?1)", [id])?;
            removed += tx.execute("DELETE FROM chat_messages WHERE session_id = ?1", [id])?;
            tx.execute("DELETE FROM chat_sessions WHERE id = ?1", [id])?;
        }
        tx.commit()?;
        Ok(removed)
    }
}
