use crate::db::Database;
use crate::models::{Memory, MemoryScope, MemorySourceKind, MemoryStatus, MemoryType, MemoryVersion};
use crate::retrieval::VectorEngine;
use anyhow::Result;
use chrono::{DateTime, Utc};
use rusqlite::params;
use std::str::FromStr;

impl Database {
    pub fn save_memory(&self, mem: &Memory) -> Result<()> {
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
                tags = ?8,
                entities = ?9,
                metadata = ?10
            WHERE id = ?11
            "#,
            params![
                mem.content,
                mem.summary,
                mem.confidence,
                mem.importance,
                mem.freshness,
                mem.status.to_string(),
                mem.updated_at.to_rfc3339(),
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
                pid
            ));
        }
        if let Some(s) = status {
            sql.push_str(&format!(" AND status = '{}'", s));
        }
        if let Some(mt) = mem_type {
            sql.push_str(&format!(" AND memory_type = '{}'", mt));
        }
        sql.push_str(" ORDER BY confidence DESC, updated_at DESC LIMIT ?1");

        let mut stmt = conn.prepare(&sql)?;
        let rows = stmt.query_map(params![limit as i64], |row| Self::row_to_memory(row))?;
        let mut list = Vec::new();
        for m in rows {
            list.push(m?);
        }
        Ok(list)
    }

    pub fn search_memories_fts(
        &self,
        query: &str,
        project_id: Option<&str>,
        limit: usize,
    ) -> Result<Vec<(Memory, f64)>> {
        let conn = self.conn();
        // Sanitize FTS query for special syntax chars
        let sanitized = query
            .replace('"', "")
            .replace('*', "")
            .replace('\'', "")
            .trim()
            .to_string();
        let words: Vec<String> = sanitized
            .split_whitespace()
            .filter(|w| !w.is_empty())
            .map(|w| format!("{}*", w))
            .collect();
        let fts_query = if words.is_empty() {
            let list = self.list_memories(project_id, Some(MemoryStatus::Active), None, limit)?;
            return Ok(list.into_iter().map(|m| (m, 1.0)).collect());
        } else {
            words.join(" ")
        };

        let mut sql = r#"
            SELECT m.id, m.scope, m.project_id, m.memory_type, m.content, m.summary,
                   m.confidence, m.importance, m.freshness, m.status, m.source_kind,
                   m.created_at, m.updated_at, m.last_recalled_at, m.tags, m.entities, m.metadata,
                   bm25(memories_fts) as rank
            FROM memories_fts fts
            JOIN memories m ON fts.id = m.id
            WHERE memories_fts MATCH ?1 AND m.status = 'active'
        "#.to_string();

        if let Some(pid) = project_id {
            sql.push_str(&format!(
                " AND (m.scope = 'global' OR m.project_id = '{0}' OR m.project_id IN (SELECT slug FROM projects WHERE id = '{0}') OR m.project_id IN (SELECT id FROM projects WHERE slug = '{0}'))",
                pid
            ));
        }

        sql.push_str(" ORDER BY rank ASC LIMIT ?2");

        let mut stmt = conn.prepare(&sql)?;
        let rows = stmt.query_map(params![fts_query, limit as i64], |row| {
            let mem = Self::row_to_memory(row)?;
            let rank: f64 = row.get(17)?;
            Ok((mem, rank))
        })?;

        let mut results = Vec::new();
        for r in rows {
            results.push(r?);
        }
        Ok(results)
    }

    pub fn archive_memory(&self, id: &str) -> Result<bool> {
        let conn = self.conn();
        let rows = conn.execute(
            "UPDATE memories SET status = 'archived', updated_at = ?1 WHERE id = ?2",
            params![Utc::now().to_rfc3339(), id],
        )?;
        Ok(rows > 0)
    }

    pub fn restore_memory(&self, id: &str) -> Result<bool> {
        let conn = self.conn();
        let rows = conn.execute(
            "UPDATE memories SET status = 'active', updated_at = ?1 WHERE id = ?2",
            params![Utc::now().to_rfc3339(), id],
        )?;
        Ok(rows > 0)
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

    pub fn link_memories(&self, source_id: &str, target_id: &str, relation: &str) -> Result<()> {
        let conn = self.conn();
        conn.execute(
            "INSERT OR REPLACE INTO memory_links (source_memory_id, target_memory_id, relation_type, created_at) VALUES (?1, ?2, ?3, ?4)",
            params![source_id, target_id, relation, Utc::now().to_rfc3339()],
        )?;
        Ok(())
    }

    pub fn save_memory_embedding(&self, memory_id: &str, embedding: &[f32], model: &str) -> Result<()> {
        let conn = self.conn();
        let blob = VectorEngine::to_blob(embedding);
        conn.execute(
            r#"
            INSERT INTO memory_embeddings (memory_id, embedding, dimensions, model, updated_at)
            VALUES (?1, ?2, ?3, ?4, ?5)
            ON CONFLICT(memory_id) DO UPDATE SET
                embedding = excluded.embedding,
                dimensions = excluded.dimensions,
                model = excluded.model,
                updated_at = excluded.updated_at
            "#,
            params![
                memory_id,
                blob,
                embedding.len() as i32,
                model,
                Utc::now().to_rfc3339(),
            ],
        )?;
        Ok(())
    }

    pub fn get_memory_embedding(&self, memory_id: &str) -> Result<Option<Vec<f32>>> {
        let conn = self.conn();
        let mut stmt = conn.prepare("SELECT embedding FROM memory_embeddings WHERE memory_id = ?1")?;
        let mut rows = stmt.query(params![memory_id])?;
        if let Some(row) = rows.next()? {
            let blob: Vec<u8> = row.get(0)?;
            Ok(Some(VectorEngine::from_blob(&blob)))
        } else {
            Ok(None)
        }
    }

    pub fn search_memories_vector(
        &self,
        query_embedding: &[f32],
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
            JOIN memory_embeddings e ON m.id = e.memory_id
            WHERE m.status = 'active'
        "#.to_string();

        if let Some(pid) = project_id {
            sql.push_str(&format!(" AND (m.scope = 'global' OR m.project_id = '{}')", pid));
        }

        let mut stmt = conn.prepare(&sql)?;
        let mut rows = stmt.query([])?;
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
        let source_kind = MemorySourceKind::from_str(&source_str).unwrap_or(MemorySourceKind::Agent);

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
