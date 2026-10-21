//! Library files in SQLite: revisions, chunked full-text search, and the rules (secrets refused, a person's file is not overwritten by an agent).
use super::model::*;
use super::tiers;
use crate::db::Database;
use crate::redaction::contains_secrets;
use anyhow::{bail, Result};
use chrono::{DateTime, Utc};
use rusqlite::{params, OptionalExtension};
use serde::Serialize;
use sha2::{Digest, Sha256};
use uuid::Uuid;

const COLS: &str = "id, project_id, path, origin, author, scope, size, sha256, content, l0, l1, created_at, updated_at, link";

fn when(s: String) -> DateTime<Utc> {
    DateTime::parse_from_rfc3339(&s).map(|d| d.with_timezone(&Utc)).unwrap_or_else(|_| Utc::now())
}

fn row(r: &rusqlite::Row) -> rusqlite::Result<LibraryFile> {
    Ok(LibraryFile {
        id: r.get(0)?,
        project_id: r.get(1)?,
        path: r.get(2)?,
        origin: Origin::parse(&r.get::<_, String>(3)?),
        author: r.get(4)?,
        scope: r.get(5)?,
        size: r.get::<_, i64>(6)? as usize,
        sha256: r.get(7)?,
        content: r.get(8)?,
        l0: r.get(9)?,
        l1: r.get(10)?,
        revisions: 0,
        linked: r.get::<_, i64>(13)? != 0,
        created_at: when(r.get(11)?),
        updated_at: when(r.get(12)?),
    })
}

/// A relative path with `/` separators, no `..`, no hidden folders, an allowed extension. Returns the cleaned path.
pub fn clean_path(path: &str) -> Result<String> {
    let p = path.trim().replace('\\', "/");
    let p = p.trim_start_matches("./").trim_matches('/').to_string();
    if p.is_empty() || p.len() > 200 {
        bail!("the path must be 1 to 200 characters");
    }
    if path.trim().starts_with('/') || p.split('/').any(|s| s.is_empty() || s == ".." || s == "." || s.starts_with('.') || s.contains(':')) {
        bail!("use a plain relative path such as plans/prd.md (no '..', no hidden folders, no drive letters)");
    }
    let ext = p.rsplit('.').next().unwrap_or("").to_lowercase();
    if !p.contains('.') || !EXTENSIONS.contains(&ext.as_str()) {
        bail!("only text files are kept: {}", EXTENSIONS.join(", "));
    }
    Ok(p)
}

/// For a file kept in place: a relative path inside the project; hidden folders are fine, `.git`, `.automemory` and `..` are not.
pub fn clean_linked_path(path: &str) -> Result<String> {
    let p = path.trim().replace('\\', "/");
    let p = p.trim_start_matches("./").to_string();
    if p.is_empty() || p.len() > 300 || p.starts_with('/') || p.split('/').any(|s| s.is_empty() || s == ".." || s == "." || s == ".git" || s == ".automemory" || s.contains(':')) {
        bail!("not a plain relative path inside the project: {path}");
    }
    let ext = p.rsplit('.').next().unwrap_or("").to_lowercase();
    if !p.contains('.') || !EXTENSIONS.contains(&ext.as_str()) {
        bail!("only text files are kept: {}", EXTENSIONS.join(", "));
    }
    Ok(p)
}

fn secret_line(content: &str) -> Option<usize> {
    content.lines().position(contains_secrets).map(|i| i + 1)
}

#[derive(Debug, Serialize)]
pub struct LibraryHit {
    pub path: String,
    pub l0: String,
    pub heading: String,
    pub snippet: String,
}

fn fts_query(q: &str) -> String {
    q.split(|c: char| !(c.is_alphanumeric() || c == '_')).filter(|w| !w.is_empty()).map(|w| format!("\"{w}\"*")).collect::<Vec<_>>().join(" ")
}

