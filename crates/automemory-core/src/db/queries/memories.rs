use super::sql_str;
use crate::db::Database;
use crate::models::{
    Memory, MemoryScope, MemorySourceKind, MemoryStatus, MemoryType, MemoryVersion,
};
use crate::redaction::redact_text;
use crate::retrieval::VectorEngine;
use anyhow::Result;
use chrono::{DateTime, Utc};
use rusqlite::params;
use std::str::FromStr;

/// What gets embedded for a memory: the summary and the start of the content.
pub fn embedding_text(summary: &str, content: &str) -> String {
    let body: String = content.chars().take(1500).collect();
    format!("{summary}\n{body}")
}

pub fn text_hash(text: &str) -> String {
    use sha2::{Digest, Sha256};
    format!("{:x}", Sha256::digest(text.as_bytes()))
}

/// Secrets never reach the database, whichever path wrote the memory (MCP, hook, CLI, sync apply).
fn redacted(mem: &Memory) -> Memory {
    let mut m = mem.clone();
    m.content = redact_text(&m.content);
    m.summary = redact_text(&m.summary);
    m
}

/// A rough stem so "rounded" finds "rounding" and "libraries" finds "library"; the search is a prefix match on the result.
/// Short words are left alone.
fn stem(w: &str) -> String {
    let n = w.chars().count();
    let cut = |suffix: &str| w.strip_suffix(suffix).map(str::to_string);
    if n > 4 {
        if let Some(base) = cut("ies") {
            return format!("{base}y");
        }
        for suffix in ["ing", "ed", "es"] {
            if let Some(base) = cut(suffix).filter(|b| b.chars().count() >= 3) {
                return base;
            }
        }
    }
    if n > 3 {
        if let Some(base) = cut("s").filter(|b| !b.ends_with('s')) {
            return base;
        }
    }
    w.to_string()
}

/// Words that make a question a question and say nothing about the answer. The last five are what `expand_hyde_query`
/// appends to how/what/where/why questions.
const QUESTION_FILLER: &[&str] = &[
    "a", "an", "the", "is", "are", "was", "were", "be", "been", "do", "does", "did", "we", "i", "you", "it", "our", "my",
    "of", "to", "in", "on", "at", "for", "and", "or", "with", "from", "by", "as", "if", "this", "that", "there", "can",
    "should", "would", "could", "how", "what", "which", "where", "when", "why", "who", "me", "us", "get", "got", "any",
    "may", "will",
    "architecture", "setup", "configuration", "pattern", "implementation",
];

impl Database {
    pub fn save_memory(&self, mem: &Memory) -> Result<()> {
        let mem = &redacted(mem);
        // Scope-Aware Deduplication: Check if active memory with same summary exists in this scope
        let existing_id: Option<String> = {
            let conn = self.conn();
            conn.query_row(
                "SELECT id FROM memories WHERE scope = ?1 AND (project_id IS ?2) AND LOWER(TRIM(summary)) = LOWER(TRIM(?3)) AND status = 'active' LIMIT 1",
                params![mem.scope.to_string(), mem.project_id, mem.summary],
                |r| r.get(0),
            ).ok()
        };

        if let Some(eid) = existing_id {
            let mut updated = mem.clone();
            updated.id = eid;
            return self.update_memory(
                &updated,
                Some("In-place update for existing scope memory"),
                &[],
            );
        }

        self.save_memory_direct(mem)
    }