impl Database {
    /// Adds or updates a file. Refused: a path that is not plain text, content over the limits, a secret (a person may override), an agent
    /// changing a file a person wrote.
    pub fn library_put(&self, new: NewFile) -> Result<(LibraryFile, PutOutcome)> {
        let path = if new.linked { clean_linked_path(&new.path)? } else { clean_path(&new.path)? };
        if !new.linked && path.eq_ignore_ascii_case("INDEX.md") {
            bail!("INDEX.md is written by AutoMemory; choose another name");
        }
        if new.content.contains('\0') {
            bail!("binary content is not accepted: only text files");
        }
        if new.content.len() > FILE_MAX {
            bail!("the file is {} KB; the limit is {} KB", new.content.len() / 1024, FILE_MAX / 1024);
        }
        if let Some(line) = secret_line(&new.content) {
            if new.origin == Origin::Agent || !new.allow_secrets {
                bail!("line {line} looks like a secret (key, token or password): remove it first{}", if new.origin == Origin::Human { " (or add the file with --allow-secrets if you are sure)" } else { "" });
            }
        }
        let existing = self.library_get(new.project_id.as_deref(), &path)?;
        if let Some(e) = &existing {
            if e.linked != new.linked {
                bail!("{path} is already in the library as {}", if e.linked { "a file kept in place" } else { "a copied file" });
            }
            if e.origin == Origin::Human && new.origin == Origin::Agent {
                bail!("{path} was written by a person: an agent may not change it (suggest an edit instead)");
            }
        }
        let used: i64 = self.conn().query_row(
            "SELECT COALESCE(SUM(size), 0) FROM library_files WHERE (project_id IS ?1) AND status = 'active' AND path != ?2", params![new.project_id, path], |r| r.get(0))?;
        if used as usize + new.content.len() > PROJECT_MAX {
            bail!("the project library would pass {} MB", PROJECT_MAX / 1_000_000);
        }
        let sha = format!("{:x}", Sha256::digest(new.content.as_bytes()));
        if existing.as_ref().is_some_and(|e| e.sha256 == sha) {
            return Ok((existing.unwrap(), PutOutcome::Unchanged));
        }
        let now = Utc::now().to_rfc3339();
        let (l0, l1) = (tiers::l0(&path, &new.content), tiers::l1(&new.content));
        let scope = if new.personal { "personal" } else { "project" };
        let id = existing.as_ref().map(|e| e.id.clone()).unwrap_or_else(|| format!("lib-{}", Uuid::now_v7().simple()));
        let created = existing.as_ref().map(|e| e.created_at.to_rfc3339()).unwrap_or_else(|| now.clone());
        let mut conn = self.conn();
        let tx = conn.transaction()?;
        tx.execute(
            &format!("INSERT OR REPLACE INTO library_files ({COLS}, status) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, 'active')"),
            params![id, new.project_id, path, new.origin.as_str(), new.author, scope, new.content.len() as i64, sha, new.content, l0, l1, created, now, new.linked as i64],
        )?;
        let rev: i64 = tx.query_row("SELECT COALESCE(MAX(rev), 0) + 1 FROM library_revisions WHERE file_id = ?1", [&id], |r| r.get(0))?;
        tx.execute("INSERT INTO library_revisions (file_id, rev, sha256, content, author, origin, created_at) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
            params![id, rev, sha, new.content, new.author, new.origin.as_str(), now])?;
        tx.execute("DELETE FROM library_revisions WHERE file_id = ?1 AND rev <= ?2", params![id, rev - REVISIONS_KEPT as i64])?;
        // Search pieces: replace the old ones.
        let old: Vec<i64> = tx.prepare("SELECT id FROM library_chunks WHERE file_id = ?1")?.query_map([&id], |r| r.get(0))?.collect::<std::result::Result<_, _>>()?;
        for cid in old {
            tx.execute("DELETE FROM library_fts WHERE rowid = ?1", [cid])?;
        }
        tx.execute("DELETE FROM library_chunks WHERE file_id = ?1", [&id])?;
        for (i, (heading, text)) in tiers::chunks(&new.content).iter().enumerate() {
            tx.execute("INSERT INTO library_chunks (file_id, seq, heading, text) VALUES (?1, ?2, ?3, ?4)", params![id, i as i64, heading, text])?;
            let cid = tx.last_insert_rowid();
            tx.execute("INSERT INTO library_fts (rowid, text) VALUES (?1, ?2)", params![cid, format!("{heading}\n{text}")])?;
        }
        tx.commit()?;
        drop(conn);
        let file = self.library_get(new.project_id.as_deref(), &path)?.expect("just written");
        Ok((file, if existing.is_some() { PutOutcome::Updated } else { PutOutcome::Created }))
    }