    /// Directly inserts or overwrites by ID without checking for summary duplicates in scope.
    /// Useful for raw imports or testing consolidation proposal generation.
    /// Removes a leaked secret from a stored memory and from every earlier version of it (version history keeps old
    /// content), by pattern and, when given, by the exact string. Returns false when the memory does not exist.
    pub fn scrub_memory(&self, id: &str, literal: Option<&str>) -> Result<bool> {
        let clean = |t: &str| {
            let t = redact_text(t);
            match literal.filter(|l| !l.is_empty()) {
                Some(l) => t.replace(l, "[REDACTED]"),
                None => t,
            }
        };
        let Some(mem) = self.get_memory_by_id(id)? else { return Ok(false) };
        let conn = self.conn();
        conn.execute(
            "UPDATE memories SET content = ?1, summary = ?2, updated_at = ?3 WHERE id = ?4",
            params![clean(&mem.content), clean(&mem.summary), Utc::now().to_rfc3339(), id],
        )?;
        let versions: Vec<(String, String, String)> = {
            let mut stmt = conn.prepare("SELECT id, content, summary FROM memory_versions WHERE memory_id = ?1")?;
            let rows = stmt.query_map(params![id], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)))?;
            rows.collect::<std::result::Result<_, _>>()?
        };
        for (vid, content, summary) in versions {
            conn.execute("UPDATE memory_versions SET content = ?1, summary = ?2 WHERE id = ?3", params![clean(&content), clean(&summary), vid])?;
        }
        // An embedding of the old text is derived data; it is rebuilt from the clean text.
        let _ = conn.execute("DELETE FROM memory_vectors WHERE memory_id = ?1", params![id]);
        Ok(true)
    }

    /// Drops the version history of a memory. A computer that is told "a secret was scrubbed" does not know the secret,
    /// so it cannot clean old versions by text; it forgets them.
    pub fn forget_memory_versions(&self, id: &str) -> Result<()> {
        self.conn().execute("DELETE FROM memory_versions WHERE memory_id = ?1", params![id])?;
        Ok(())
    }

    pub fn save_memory_direct(&self, mem: &Memory) -> Result<()> {
        let mem = &redacted(mem);
        let conn = self.conn();
        conn.execute(
            r#"
            INSERT INTO memories (
                id, scope, project_id, memory_type, content, summary,
                confidence, importance, freshness, status, source_kind,
                created_at, updated_at, last_recalled_at, tags, entities, metadata
            ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15, ?16, ?17)
            ON CONFLICT(id) DO UPDATE SET
                content = excluded.content,
                summary = excluded.summary,
                confidence = excluded.confidence,
                importance = excluded.importance,
                freshness = excluded.freshness,
                status = excluded.status,
                updated_at = excluded.updated_at,
                last_recalled_at = excluded.last_recalled_at,
                tags = excluded.tags,
                entities = excluded.entities,
                metadata = excluded.metadata
            "#,
            params![
                mem.id,
                mem.scope.to_string(),
                mem.project_id,
                mem.memory_type.to_string(),
                mem.content,
                mem.summary,
                mem.confidence,
                mem.importance,
                mem.freshness,
                mem.status.to_string(),
                mem.source_kind.to_string(),
                mem.created_at.to_rfc3339(),
                mem.updated_at.to_rfc3339(),
                mem.last_recalled_at.map(|dt| dt.to_rfc3339()),
                serde_json::to_string(&mem.tags)?,
                serde_json::to_string(&mem.entities)?,
                mem.metadata.to_string(),
            ],
        )?;

        // Also record initial version 1 if not exists
        conn.execute(
            r#"
            INSERT OR IGNORE INTO memory_versions (
                id, memory_id, version_num, content, summary, reason, evidence_event_ids, created_at
            ) VALUES (?1, ?2, 1, ?3, ?4, 'Initial version', '[]', ?5)
            "#,
            params![
                uuid::Uuid::new_v4().to_string(),
                mem.id,
                mem.content,
                mem.summary,
                mem.created_at.to_rfc3339(),
            ],
        )?;

        Ok(())
    }

    pub fn update_memory(
        &self,
        mem: &Memory,
        reason: Option<&str>,
        evidence_ids: &[String],
    ) -> Result<()> {
        let mem = &redacted(mem);
        let conn = self.conn();
        // Get next version number
        let next_ver: i32 = conn.query_row(
            "SELECT COALESCE(MAX(version_num), 0) + 1 FROM memory_versions WHERE memory_id = ?1",
            params![mem.id],
            |row| row.get(0),
        )?;

        conn.execute(
            r#"
            UPDATE memories SET
                content = ?1,
                summary = ?2,
                confidence = ?3,
                importance = ?4,
                freshness = ?5,
                status = ?6,
                updated_at = ?7,
                last_recalled_at = ?8,
                tags = ?9,
                entities = ?10,
                metadata = ?11
            WHERE id = ?12
            "#,
            params![
                mem.content,
                mem.summary,
                mem.confidence,
                mem.importance,
                mem.freshness,
                mem.status.to_string(),
                mem.updated_at.to_rfc3339(),
                mem.last_recalled_at.map(|dt| dt.to_rfc3339()),
                serde_json::to_string(&mem.tags)?,
                serde_json::to_string(&mem.entities)?,
                mem.metadata.to_string(),
                mem.id,
            ],
        )?;

        // Record new version
        conn.execute(
            r#"
            INSERT INTO memory_versions (
                id, memory_id, version_num, content, summary, reason, evidence_event_ids, created_at
            ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)
            "#,
            params![
                uuid::Uuid::new_v4().to_string(),
                mem.id,
                next_ver,
                mem.content,
                mem.summary,
                reason,
                serde_json::to_string(evidence_ids)?,
                Utc::now().to_rfc3339(),
            ],
        )?;

        Ok(())
    }

    pub fn get_memory_by_id(&self, id: &str) -> Result<Option<Memory>> {
        let conn = self.conn();
        let mut stmt = conn.prepare(
            r#"
            SELECT id, scope, project_id, memory_type, content, summary,
                   confidence, importance, freshness, status, source_kind,
                   created_at, updated_at, last_recalled_at, tags, entities, metadata
            FROM memories WHERE id = ?1
            "#,
        )?;
        let mut rows = stmt.query(params![id])?;
        if let Some(row) = rows.next()? {
            Ok(Some(Self::row_to_memory(row)?))
        } else {
            Ok(None)
        }
    }

    pub fn list_memories(
        &self,
        project_id: Option<&str>,
        status: Option<MemoryStatus>,
        mem_type: Option<MemoryType>,
        limit: usize,
    ) -> Result<Vec<Memory>> {
        let conn = self.conn();
        let mut sql = "SELECT id, scope, project_id, memory_type, content, summary, confidence, importance, freshness, status, source_kind, created_at, updated_at, last_recalled_at, tags, entities, metadata FROM memories WHERE 1=1".to_string();

        if let Some(pid) = project_id {
            sql.push_str(&format!(
                " AND (scope = 'global' OR project_id = '{0}' OR project_id IN (SELECT slug FROM projects WHERE id = '{0}') OR project_id IN (SELECT id FROM projects WHERE slug = '{0}'))",
                sql_str(pid)
            ));
        }
        if let Some(s) = status {
            sql.push_str(&format!(" AND status = '{}'", sql_str(s)));
        }
        if let Some(mt) = mem_type {
            sql.push_str(&format!(" AND memory_type = '{}'", sql_str(mt)));
        }
        sql.push_str(" ORDER BY confidence DESC, updated_at DESC LIMIT ?1");

        let mut stmt = conn.prepare(&sql)?;
        let rows = stmt.query_map(params![limit as i64], Self::row_to_memory)?;
        let mut list = Vec::new();
        for m in rows {
            list.push(m?);
        }
        Ok(list)
    }

    /// What is queryable: active decision topics plus distinct tags.
    pub fn list_topics(&self, project_id: Option<&str>, limit: usize) -> Result<Vec<String>> {
        let mut topics: Vec<String> = self
            .list_decisions(
                project_id,
                Some(crate::models::DecisionStatus::Active),
                limit,
            )?
            .into_iter()
            .map(|d| d.topic)
            .collect();
        let conn = self.conn();
        let mut stmt = conn.prepare(
            "SELECT DISTINCT j.value FROM memories m, json_each(m.tags) j
             WHERE m.status = 'active'
               AND (?1 IS NULL OR m.scope = 'global' OR m.project_id = ?1)
             LIMIT ?2",
        )?;
        let tags = stmt.query_map(params![project_id, limit as i64], |row| row.get(0))?;
        for t in tags {
            let tag: String = t?;
            if !topics.contains(&tag) {
                topics.push(tag);
            }
        }
        topics.truncate(limit);
        Ok(topics)
    }

    /// Active and pinned notes that match, best first. (Pinned invariants are returned too: they are the notes a person cares most about.)
    pub fn search_memories_fts(
        &self,
        query: &str,
        project_id: Option<&str>,
        limit: usize,
    ) -> Result<Vec<(Memory, f64)>> {
        self.fts_search(query, project_id, limit, false)
    }

    /// What `memory.recall` returns: the same, plus notes a commit has made `changed` or the clock has made `stale`, ranked below
    /// the live ones. They carry their status, so an agent sees "this may be out of date" instead of the note silently disappearing.
    pub fn search_memories_labelled(
        &self,
        query: &str,
        project_id: Option<&str>,
        limit: usize,
    ) -> Result<Vec<(Memory, f64)>> {
        self.fts_search(query, project_id, limit, true)
    }

    fn fts_search(
        &self,
        query: &str,
        project_id: Option<&str>,
        limit: usize,
        include_labelled: bool,
    ) -> Result<Vec<(Memory, f64)>> {
        let conn = self.conn();
        // Sanitize FTS query for special syntax chars, hyphens, and punctuation
        let sanitized: String = query
            .chars()
            .map(|c| {
                if c.is_alphanumeric() || c == '_' || c.is_whitespace() {
                    c
                } else {
                    ' '
                }
            })
            .collect();
        let terms: Vec<String> = sanitized.split_whitespace().map(|w| stem(&w.to_lowercase())).collect();
        let words: Vec<String> = terms.iter().map(|w| format!("{}*", w)).collect();
        if words.is_empty() {
            let list = self.list_memories(project_id, Some(MemoryStatus::Active), None, limit)?;
            return Ok(list
                .into_iter()
                .filter(|m| {
                    m.project_id.as_deref() != Some("demo")
                        && m.metadata.get("demo").and_then(|v| v.as_bool()) != Some(true)
                })
                .map(|m| (m, 1.0))
                .collect());
        }
        // Precise first: every word must be in the note. A question has words no note contains ("which", "do", "we"), so
        // when that finds nothing, fall back to notes that contain any of the words that carry meaning, best match first.
        let all_words = words.join(" ");
        let meaningful: Vec<String> = terms
            .iter()
            .filter(|t| t.chars().count() > 1 && !QUESTION_FILLER.contains(&t.as_str()))
            .map(|t| format!("{}*", t))
            .collect();
        let mut attempts = vec![all_words];
        if meaningful.len() > 1 || (meaningful.len() == 1 && words.len() > 1) {
            attempts.push(meaningful.join(" OR "));
        }
        // The same words plus their developer-vocabulary synonyms ("database" also looks for "postgres"); a superset of the
        // plain OR query, so it replaces it: a note that matches several synonyms ranks above one that matches a single filler-ish word.
        let mut expanded = meaningful.clone();
        for t in terms.iter().filter(|t| t.chars().count() > 1 && !QUESTION_FILLER.contains(&t.as_str())) {
            for r in crate::retrieval::synonyms::related(t) {
                let q = format!("{r}*");
                if !expanded.contains(&q) {
                    expanded.push(q);
                }
            }
        }
        if expanded.len() > meaningful.len() {
            attempts.pop_if(|_| meaningful.len() > 1 || words.len() > 1);
            attempts.push(expanded.join(" OR "));
        }

        let mut sql = r#"
            SELECT m.id, m.scope, m.project_id, m.memory_type, m.content, m.summary,
                   m.confidence, m.importance, m.freshness, m.status, m.source_kind,
                   m.created_at, m.updated_at, m.last_recalled_at, m.tags, m.entities, m.metadata,
                   bm25(memories_fts) as rank
            FROM memories_fts fts
            JOIN memories m ON fts.id = m.id
            WHERE memories_fts MATCH ?1 AND m.status IN ('active', 'pinned' /*LABELLED*/)
              AND (m.project_id IS NULL OR m.project_id != 'demo')
              AND (m.metadata IS NULL OR (json_extract(m.metadata, '$.demo') IS NOT 1 AND json_extract(m.metadata, '$.demo') IS NOT true))
        "#
        .to_string();
        if include_labelled {
            sql = sql.replace("/*LABELLED*/", ", 'changed', 'stale'");
        }

        if let Some(pid) = project_id {
            sql.push_str(&format!(
                " AND (m.scope = 'global' OR m.project_id = '{0}' OR m.project_id IN (SELECT slug FROM projects WHERE id = '{0}') OR m.project_id IN (SELECT id FROM projects WHERE slug = '{0}'))",
                sql_str(pid)
            ));
        }

        // bm25 is lower-is-better; a changed or stale note is pushed behind the live ones.
        sql.push_str(" ORDER BY rank + CASE WHEN m.status IN ('changed', 'stale') THEN 3.0 ELSE 0.0 END ASC LIMIT ?2");

        let mut stmt = conn.prepare(&sql)?;
        for fts_query in attempts {
            let rows = stmt.query_map(params![fts_query, limit as i64], |row| {
                let mem = Self::row_to_memory(row)?;
                let rank: f64 = row.get(17)?;
                Ok((mem, rank))
            })?;
            let mut results = Vec::new();
            for r in rows {
                results.push(r?);
            }
            if !results.is_empty() {
                return Ok(results);
            }
        }
        Ok(Vec::new())
    }

    pub fn archive_memory(&self, id: &str) -> Result<bool> {
        let conn = self.conn();
        let rows = conn.execute(
            "UPDATE memories SET status = 'archived', updated_at = ?1 WHERE id = ?2",
            params![Utc::now().to_rfc3339(), id],
        )?;
        Ok(rows > 0)
    }

    /// Records that these memories were handed to an agent (feeds the hygiene view). Does not touch `updated_at`.
    pub fn mark_recalled(&self, ids: &[String]) {
        let now = Utc::now().to_rfc3339();
        let conn = self.conn();
        for id in ids {
            let _ = conn.execute("UPDATE memories SET last_recalled_at = ?1 WHERE id = ?2", rusqlite::params![now, id]);
        }
    }

    /// Active memories with what hygiene needs: age, last recall and confidence.
    #[allow(clippy::type_complexity)]
    pub fn hygiene_rows(&self, project_id: Option<&str>) -> Result<Vec<(String, String, String, f64, String, Option<String>, String)>> {
        let conn = self.conn();
        let mut stmt = conn.prepare(
            "SELECT id, summary, memory_type, confidence, created_at, last_recalled_at, content FROM memories
             WHERE status = 'active' AND (?1 IS NULL OR project_id = ?1 OR project_id IS NULL) ORDER BY created_at ASC",
        )?;
        let rows = stmt.query_map([project_id], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?, r.get(4)?, r.get(5)?, r.get(6)?)))?;
        Ok(rows.flatten().collect())
    }

    pub fn restore_memory(&self, id: &str) -> Result<bool> {
        let conn = self.conn();
        let rows = conn.execute(
            "UPDATE memories SET status = 'active', updated_at = ?1 WHERE id = ?2",
            params![Utc::now().to_rfc3339(), id],
        )?;
        Ok(rows > 0)
    }

    /// Flips one memory to `changed` without touching content/version history:
    /// linkage changed, content didn't. Pinned invariants are never flipped.
    pub fn mark_memory_changed(&self, id: &str) -> Result<bool> {
        let rows = self.conn().execute(
            "UPDATE memories SET status = 'changed' WHERE id = ?1 AND status = 'active'",
            params![id],
        )?;
        Ok(rows > 0)
    }

    /// PostCommit staleness: memories linked (via entities or recorded files)
    /// to committed files flip to `changed` so the next recall re-verifies.
    /// Returns the flipped ids. A memory is linked by the files recorded with it.
    pub fn flag_memories_changed_by_files(
        &self,
        project_id: Option<&str>,
        files: &[String],
    ) -> Result<Vec<String>> {
        self.flag_memories_changed_by_files_with(project_id, files, &|_| None)
    }

    /// Like [`flag_memories_changed_by_files`], refined by symbol hashes: a memory that recorded the hash of the symbols it is about
    /// (`metadata.symbol_hashes`, stored when a CodeIntel engine was present) stays active when every one of those symbols still
    /// has the same hash, because an edit elsewhere in the file did not touch what the note says. `current_hash` answers `None`
    /// when it cannot tell, and then the memory is flagged, as before.
    pub fn flag_memories_changed_by_files_with(
        &self,
        project_id: Option<&str>,
        files: &[String],
        current_hash: &dyn Fn(&str) -> Option<String>,
    ) -> Result<Vec<String>> {
        if files.is_empty() {
            return Ok(Vec::new());
        }
        let mut flipped = Vec::new();
        for m in self
            .list_memories(project_id, Some(MemoryStatus::Active), None, 500)
            .unwrap_or_default()
        {
            let linked = metadata_files(&m).iter().any(|f| files.contains(f));
            let hashes = metadata_symbol_hashes(&m);
            let symbols_unchanged = !hashes.is_empty() && hashes.iter().all(|(sym, stored)| current_hash(sym).as_deref() == Some(stored.as_str()));
            // A symbol that now hashes differently changes the note even when its file was not recorded with it (decisions have none).
            let symbol_moved = hashes.iter().any(|(sym, stored)| current_hash(sym).is_some_and(|h| h != *stored));
            if (symbol_moved || (linked && !symbols_unchanged)) && self.mark_memory_changed(&m.id).unwrap_or(false) {
                flipped.push(m.id);
            }
        }
        Ok(flipped)
    }

    /// Pin a memory as an immutable Tier 1 invariant (human control)
    pub fn pin_memory(&self, id: &str) -> Result<bool> {
        if let Some(mut mem) = self.get_memory_by_id(id)? {
            mem.status = MemoryStatus::Pinned;
            mem.confidence = 1.0;
            mem.set_trust_tier(crate::models::TrustTier::Pinned);
            mem.updated_at = Utc::now();
            self.update_memory(&mem, Some("Pinned as Tier 1 invariant by human"), &[])?;
            Ok(true)
        } else {
            Ok(false)
        }
    }

    /// Veto/reject a candidate memory or mark invalid (human control)
    pub fn veto_memory(&self, id: &str, reason: Option<&str>) -> Result<bool> {
        if let Some(mut mem) = self.get_memory_by_id(id)? {
            mem.status = MemoryStatus::Invalid;
            mem.confidence = 0.0;
            mem.updated_at = Utc::now();
            let veto_reason = reason.unwrap_or("Vetoed/rejected by human");
            self.update_memory(&mem, Some(veto_reason), &[])?;
            Ok(true)
        } else {
            Ok(false)
        }
    }

    /// Revoke memories created by a session, agent, or specific memory ID
    pub fn revoke_memories(
        &self,
        id: Option<&str>,
        session_id: Option<&str>,
        agent: Option<&str>,
    ) -> Result<usize> {
        let conn = self.conn();
        let now = Utc::now().to_rfc3339();
        let mut count = 0;

        if let Some(mid) = id {
            count += conn.execute(
                "UPDATE memories SET status = 'archived', updated_at = ?1 WHERE id = ?2 AND status != 'archived'",
                params![now, mid],
            )?;
        }
        if let Some(sess) = session_id {
            count += conn.execute(
                "UPDATE memories SET status = 'archived', updated_at = ?1 WHERE (json_extract(metadata, '$.session_id') = ?2 OR tags LIKE ?3) AND status != 'archived'",
                params![now, sess, format!("%\"{}\"%", sess)],
            )?;
        }
        if let Some(ag) = agent {
            count += conn.execute(
                "UPDATE memories SET status = 'archived', updated_at = ?1 WHERE (json_extract(metadata, '$.agent') = ?2 OR tags LIKE ?3) AND status != 'archived'",
                params![now, ag, format!("%\"{}\"%", ag)],
            )?;
        }
        Ok(count)
    }

    pub fn get_memory_versions(&self, memory_id: &str) -> Result<Vec<MemoryVersion>> {
        let conn = self.conn();
        let mut stmt = conn.prepare(
            "SELECT id, memory_id, version_num, content, summary, reason, evidence_event_ids, created_at FROM memory_versions WHERE memory_id = ?1 ORDER BY version_num ASC",
        )?;
        let rows = stmt.query_map(params![memory_id], |row| {
            let created_at_str: String = row.get(7)?;
            let created_at = DateTime::parse_from_rfc3339(&created_at_str)
                .map(|dt| dt.with_timezone(&Utc))
                .unwrap_or_else(|_| Utc::now());
            let ev_str: String = row.get(6)?;
            let evidence_event_ids = serde_json::from_str(&ev_str).unwrap_or_default();

            Ok(MemoryVersion {
                id: row.get(0)?,
                memory_id: row.get(1)?,
                version_num: row.get(2)?,
                content: row.get(3)?,
                summary: row.get(4)?,
                reason: row.get(5)?,
                evidence_event_ids,
                created_at,
            })
        })?;

        let mut list = Vec::new();
        for v in rows {
            list.push(v?);
        }
        Ok(list)
    }

    /// Nearest active memories by cosine similarity among vectors of `model`.
    pub fn search_memories_vector(
        &self,
        query_embedding: &[f32],
        model: &str,
        project_id: Option<&str>,
        limit: usize,
    ) -> Result<Vec<(Memory, f32)>> {
        let conn = self.conn();
        let mut sql = r#"
            SELECT m.id, m.scope, m.project_id, m.memory_type, m.content, m.summary,
                   m.confidence, m.importance, m.freshness, m.status, m.source_kind,
                   m.created_at, m.updated_at, m.last_recalled_at, m.tags, m.entities, m.metadata,
                   e.embedding
            FROM memories m
            JOIN memory_vectors e ON m.id = e.memory_id
            WHERE m.status = 'active' AND e.model = ?1
              AND (m.project_id IS NULL OR m.project_id != 'demo')
              AND (m.metadata IS NULL OR (json_extract(m.metadata, '$.demo') IS NOT 1 AND json_extract(m.metadata, '$.demo') IS NOT true))
        "#
        .to_string();

        if let Some(pid) = project_id {
            sql.push_str(&format!(
                " AND (m.scope = 'global' OR m.project_id = '{0}' OR m.project_id IN (SELECT slug FROM projects WHERE id = '{0}') OR m.project_id IN (SELECT id FROM projects WHERE slug = '{0}'))",
                sql_str(pid)
            ));
        }

        let mut stmt = conn.prepare(&sql)?;
        let mut rows = stmt.query(params![model])?;
        let mut candidates = Vec::new();

        while let Some(row) = rows.next()? {
            let mem = Self::row_to_memory(row)?;
            let blob: Vec<u8> = row.get(17)?;
            let emb = VectorEngine::from_blob(&blob);
            let sim = VectorEngine::cosine_similarity(query_embedding, &emb);
            candidates.push((mem, sim));
        }

        candidates.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));
        candidates.truncate(limit);
        Ok(candidates)
    }

    /// Stores (or replaces) the vector of one memory for `model`.
    pub fn upsert_memory_vector(&self, memory_id: &str, model: &str, vector: &[f32], content_hash: &str) -> Result<()> {
        self.conn().execute(
            "INSERT INTO memory_vectors (memory_id, model, dimensions, embedding, content_hash, updated_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6)
             ON CONFLICT(memory_id, model) DO UPDATE SET
                dimensions = excluded.dimensions, embedding = excluded.embedding,
                content_hash = excluded.content_hash, updated_at = excluded.updated_at",
            params![
                memory_id,
                model,
                vector.len() as i64,
                VectorEngine::to_blob(vector),
                content_hash,
                chrono::Utc::now().to_rfc3339()
            ],
        )?;
        Ok(())
    }

    /// Active memories with no vector for `model`, or whose text changed since it was embedded:
    /// `(id, text to embed, content hash)`.
    pub fn memories_needing_vectors(&self, model: &str, limit: usize) -> Result<Vec<(String, String, String)>> {
        let conn = self.conn();
        let mut stmt = conn.prepare(
            "SELECT m.id, m.summary, m.content, v.content_hash
             FROM memories m
             LEFT JOIN memory_vectors v ON v.memory_id = m.id AND v.model = ?1
             WHERE m.status = 'active'
             ORDER BY m.updated_at DESC",
        )?;
        let rows = stmt.query_map(params![model], |r| {
            Ok((r.get::<_, String>(0)?, r.get::<_, String>(1)?, r.get::<_, String>(2)?, r.get::<_, Option<String>>(3)?))
        })?;
        let mut out = Vec::new();
        for row in rows.flatten() {
            let (id, summary, content, stored) = row;
            let text = embedding_text(&summary, &content);
            let hash = text_hash(&text);
            if stored.as_deref() != Some(hash.as_str()) {
                out.push((id, text, hash));
                if out.len() >= limit {
                    break;
                }
            }
        }
        Ok(out)
    }

    /// `(embedded, active)` memory counts for `model`, and the other models present.
    #[allow(clippy::type_complexity)]
    pub fn vector_coverage(&self, model: &str) -> Result<(usize, usize, Vec<(String, usize)>)> {
        let conn = self.conn();
        let active: i64 = conn.query_row("SELECT COUNT(*) FROM memories WHERE status = 'active'", [], |r| r.get(0))?;
        let embedded: i64 = conn.query_row(
            "SELECT COUNT(*) FROM memory_vectors v JOIN memories m ON m.id = v.memory_id WHERE m.status = 'active' AND v.model = ?1",
            params![model],
            |r| r.get(0),
        )?;
        let mut stmt = conn.prepare("SELECT model, COUNT(*) FROM memory_vectors WHERE model != ?1 GROUP BY model")?;
        let others = stmt
            .query_map(params![model], |r| Ok((r.get::<_, String>(0)?, r.get::<_, i64>(1)? as usize)))?
            .flatten()
            .collect();
        Ok((embedded as usize, active as usize, others))
    }

    pub fn row_to_memory(row: &rusqlite::Row) -> rusqlite::Result<Memory> {
        let id: String = row.get(0)?;
        let scope_str: String = row.get(1)?;
        let project_id: Option<String> = row.get(2)?;
        let type_str: String = row.get(3)?;
        let content: String = row.get(4)?;
        let summary: String = row.get(5)?;
        let confidence: f64 = row.get(6)?;
        let importance: f64 = row.get(7)?;
        let freshness: f64 = row.get(8)?;
        let status_str: String = row.get(9)?;
        let source_str: String = row.get(10)?;
        let created_at_str: String = row.get(11)?;
        let updated_at_str: String = row.get(12)?;
        let last_recalled_at_str: Option<String> = row.get(13)?;
        let tags_str: String = row.get(14)?;
        let entities_str: String = row.get(15)?;
        let meta_str: String = row.get(16)?;

        let scope = MemoryScope::from_str(&scope_str).unwrap_or(MemoryScope::Project);
        let memory_type = MemoryType::from_str(&type_str).unwrap_or(MemoryType::Fact);
        let status = MemoryStatus::from_str(&status_str).unwrap_or(MemoryStatus::Active);
        let source_kind =
            MemorySourceKind::from_str(&source_str).unwrap_or(MemorySourceKind::Agent);

        let created_at = DateTime::parse_from_rfc3339(&created_at_str)
            .map(|dt| dt.with_timezone(&Utc))
            .unwrap_or_else(|_| Utc::now());
        let updated_at = DateTime::parse_from_rfc3339(&updated_at_str)
            .map(|dt| dt.with_timezone(&Utc))
            .unwrap_or_else(|_| Utc::now());
        let last_recalled_at = last_recalled_at_str.and_then(|s| {
            DateTime::parse_from_rfc3339(&s)
                .map(|dt| dt.with_timezone(&Utc))
                .ok()
        });

        let tags: Vec<String> = serde_json::from_str(&tags_str).unwrap_or_default();
        let entities: Vec<String> = serde_json::from_str(&entities_str).unwrap_or_default();
        let metadata: serde_json::Value = serde_json::from_str(&meta_str).unwrap_or_default();

        Ok(Memory {
            id,
            scope,
            project_id,
            memory_type,
            content,
            summary,
            confidence,
            importance,
            freshness,
            status,
            source_kind,
            version: Some(1),
            created_at,
            updated_at,
            last_recalled_at,
            tags,
            entities,
            metadata,
        })
    }
}

/// File paths a memory recorded itself against (`memory.save` stores them
/// under `metadata.files_modified`).
/// `{symbol: hash}` recorded with the memory (see `flag_memories_changed_by_files_with`).
fn metadata_symbol_hashes(m: &Memory) -> Vec<(String, String)> {
    m.metadata
        .get("symbol_hashes")
        .and_then(|v| v.as_object())
        .map(|o| o.iter().filter_map(|(k, v)| v.as_str().map(|h| (k.clone(), h.to_string()))).collect())
        .unwrap_or_default()
}

fn metadata_files(m: &Memory) -> Vec<String> {
    m.metadata
        .get("files_modified")
        .or_else(|| m.metadata.get("files"))
        .and_then(|v| v.as_array())
        .map(|arr| {
            arr.iter()
                .filter_map(|i| i.as_str().map(str::to_string))
                .collect()
        })
        .unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::*;
    use uuid::Uuid;

    fn note(id: &str, text: &str) -> Memory {
        Memory {
            id: id.into(),
            scope: MemoryScope::Global,
            project_id: None,
            memory_type: MemoryType::Fact,
            content: text.into(),
            summary: text.into(),
            confidence: 1.0,
            importance: 0.5,
            freshness: 1.0,
            status: MemoryStatus::Active,
            source_kind: MemorySourceKind::User,
            version: Some(1),
            created_at: Utc::now(),
            updated_at: Utc::now(),
            last_recalled_at: None,
            tags: vec![],
            entities: vec![],
            metadata: serde_json::json!({}),
        }
    }

    #[test]
    fn a_question_finds_the_note_that_answers_it() {
        let db = Database::open_in_memory().unwrap();
        for (id, t) in [
            ("pg", "We use Postgres 15 because the reporting team needs window functions"),
            ("round", "Never round before tax; round the cart total at the end"),
            ("deploy", "Deploys go through make release; never push tags by hand"),
            ("pnpm", "The frontend uses pnpm, not npm"),
            ("reset", "Reset the test database with scripts/reset_db.sh before running integration tests"),
        ] {
            db.save_memory_direct(&note(id, t)).unwrap();
        }
        let top = |q: &str| db.search_memories_fts(q, None, 5).unwrap().into_iter().map(|(m, _)| m.id).collect::<Vec<_>>();

        // Sentences used to return nothing: every word had to be in the note.
        assert_eq!(top("how do I deploy a release")[0], "deploy");
        assert_eq!(top("when do I round the total")[0], "round");
        assert_eq!(top("package manager for the frontend")[0], "pnpm");
        assert_eq!(top("how to reset test data")[0], "reset");
        assert!(top("which database do we use").contains(&"reset".to_string()), "matches the word it has");
        assert!(top("which database do we use").contains(&"pg".to_string()), "and the note that says what we use");
        // Word forms: "rounded" finds "rounding", "libraries" finds "library".
        let db2 = Database::open_in_memory().unwrap();
        db2.save_memory_direct(&note("r", "Money rounding: round once, at the end, after tax")).unwrap();
        db2.save_memory_direct(&note("l", "Constraint: standard library only. Do not add dependencies.")).unwrap();
        let top2 = |q: &str| db2.search_memories_fts(q, None, 5).unwrap().into_iter().map(|(m, _)| m.id).collect::<Vec<_>>();
        assert_eq!(top2("how should the total be rounded")[0], "r");
        assert_eq!(top2("which libraries may I use")[0], "l");

        // A question with the padding the context builder adds to how/what/where/why questions.
        assert_eq!(top("how do I deploy a release architecture setup configuration pattern implementation")[0], "deploy");

        // Precise queries stay precise: all words match, so only that note comes back.
        assert_eq!(top("pnpm"), vec!["pnpm"]);
        assert_eq!(top("round tax"), vec!["round"]);
        // Words no note has still find nothing.
        assert!(top("kubernetes helm chart").is_empty());
    }

    #[test]
    fn secrets_are_redacted_on_every_write_path() {
        let db = Database::open_in_memory().expect("open db");
        let key = "AKIAIOSFODNN7EXAMPLE";
        let mem = Memory {
            id: Uuid::now_v7().to_string(),
            scope: MemoryScope::Project,
            project_id: Some("p".to_string()),
            memory_type: MemoryType::Decision,
            content: format!("deploy with key {key}"),
            summary: format!("uses {key}"),
            confidence: 0.9,
            importance: 0.8,
            freshness: 1.0,
            status: MemoryStatus::Active,
            source_kind: MemorySourceKind::Agent,
            version: Some(1),
            created_at: Utc::now(),
            updated_at: Utc::now(),
            last_recalled_at: None,
            tags: vec![],
            entities: vec![],
            metadata: serde_json::json!({}),
        };
        db.save_memory(&mem).expect("save");
        let mut direct = mem.clone();
        direct.id = Uuid::now_v7().to_string();
        direct.summary = format!("direct {key}");
        db.save_memory_direct(&direct).expect("direct");
        for m in db.list_memories(Some("p"), None, None, 10).expect("list") {
            assert!(!m.content.contains(key) && !m.summary.contains(key), "secret stored: {m:?}");
        }
        let d = db
            .record_decision(crate::models::DecisionRecordArgs {
                project_id: Some("p"),
                topic: "deploy",
                decision: &format!("export AWS key {key}"),
                reason: &format!("token = {key}abcdef"),
                alternatives: &[],
                dissent: None,
                agent: "a",
                symbols: &[],
                evidence: &[],
            })
            .expect("decision");
        assert!(!d.decision.contains(key) && !d.reason.contains(key));
    }

    #[test]
    fn test_save_memory_deduplication_in_same_scope() {
        let db = Database::open_in_memory().expect("open db");
        let mem1 = Memory {
            id: Uuid::now_v7().to_string(),
            scope: MemoryScope::Project,
            project_id: Some("calc-proj".to_string()),
            memory_type: MemoryType::Decision,
            content: "Initial decision: use Raylib 5.0".to_string(),
            summary: "Use Raylib 5.0 for UI".to_string(),
            confidence: 0.9,
            importance: 0.8,
            freshness: 1.0,
            status: MemoryStatus::Active,
            source_kind: MemorySourceKind::Agent,
            version: Some(1),
            created_at: Utc::now(),
            updated_at: Utc::now(),
            last_recalled_at: None,
            tags: vec!["raylib".to_string()],
            entities: vec![],
            metadata: serde_json::json!({}),
        };

        db.save_memory(&mem1).expect("save mem1");
        let count1 = db
            .list_memories(Some("calc-proj"), Some(MemoryStatus::Active), None, 10)
            .expect("list");
        assert_eq!(count1.len(), 1);

        // Save second memory with DIFFERENT UUID but SAME summary & project_id
        let mem2 = Memory {
            id: Uuid::now_v7().to_string(),
            scope: MemoryScope::Project,
            project_id: Some("calc-proj".to_string()),
            memory_type: MemoryType::Decision,
            content: "Updated decision: use Raylib 5.0 with Cocoa framework on macOS".to_string(),
            summary: "Use Raylib 5.0 for UI".to_string(),
            confidence: 0.95,
            importance: 0.85,
            freshness: 1.0,
            status: MemoryStatus::Active,
            source_kind: MemorySourceKind::Agent,
            version: Some(1),
            created_at: Utc::now(),
            updated_at: Utc::now(),
            last_recalled_at: None,
            tags: vec!["raylib".to_string(), "macos".to_string()],
            entities: vec![],
            metadata: serde_json::json!({}),
        };

        db.save_memory(&mem2).expect("save mem2");

        // There must still be EXACTLY 1 active memory in this scope, NOT 2!
        let count2 = db
            .list_memories(Some("calc-proj"), Some(MemoryStatus::Active), None, 10)
            .expect("list");
        assert_eq!(count2.len(), 1);
        assert_eq!(count2[0].id, mem1.id);
        assert!(count2[0].content.contains("Cocoa framework on macOS"));

        // Verify version history tracked the previous version
        let versions = db.get_memory_versions(&mem1.id).expect("versions");
        assert!(versions.len() >= 2);
    }

    #[test]
    fn test_pin_veto_revoke_memories() {
        let db = Database::open_in_memory().expect("in-memory db open");

        let mem = Memory {
            id: "mem-pin-test".to_string(),
            scope: MemoryScope::Project,
            project_id: Some("proj-test".to_string()),
            memory_type: MemoryType::Fact,
            content: "Never allow plain-text secrets in git".to_string(),
            summary: "Secret security rule".to_string(),
            confidence: 0.9,
            importance: 0.9,
            freshness: 1.0,
            status: MemoryStatus::Active,
            source_kind: MemorySourceKind::Agent,
            version: Some(1),
            created_at: Utc::now(),
            updated_at: Utc::now(),
            last_recalled_at: None,
            tags: vec!["security".to_string(), "secrets".to_string()],
            entities: vec![],
            metadata: serde_json::json!({
                "agent": "claude-code",
                "session_id": "sess-42"
            }),
        };

        db.save_memory(&mem).expect("save memory");

        // 1. Test pin
        let pinned = db.pin_memory(&mem.id).expect("pin memory");
        assert!(pinned);
        let fetched = db.get_memory_by_id(&mem.id).expect("get").expect("exists");
        assert_eq!(fetched.status, MemoryStatus::Pinned);
        assert_eq!(fetched.trust_tier(), crate::models::TrustTier::Pinned);
        assert_eq!(fetched.confidence, 1.0);

        // 2. Test veto
        let vetoed = db
            .veto_memory(&mem.id, Some("Violates updated policy"))
            .expect("veto memory");
        assert!(vetoed);
        let fetched_veto = db.get_memory_by_id(&mem.id).expect("get").expect("exists");
        assert_eq!(fetched_veto.status, MemoryStatus::Invalid);
        assert_eq!(fetched_veto.confidence, 0.0);

        // 3. Test revoke by session
        let revoked = db
            .revoke_memories(None, Some("sess-42"), None)
            .expect("revoke");
        assert_eq!(revoked, 1);
        let fetched_revoked = db.get_memory_by_id(&mem.id).expect("get").expect("exists");
        assert_eq!(fetched_revoked.status, MemoryStatus::Archived);
    }

    #[test]
    fn test_flag_changed_by_commit_files() {
        let db = Database::open_in_memory().expect("open db");
        let now = Utc::now();

        let mk = |summary: &str,
                  entities: Vec<String>,
                  metadata: serde_json::Value,
                  status: MemoryStatus| {
            let mem = Memory {
                id: Uuid::now_v7().to_string(),
                scope: MemoryScope::Project,
                project_id: None,
                memory_type: MemoryType::Fact,
                content: "c".to_string(),
                summary: summary.to_string(),
                confidence: 0.9,
                importance: 0.8,
                freshness: 1.0,
                status,
                source_kind: MemorySourceKind::Agent,
                version: Some(1),
                created_at: now,
                updated_at: now,
                last_recalled_at: None,
                tags: vec![],
                entities,
                metadata,
            };
            db.save_memory_direct(&mem).expect("seed memory");
            mem.id
        };

        // A memory linked only by a symbol name no longer flips on a commit: without a code index there is no way to map files to symbols.
        let by_entity = mk(
            "entity linked",
            vec!["auth.session.createSession".to_string()],
            serde_json::json!({}),
            MemoryStatus::Active,
        );
        let by_file = mk(
            "file linked",
            vec![],
            serde_json::json!({"files_modified": ["src/auth/session.ts"]}),
            MemoryStatus::Active,
        );
        let unrelated = mk(
            "unrelated",
            vec!["other.fn".to_string()],
            serde_json::json!({}),
            MemoryStatus::Active,
        );
        let pinned = mk(
            "pinned linked",
            vec!["auth.session.createSession".to_string()],
            serde_json::json!({}),
            MemoryStatus::Pinned,
        );

        assert!(db
            .flag_memories_changed_by_files(None, &[])
            .unwrap()
            .is_empty());
        let flipped = db
            .flag_memories_changed_by_files(None, &["src/auth/session.ts".to_string()])
            .unwrap();
        assert_eq!(flipped, vec![by_file.clone()]);

        let status = |id: &str| db.get_memory_by_id(id).unwrap().unwrap().status;
        assert_eq!(status(&by_entity), MemoryStatus::Active);
        assert_eq!(status(&by_file), MemoryStatus::Changed);
        assert_eq!(status(&unrelated), MemoryStatus::Active);
        assert_eq!(status(&pinned), MemoryStatus::Pinned);
    }

    #[test]
    fn a_memory_with_symbol_hashes_is_flagged_only_when_its_symbol_changed() {
        let db = Database::open_in_memory().unwrap();
        let mk = |id: &str, extra: serde_json::Value| {
            let mut meta = serde_json::json!({ "files_modified": ["src/a.rs"] });
            meta.as_object_mut().unwrap().extend(extra.as_object().unwrap().clone());
            let mut m = note(id, "about a::parse");
            m.metadata = meta;
            db.save_memory_direct(&m).unwrap();
        };
        for id in ["hashed-1", "hashed-2", "hashed-3"] {
            mk(id, serde_json::json!({ "symbol_hashes": { "a::parse": "h1" } }));
        }
        mk("plain", serde_json::json!({}));
        let files = vec!["src/a.rs".to_string()];

        // The engine says a::parse still hashes to h1: only the memory with no hashes is flagged (file-level, as before).
        let flipped = db.flag_memories_changed_by_files_with(None, &files, &|_| Some("h1".into())).unwrap();
        assert_eq!(flipped, vec!["plain".to_string()]);
        // The symbol changed: now the hashed ones go too. An engine that cannot tell (None) flags them as well.
        let flipped = db.flag_memories_changed_by_files_with(None, &files, &|_| Some("h2".into())).unwrap();
        assert_eq!(flipped.len(), 3, "{flipped:?}");
        let db2 = Database::open_in_memory().unwrap();
        let mut m = note("x", "about a::parse");
        m.metadata = serde_json::json!({ "files_modified": ["src/a.rs"], "symbol_hashes": { "a::parse": "h1" } });
        db2.save_memory_direct(&m).unwrap();
        assert_eq!(db2.flag_memories_changed_by_files_with(None, &files, &|_| None).unwrap(), vec!["x".to_string()]);

        // A decision's memory has symbol hashes but no file: it is flagged when a symbol moved, left alone when it did not or cannot be told.
        let db3 = Database::open_in_memory().unwrap();
        let d = db3
            .record_decision(crate::models::DecisionRecordArgs {
                project_id: None, topic: "parsing", decision: "parse is pure", reason: "easy to test", alternatives: &[], dissent: None,
                agent: "human", symbols: &["a::parse".to_string()], evidence: &[],
            })
            .unwrap();
        let hashes: serde_json::Map<String, serde_json::Value> = [("a::parse".to_string(), serde_json::json!("h1"))].into_iter().collect();
        assert!(db3.set_decision_symbol_hashes(&d.id, &hashes).unwrap());
        let other = vec!["src/unrelated.rs".to_string()];
        assert!(db3.flag_memories_changed_by_files_with(None, &other, &|_| Some("h1".into())).unwrap().is_empty(), "same hash");
        assert!(db3.flag_memories_changed_by_files_with(None, &other, &|_| None).unwrap().is_empty(), "cannot tell: left alone");
        assert_eq!(db3.flag_memories_changed_by_files_with(None, &other, &|_| Some("h2".into())).unwrap().len(), 1, "the symbol changed");
    }
}