    pub fn library_get(&self, project_id: Option<&str>, path: &str) -> Result<Option<LibraryFile>> {
        let path = path.trim().replace('\\', "/");
        let path = path.trim_start_matches("./");
        let conn = self.conn();
        let f = conn.query_row(
            &format!("SELECT {COLS} FROM library_files WHERE (project_id IS ?1) AND path = ?2 AND status = 'active'"),
            params![project_id, path], row,
        ).optional()?;
        Ok(f.map(|mut f| {
            f.revisions = conn.query_row("SELECT COUNT(*) FROM library_revisions WHERE file_id = ?1", [&f.id], |r| r.get::<_, i64>(0)).unwrap_or(0) as usize;
            f
        }))
    }

    /// Files of a project (oldest path first), optionally under a folder prefix. Personal files are included: this is the local view.
    pub fn library_list(&self, project_id: Option<&str>, prefix: Option<&str>) -> Result<Vec<LibraryFile>> {
        let conn = self.conn();
        let mut stmt = conn.prepare(&format!("SELECT {COLS} FROM library_files WHERE (project_id IS ?1) AND status = 'active' ORDER BY path"))?;
        let rows = stmt.query_map([project_id], row)?.collect::<std::result::Result<Vec<_>, _>>()?;
        let prefix = prefix.map(|p| p.trim().trim_start_matches("./").to_string()).filter(|p| !p.is_empty());
        Ok(rows.into_iter().filter(|f| prefix.as_ref().is_none_or(|p| f.path.starts_with(p.as_str()))).collect())
    }

    /// Removes a file from the library (its revisions go too).
    pub fn library_remove(&self, project_id: Option<&str>, path: &str) -> Result<bool> {
        let Some(f) = self.library_get(project_id, path)? else { return Ok(false) };
        let mut conn = self.conn();
        let tx = conn.transaction()?;
        let old: Vec<i64> = tx.prepare("SELECT id FROM library_chunks WHERE file_id = ?1")?.query_map([&f.id], |r| r.get(0))?.collect::<std::result::Result<_, _>>()?;
        for cid in old {
            tx.execute("DELETE FROM library_fts WHERE rowid = ?1", [cid])?;
        }
        tx.execute("DELETE FROM library_chunks WHERE file_id = ?1", [&f.id])?;
        tx.execute("DELETE FROM library_revisions WHERE file_id = ?1", [&f.id])?;
        tx.execute("DELETE FROM library_files WHERE id = ?1", [&f.id])?;
        tx.commit()?;
        Ok(true)
    }

    pub fn library_search(&self, project_id: Option<&str>, query: &str, limit: usize) -> Result<Vec<LibraryHit>> {
        let q = fts_query(query);
        if q.is_empty() {
            return Ok(vec![]);
        }
        let conn = self.conn();
        let mut stmt = conn.prepare(
            "SELECT f.path, f.l0, c.heading, snippet(library_fts, 0, '[', ']', '...', 14)
             FROM library_fts JOIN library_chunks c ON c.id = library_fts.rowid JOIN library_files f ON f.id = c.file_id
             WHERE library_fts MATCH ?1 AND (f.project_id IS ?2) AND f.status = 'active' ORDER BY rank LIMIT ?3",
        )?;
        let rows = stmt.query_map(params![q, project_id, limit as i64], |r| Ok(LibraryHit { path: r.get(0)?, l0: r.get(1)?, heading: r.get(2)?, snippet: r.get(3)? }))?
            .collect::<std::result::Result<Vec<_>, _>>()?;
        Ok(rows)
    }

    pub fn library_revisions(&self, project_id: Option<&str>, path: &str) -> Result<Vec<(i64, String, String, String)>> {
        let Some(f) = self.library_get(project_id, path)? else { return Ok(vec![]) };
        let conn = self.conn();
        let mut stmt = conn.prepare("SELECT rev, author, origin, created_at FROM library_revisions WHERE file_id = ?1 ORDER BY rev DESC")?;
        let rows = stmt.query_map([&f.id], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?)))?.collect::<std::result::Result<Vec<_>, _>>()?;
        Ok(rows)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn put(db: &Database, path: &str, content: &str, origin: Origin) -> Result<(LibraryFile, PutOutcome)> {
        db.library_put(NewFile { project_id: Some("p".into()), path: path.into(), content: content.into(), origin, author: if origin == Origin::Human { "human".into() } else { "claude".into() }, personal: false, allow_secrets: false, linked: false })
    }

    #[test]
    fn paths_must_be_plain_relative_text_files() {
        for bad in ["/etc/passwd.md", "../x.md", "a/../b.md", ".git/config.txt", "plans/prd", "bin.exe", "c:/x.md", ""] {
            assert!(clean_path(bad).is_err(), "{bad}");
        }
        assert_eq!(clean_path("./plans\\Prd.MD").unwrap(), "plans/Prd.MD");
    }

    #[test]
    fn a_file_is_stored_with_tiers_revisions_and_search() {
        let db = Database::open_in_memory().unwrap();
        let (f, o) = put(&db, "plans/cart.md", "# Cart PRD\n\nCarts expire after 30 days.\n\n## Decisions\n- Decision: SQLite\n", Origin::Human).unwrap();
        assert_eq!(o, PutOutcome::Created);
        assert!(f.l0.starts_with("Cart PRD") && f.l1.contains("Decision: SQLite"));
        assert_eq!(put(&db, "plans/cart.md", &f.content, Origin::Human).unwrap().1, PutOutcome::Unchanged);
        let (g, o) = put(&db, "plans/cart.md", "# Cart PRD\n\nCarts expire after 90 days.\n", Origin::Human).unwrap();
        assert_eq!((o, g.revisions), (PutOutcome::Updated, 2));
        let hits = db.library_search(Some("p"), "expire", 5).unwrap();
        assert_eq!(hits.len(), 1, "{hits:?}");
        assert!(db.library_search(Some("p"), "SQLite", 5).unwrap().is_empty(), "old text left the index");
        assert!(db.library_search(Some("other"), "expire", 5).unwrap().is_empty(), "scoped to the project");
        assert_eq!(db.library_list(Some("p"), Some("plans/")).unwrap().len(), 1);
        assert!(db.library_remove(Some("p"), "plans/cart.md").unwrap());
        assert!(db.library_search(Some("p"), "expire", 5).unwrap().is_empty() && db.library_get(Some("p"), "plans/cart.md").unwrap().is_none());
    }

    #[test]
    fn secrets_are_refused_with_the_line_and_only_a_person_can_override() {
        let db = Database::open_in_memory().unwrap();
        let text = "# Notes\nkey is sk-abcdefghijklmnopqrstuvwxyz1234567890\n";
        let e = put(&db, "n.md", text, Origin::Human).unwrap_err().to_string();
        assert!(e.contains("line 2") && e.contains("--allow-secrets"), "{e}");
        let e = put(&db, "n.md", text, Origin::Agent).unwrap_err().to_string();
        assert!(e.contains("line 2") && !e.contains("--allow-secrets"), "{e}");
        let ok = db.library_put(NewFile { project_id: Some("p".into()), path: "n.md".into(), content: text.into(), origin: Origin::Human, author: "human".into(), personal: true, allow_secrets: true, linked: false });
        assert!(ok.is_ok());
    }

    #[test]
    fn an_agent_cannot_change_a_file_a_person_wrote_but_can_change_its_own() {
        let db = Database::open_in_memory().unwrap();
        put(&db, "mine.md", "# Mine\n\nWritten by a person.\n", Origin::Human).unwrap();
        let e = put(&db, "mine.md", "# Mine\n\nOverwritten.\n", Origin::Agent).unwrap_err().to_string();
        assert!(e.contains("written by a person"), "{e}");
        put(&db, "agent.md", "# A\n\nfirst.\n", Origin::Agent).unwrap();
        assert_eq!(put(&db, "agent.md", "# A\n\nsecond.\n", Origin::Agent).unwrap().1, PutOutcome::Updated);
        assert!(put(&db, "big.md", &"x".repeat(FILE_MAX + 1), Origin::Human).is_err());
        assert!(put(&db, "bin.md", "a\0b", Origin::Human).is_err());
    }

    #[test]
    fn only_the_last_revisions_are_kept_and_the_index_lists_files_within_budget() {
        let db = Database::open_in_memory().unwrap();
        for i in 0..(REVISIONS_KEPT + 5) {
            put(&db, "r.md", &format!("# R\n\nversion {i}.\n"), Origin::Human).unwrap();
        }
        assert_eq!(db.library_revisions(Some("p"), "r.md").unwrap().len(), REVISIONS_KEPT);
        put(&db, "a/agent.md", "# From an agent\n\nNotes.\n", Origin::Agent).unwrap();
        let idx = crate::library::index_block(&db, Some("p"), 600).unwrap();
        assert!(idx.contains("- r.md - R: version") && idx.contains("(agent-written by claude)") && idx.contains("library.get"), "{idx}");
        assert!(crate::library::index_block(&db, Some("none"), 600).is_none());
    }
}
