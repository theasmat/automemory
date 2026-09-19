use super::connection::Database;
use crate::models::*;
use anyhow::Result;
use chrono::{DateTime, Utc};
use rusqlite::params;
use std::str::FromStr;

impl Database {
    // ==========================================
    // Project queries
    // ==========================================

    pub fn save_project(&self, project: &Project) -> Result<()> {
        let conn = self.conn();
        conn.execute(
            r#"
            INSERT INTO projects (
                id, slug, display_name, created_at, updated_at,
                capture_policy, context_budget, document_config, sync_config
            ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)
            ON CONFLICT(id) DO UPDATE SET
                slug = excluded.slug,
                display_name = excluded.display_name,
                updated_at = excluded.updated_at,
                capture_policy = excluded.capture_policy,
                context_budget = excluded.context_budget,
                document_config = excluded.document_config,
                sync_config = excluded.sync_config
            "#,
            params![
                project.id,
                project.slug,
                project.display_name,
                project.created_at.to_rfc3339(),
                project.updated_at.to_rfc3339(),
                serde_json::to_string(&project.capture_policy)?,
                serde_json::to_string(&project.context_budget)?,
                serde_json::to_string(&project.document_config)?,
                serde_json::to_string(&project.sync_config)?,
            ],
        )?;
        Ok(())
    }

    pub fn get_project_by_id(&self, id: &str) -> Result<Option<Project>> {
        let conn = self.conn();
        let mut stmt = conn.prepare(
            "SELECT id, slug, display_name, created_at, updated_at, capture_policy, context_budget, document_config, sync_config FROM projects WHERE id = ?1",
        )?;
        let mut rows = stmt.query(params![id])?;
        if let Some(row) = rows.next()? {
            let created_at_str: String = row.get(3)?;
            let updated_at_str: String = row.get(4)?;
            let cp_str: String = row.get(5)?;
            let cb_str: String = row.get(6)?;
            let dc_str: String = row.get(7)?;
            let sc_str: String = row.get(8)?;

            Ok(Some(Project {
                id: row.get(0)?,
                slug: row.get(1)?,
                display_name: row.get(2)?,
                created_at: DateTime::parse_from_rfc3339(&created_at_str)?.with_timezone(&Utc),
                updated_at: DateTime::parse_from_rfc3339(&updated_at_str)?.with_timezone(&Utc),
                capture_policy: serde_json::from_str(&cp_str)?,
                context_budget: serde_json::from_str(&cb_str)?,
                document_config: serde_json::from_str(&dc_str)?,
                sync_config: serde_json::from_str(&sc_str)?,
            }))
        } else {
            Ok(None)
        }
    }

    pub fn get_project_by_slug(&self, slug: &str) -> Result<Option<Project>> {
        let conn = self.conn();
        let mut stmt = conn.prepare(
            "SELECT id, slug, display_name, created_at, updated_at, capture_policy, context_budget, document_config, sync_config FROM projects WHERE slug = ?1",
        )?;
        let mut rows = stmt.query(params![slug])?;
        if let Some(row) = rows.next()? {
            let created_at_str: String = row.get(3)?;
            let updated_at_str: String = row.get(4)?;
            let cp_str: String = row.get(5)?;
            let cb_str: String = row.get(6)?;
            let dc_str: String = row.get(7)?;
            let sc_str: String = row.get(8)?;

            Ok(Some(Project {
                id: row.get(0)?,
                slug: row.get(1)?,
                display_name: row.get(2)?,
                created_at: DateTime::parse_from_rfc3339(&created_at_str)?.with_timezone(&Utc),
                updated_at: DateTime::parse_from_rfc3339(&updated_at_str)?.with_timezone(&Utc),
                capture_policy: serde_json::from_str(&cp_str)?,
                context_budget: serde_json::from_str(&cb_str)?,
                document_config: serde_json::from_str(&dc_str)?,
                sync_config: serde_json::from_str(&sc_str)?,
            }))
        } else {
            Ok(None)
        }
    }

    pub fn list_projects(&self) -> Result<Vec<Project>> {
        let conn = self.conn();
        let mut stmt = conn.prepare(
            "SELECT id, slug, display_name, created_at, updated_at, capture_policy, context_budget, document_config, sync_config FROM projects ORDER BY display_name ASC",
        )?;
        let rows = stmt.query_map([], |row| {
            let created_at_str: String = row.get(3)?;
            let updated_at_str: String = row.get(4)?;
            let cp_str: String = row.get(5)?;
            let cb_str: String = row.get(6)?;
            let dc_str: String = row.get(7)?;
            let sc_str: String = row.get(8)?;

            let created_at = DateTime::parse_from_rfc3339(&created_at_str)
                .map(|dt| dt.with_timezone(&Utc))
                .unwrap_or_else(|_| Utc::now());
            let updated_at = DateTime::parse_from_rfc3339(&updated_at_str)
                .map(|dt| dt.with_timezone(&Utc))
                .unwrap_or_else(|_| Utc::now());

            let capture_policy = serde_json::from_str(&cp_str).unwrap_or_default();
            let context_budget = serde_json::from_str(&cb_str).unwrap_or_default();
            let document_config = serde_json::from_str(&dc_str).unwrap_or_default();
            let sync_config = serde_json::from_str(&sc_str).unwrap_or_default();

            Ok(Project {
                id: row.get(0)?,
                slug: row.get(1)?,
                display_name: row.get(2)?,
                created_at,
                updated_at,
                capture_policy,
                context_budget,
                document_config,
                sync_config,
            })
        })?;

        let mut list = Vec::new();
        for p in rows {
            list.push(p?);
        }
        Ok(list)
    }

    pub fn add_project_path(&self, project_id: &str, local_path: &str, is_primary: bool) -> Result<()> {
        let conn = self.conn();
        conn.execute(
            r#"
            INSERT INTO project_paths (project_id, local_path, is_primary, created_at)
            VALUES (?1, ?2, ?3, ?4)
            ON CONFLICT(project_id, local_path) DO UPDATE SET is_primary = excluded.is_primary
            "#,
            params![project_id, local_path, if is_primary { 1 } else { 0 }, Utc::now().to_rfc3339()],
        )?;
        Ok(())
    }

    pub fn get_project_paths(&self, project_id: &str) -> Result<Vec<ProjectPath>> {
        let conn = self.conn();
        let mut stmt = conn.prepare(
            "SELECT project_id, local_path, is_primary, created_at FROM project_paths WHERE project_id = ?1 ORDER BY is_primary DESC",
        )?;
        let rows = stmt.query_map(params![project_id], |row| {
            let created_at_str: String = row.get(3)?;
            let created_at = DateTime::parse_from_rfc3339(&created_at_str)
                .map(|dt| dt.with_timezone(&Utc))
                .unwrap_or_else(|_| Utc::now());
            let is_primary_int: i32 = row.get(2)?;
            Ok(ProjectPath {
                project_id: row.get(0)?,
                local_path: row.get(1)?,
                is_primary: is_primary_int != 0,
                created_at,
            })
        })?;

        let mut paths = Vec::new();
        for p in rows {
            paths.push(p?);
        }
        Ok(paths)
    }

    pub fn find_project_by_path(&self, path: &str) -> Result<Option<Project>> {
        let conn = self.conn();
        // Exact match first, or path starts with registered root
        let mut stmt = conn.prepare(
            r#"
            SELECT p.id, p.slug, p.display_name, p.created_at, p.updated_at,
                   p.capture_policy, p.context_budget, p.document_config, p.sync_config
            FROM projects p
            JOIN project_paths pp ON p.id = pp.project_id
            WHERE ?1 = pp.local_path OR ?1 LIKE (pp.local_path || '/%')
            ORDER BY LENGTH(pp.local_path) DESC
            LIMIT 1
            "#,
        )?;
        let mut rows = stmt.query(params![path])?;
        if let Some(row) = rows.next()? {
            let created_at_str: String = row.get(3)?;
            let updated_at_str: String = row.get(4)?;
            let cp_str: String = row.get(5)?;
            let cb_str: String = row.get(6)?;
            let dc_str: String = row.get(7)?;
            let sc_str: String = row.get(8)?;

            Ok(Some(Project {
                id: row.get(0)?,
                slug: row.get(1)?,
                display_name: row.get(2)?,
                created_at: DateTime::parse_from_rfc3339(&created_at_str)?.with_timezone(&Utc),
                updated_at: DateTime::parse_from_rfc3339(&updated_at_str)?.with_timezone(&Utc),
                capture_policy: serde_json::from_str(&cp_str)?,
                context_budget: serde_json::from_str(&cb_str)?,
                document_config: serde_json::from_str(&dc_str)?,
                sync_config: serde_json::from_str(&sc_str)?,
            }))
        } else {
            Ok(None)
        }
    }

    // ==========================================
    // Tool Events (Append-only Ledger)
    // ==========================================

    pub fn append_event(&self, event: &NormalizedEvent) -> Result<()> {
        let conn = self.conn();
        let (tool_name, tool_category, tool_input, tool_input_hash, tool_output, tool_output_hash, status, exit_code, duration_ms) =
            if let Some(ref t) = event.tool {
                (
                    Some(t.name.clone()),
                    Some(t.category.clone()),
                    t.input.as_ref().map(|v| v.to_string()),
                    t.input_hash.clone(),
                    t.output.as_ref().map(|v| v.to_string()),
                    t.output_hash.clone(),
                    t.status.clone(),
                    t.exit_code,
                    t.duration_ms.map(|d| d as i64),
                )
            } else {
                (None, None, None, None, None, None, "success".to_string(), None, None)
            };

        let artifacts_json = serde_json::to_string(&event.artifacts)?;
        let raw_payload_json = event.raw_payload.as_ref().map(|v| v.to_string());

        conn.execute(
            r#"
            INSERT INTO tool_events (
                event_id, event_type, session_id, task_id, project_id,
                agent_adapter, tool_name, tool_category, tool_input, tool_input_hash,
                tool_output, tool_output_hash, status, exit_code, duration_ms,
                artifacts, sensitivity, timestamp, processed, raw_payload
            ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15, ?16, ?17, ?18, 0, ?19)
            ON CONFLICT(event_id) DO NOTHING
            "#,
            params![
                event.event_id,
                event.event_type,
                event.session_id,
                event.task_id,
                event.project.id,
                event.agent.adapter_id,
                tool_name,
                tool_category,
                tool_input,
                tool_input_hash,
                tool_output,
                tool_output_hash,
                status,
                exit_code,
                duration_ms,
                artifacts_json,
                event.sensitivity,
                event.timestamp.to_rfc3339(),
                raw_payload_json,
            ],
        )?;
        Ok(())
    }

    pub fn get_unprocessed_events(&self, limit: usize) -> Result<Vec<NormalizedEvent>> {
        let conn = self.conn();
        let mut stmt = conn.prepare(
            r#"
            SELECT event_id, event_type, session_id, task_id, project_id,
                   agent_adapter, tool_name, tool_category, tool_input, tool_input_hash,
                   tool_output, tool_output_hash, status, exit_code, duration_ms,
                   artifacts, sensitivity, timestamp, raw_payload
            FROM tool_events
            WHERE processed = 0
            ORDER BY timestamp ASC
            LIMIT ?1
            "#,
        )?;

        let rows = stmt.query_map(params![limit as i64], |row| {
            let event_id: String = row.get(0)?;
            let event_type: String = row.get(1)?;
            let session_id: String = row.get(2)?;
            let task_id: Option<String> = row.get(3)?;
            let project_id: Option<String> = row.get(4)?;
            let agent_adapter: String = row.get(5)?;

            let tool_name: Option<String> = row.get(6)?;
            let tool_category: Option<String> = row.get(7)?;
            let tool_input_str: Option<String> = row.get(8)?;
            let tool_input_hash: Option<String> = row.get(9)?;
            let tool_output_str: Option<String> = row.get(10)?;
            let tool_output_hash: Option<String> = row.get(11)?;
            let status: String = row.get(12)?;
            let exit_code: Option<i32> = row.get(13)?;
            let duration_ms: Option<i64> = row.get(14)?;

            let artifacts_str: String = row.get(15)?;
            let sensitivity: String = row.get(16)?;
            let timestamp_str: String = row.get(17)?;
            let raw_payload_str: Option<String> = row.get(18)?;

            let timestamp = DateTime::parse_from_rfc3339(&timestamp_str)
                .map(|dt| dt.with_timezone(&Utc))
                .unwrap_or_else(|_| Utc::now());

            let tool = tool_name.map(|name| ToolInfo {
                name,
                category: tool_category.unwrap_or_else(|| "tool".to_string()),
                input: tool_input_str.and_then(|s| serde_json::from_str(&s).ok()),
                input_hash: tool_input_hash,
                output: tool_output_str.and_then(|s| serde_json::from_str(&s).ok()),
                output_hash: tool_output_hash,
                status,
                exit_code,
                duration_ms: duration_ms.map(|d| d as u64),
            });

            let artifacts: Vec<ArtifactRef> = serde_json::from_str(&artifacts_str).unwrap_or_default();
            let raw_payload = raw_payload_str.and_then(|s| serde_json::from_str(&s).ok());

            Ok(NormalizedEvent {
                event_id,
                event_type,
                timestamp,
                agent: AgentInfo {
                    adapter_id: agent_adapter,
                    agent_instance_id: None,
                },
                session_id,
                task_id,
                project: ProjectRef {
                    id: project_id,
                    slug: None,
                    root_path: String::new(),
                    source_git_remote: None,
                },
                tool,
                artifacts,
                sensitivity,
                schema_version: 1,
                raw_payload,
            })
        })?;

        let mut events = Vec::new();
        for e in rows {
            events.push(e?);
        }
        Ok(events)
    }

    pub fn mark_event_processed(&self, event_id: &str) -> Result<()> {
        let conn = self.conn();
        conn.execute("UPDATE tool_events SET processed = 1 WHERE event_id = ?1", params![event_id])?;
        Ok(())
    }

    pub fn list_recent_events(&self, limit: usize) -> Result<Vec<NormalizedEvent>> {
        let conn = self.conn();
        let mut stmt = conn.prepare(
            r#"
            SELECT event_id, event_type, session_id, task_id, project_id,
                   agent_adapter, tool_name, tool_category, tool_input, tool_input_hash,
                   tool_output, tool_output_hash, status, exit_code, duration_ms,
                   artifacts, sensitivity, timestamp, raw_payload
            FROM tool_events
            ORDER BY timestamp DESC
            LIMIT ?1
            "#,
        )?;

        let rows = stmt.query_map(params![limit as i64], |row| {
            let event_id: String = row.get(0)?;
            let event_type: String = row.get(1)?;
            let session_id: String = row.get(2)?;
            let task_id: Option<String> = row.get(3)?;
            let project_id: Option<String> = row.get(4)?;
            let agent_adapter: String = row.get(5)?;

            let tool_name: Option<String> = row.get(6)?;
            let tool_category: Option<String> = row.get(7)?;
            let tool_input_str: Option<String> = row.get(8)?;
            let tool_input_hash: Option<String> = row.get(9)?;
            let tool_output_str: Option<String> = row.get(10)?;
            let tool_output_hash: Option<String> = row.get(11)?;
            let status: String = row.get(12)?;
            let exit_code: Option<i32> = row.get(13)?;
            let duration_ms: Option<i64> = row.get(14)?;

            let artifacts_str: String = row.get(15)?;
            let sensitivity: String = row.get(16)?;
            let timestamp_str: String = row.get(17)?;
            let raw_payload_str: Option<String> = row.get(18)?;

            let timestamp = DateTime::parse_from_rfc3339(&timestamp_str)
                .map(|dt| dt.with_timezone(&Utc))
                .unwrap_or_else(|_| Utc::now());

            let tool = tool_name.map(|name| ToolInfo {
                name,
                category: tool_category.unwrap_or_else(|| "tool".to_string()),
                input: tool_input_str.and_then(|s| serde_json::from_str(&s).ok()),
                input_hash: tool_input_hash,
                output: tool_output_str.and_then(|s| serde_json::from_str(&s).ok()),
                output_hash: tool_output_hash,
                status,
                exit_code,
                duration_ms: duration_ms.map(|d| d as u64),
            });

            let artifacts: Vec<ArtifactRef> = serde_json::from_str(&artifacts_str).unwrap_or_default();
            let raw_payload = raw_payload_str.and_then(|s| serde_json::from_str(&s).ok());

            Ok(NormalizedEvent {
                event_id,
                event_type,
                timestamp,
                agent: AgentInfo {
                    adapter_id: agent_adapter,
                    agent_instance_id: None,
                },
                session_id,
                task_id,
                project: ProjectRef {
                    id: project_id,
                    slug: None,
                    root_path: String::new(),
                    source_git_remote: None,
                },
                tool,
                artifacts,
                sensitivity,
                schema_version: 1,
                raw_payload,
            })
        })?;

        let mut events = Vec::new();
        for e in rows {
            events.push(e?);
        }
        Ok(events)
    }

    // ==========================================
    // Memories & Versions
    // ==========================================

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
            sql.push_str(&format!(" AND (scope = 'global' OR project_id = '{}')", pid));
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
            sql.push_str(&format!(" AND (m.scope = 'global' OR m.project_id = '{}')", pid));
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

    pub fn count_memories(&self, project_id: Option<&str>) -> Result<(i64, i64)> {
        let conn = self.conn();
        let active: i64 = if let Some(pid) = project_id {
            conn.query_row(
                "SELECT COUNT(*) FROM memories WHERE status = 'active' AND (scope = 'global' OR project_id = ?1)",
                params![pid],
                |r| r.get(0),
            )?
        } else {
            conn.query_row(
                "SELECT COUNT(*) FROM memories WHERE status = 'active'",
                [],
                |r| r.get(0),
            )?
        };

        let archived: i64 = if let Some(pid) = project_id {
            conn.query_row(
                "SELECT COUNT(*) FROM memories WHERE status = 'archived' AND (scope = 'global' OR project_id = ?1)",
                params![pid],
                |r| r.get(0),
            )?
        } else {
            conn.query_row(
                "SELECT COUNT(*) FROM memories WHERE status = 'archived'",
                [],
                |r| r.get(0),
            )?
        };

        Ok((active, archived))
    }

    fn row_to_memory(row: &rusqlite::Row) -> rusqlite::Result<Memory> {
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
            created_at,
            updated_at,
            last_recalled_at,
            tags,
            entities,
            metadata,
        })
    }

    // ==========================================
    // Success Traces
    // ==========================================

    pub fn save_trace(&self, trace: &SuccessTrace) -> Result<()> {
        let conn = self.conn();
        conn.execute(
            r#"
            INSERT INTO success_traces (
                id, project_id, title, category, status, preconditions,
                steps, reproducibility_score, caveats, source_task_id,
                evidence_event_ids, created_at, updated_at
            ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13)
            ON CONFLICT(id) DO UPDATE SET
                title = excluded.title,
                category = excluded.category,
                status = excluded.status,
                preconditions = excluded.preconditions,
                steps = excluded.steps,
                reproducibility_score = excluded.reproducibility_score,
                caveats = excluded.caveats,
                updated_at = excluded.updated_at
            "#,
            params![
                trace.id,
                trace.project_id,
                trace.title,
                trace.category,
                trace.status,
                serde_json::to_string(&trace.preconditions)?,
                serde_json::to_string(&trace.steps)?,
                trace.reproducibility_score,
                serde_json::to_string(&trace.caveats)?,
                trace.source_task_id,
                serde_json::to_string(&trace.evidence_event_ids)?,
                trace.created_at.to_rfc3339(),
                trace.updated_at.to_rfc3339(),
            ],
        )?;
        Ok(())
    }

    pub fn get_trace_by_id(&self, id: &str) -> Result<Option<SuccessTrace>> {
        let conn = self.conn();
        let mut stmt = conn.prepare(
            "SELECT id, project_id, title, category, status, preconditions, steps, reproducibility_score, caveats, source_task_id, evidence_event_ids, created_at, updated_at FROM success_traces WHERE id = ?1",
        )?;
        let mut rows = stmt.query(params![id])?;
        if let Some(row) = rows.next()? {
            Ok(Some(Self::row_to_trace(row)?))
        } else {
            Ok(None)
        }
    }

    pub fn list_traces(&self, project_id: Option<&str>, limit: usize) -> Result<Vec<SuccessTrace>> {
        let conn = self.conn();
        let mut sql = "SELECT id, project_id, title, category, status, preconditions, steps, reproducibility_score, caveats, source_task_id, evidence_event_ids, created_at, updated_at FROM success_traces WHERE 1=1".to_string();
        if let Some(pid) = project_id {
            sql.push_str(&format!(" AND (project_id IS NULL OR project_id = '{}')", pid));
        }
        sql.push_str(" ORDER BY reproducibility_score DESC, updated_at DESC LIMIT ?1");

        let mut stmt = conn.prepare(&sql)?;
        let rows = stmt.query_map(params![limit as i64], |row| Self::row_to_trace(row))?;
        let mut list = Vec::new();
        for t in rows {
            list.push(t?);
        }
        Ok(list)
    }

    pub fn invalidate_trace(&self, id: &str) -> Result<bool> {
        let conn = self.conn();
        let rows = conn.execute(
            "UPDATE success_traces SET status = 'invalid', updated_at = ?1 WHERE id = ?2",
            params![Utc::now().to_rfc3339(), id],
        )?;
        Ok(rows > 0)
    }

    fn row_to_trace(row: &rusqlite::Row) -> rusqlite::Result<SuccessTrace> {
        let id: String = row.get(0)?;
        let project_id: Option<String> = row.get(1)?;
        let title: String = row.get(2)?;
        let category: String = row.get(3)?;
        let status: String = row.get(4)?;
        let pre_str: String = row.get(5)?;
        let steps_str: String = row.get(6)?;
        let reproducibility_score: f64 = row.get(7)?;
        let caveats_str: String = row.get(8)?;
        let source_task_id: Option<String> = row.get(9)?;
        let ev_str: String = row.get(10)?;
        let created_at_str: String = row.get(11)?;
        let updated_at_str: String = row.get(12)?;

        let created_at = DateTime::parse_from_rfc3339(&created_at_str)
            .map(|dt| dt.with_timezone(&Utc))
            .unwrap_or_else(|_| Utc::now());
        let updated_at = DateTime::parse_from_rfc3339(&updated_at_str)
            .map(|dt| dt.with_timezone(&Utc))
            .unwrap_or_else(|_| Utc::now());

        let preconditions = serde_json::from_str(&pre_str).unwrap_or_default();
        let steps = serde_json::from_str(&steps_str).unwrap_or_default();
        let caveats = serde_json::from_str(&caveats_str).unwrap_or_default();
        let evidence_event_ids = serde_json::from_str(&ev_str).unwrap_or_default();

        Ok(SuccessTrace {
            id,
            project_id,
            title,
            category,
            status,
            preconditions,
            steps,
            reproducibility_score,
            caveats,
            source_task_id,
            evidence_event_ids,
            created_at,
            updated_at,
        })
    }

    // ==========================================
    // Failure Records
    // ==========================================

    pub fn save_failure(&self, failure: &FailureRecord) -> Result<()> {
        let conn = self.conn();
        conn.execute(
            r#"
            INSERT INTO failure_records (
                id, project_id, task_title, category, failed_tool, error_message,
                root_cause_hypothesis, fix_applied, verification_result, status,
                evidence_event_ids, created_at, updated_at
            ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13)
            ON CONFLICT(id) DO UPDATE SET
                task_title = excluded.task_title,
                category = excluded.category,
                failed_tool = excluded.failed_tool,
                error_message = excluded.error_message,
                root_cause_hypothesis = excluded.root_cause_hypothesis,
                fix_applied = excluded.fix_applied,
                verification_result = excluded.verification_result,
                status = excluded.status,
                updated_at = excluded.updated_at
            "#,
            params![
                failure.id,
                failure.project_id,
                failure.task_title,
                failure.category,
                failure.failed_tool,
                failure.error_message,
                failure.root_cause_hypothesis,
                failure.fix_applied,
                failure.verification_result,
                failure.status,
                serde_json::to_string(&failure.evidence_event_ids)?,
                failure.created_at.to_rfc3339(),
                failure.updated_at.to_rfc3339(),
            ],
        )?;
        Ok(())
    }

    pub fn get_failure_by_id(&self, id: &str) -> Result<Option<FailureRecord>> {
        let conn = self.conn();
        let mut stmt = conn.prepare(
            "SELECT id, project_id, task_title, category, failed_tool, error_message, root_cause_hypothesis, fix_applied, verification_result, status, evidence_event_ids, created_at, updated_at FROM failure_records WHERE id = ?1",
        )?;
        let mut rows = stmt.query(params![id])?;
        if let Some(row) = rows.next()? {
            Ok(Some(Self::row_to_failure(row)?))
        } else {
            Ok(None)
        }
    }

    pub fn list_failures(
        &self,
        project_id: Option<&str>,
        status: Option<&str>,
        limit: usize,
    ) -> Result<Vec<FailureRecord>> {
        let conn = self.conn();
        let mut sql = "SELECT id, project_id, task_title, category, failed_tool, error_message, root_cause_hypothesis, fix_applied, verification_result, status, evidence_event_ids, created_at, updated_at FROM failure_records WHERE 1=1".to_string();
        if let Some(pid) = project_id {
            sql.push_str(&format!(" AND (project_id IS NULL OR project_id = '{}')", pid));
        }
        if let Some(s) = status {
            sql.push_str(&format!(" AND status = '{}'", s));
        }
        sql.push_str(" ORDER BY updated_at DESC LIMIT ?1");

        let mut stmt = conn.prepare(&sql)?;
        let rows = stmt.query_map(params![limit as i64], |row| Self::row_to_failure(row))?;
        let mut list = Vec::new();
        for f in rows {
            list.push(f?);
        }
        Ok(list)
    }

    pub fn resolve_failure(&self, id: &str, fix: &str, verification: &str) -> Result<bool> {
        let conn = self.conn();
        let rows = conn.execute(
            "UPDATE failure_records SET status = 'resolved', fix_applied = ?1, verification_result = ?2, updated_at = ?3 WHERE id = ?4",
            params![fix, verification, Utc::now().to_rfc3339(), id],
        )?;
        Ok(rows > 0)
    }

    fn row_to_failure(row: &rusqlite::Row) -> rusqlite::Result<FailureRecord> {
        let id: String = row.get(0)?;
        let project_id: Option<String> = row.get(1)?;
        let task_title: String = row.get(2)?;
        let category: Option<String> = row.get(3)?;
        let failed_tool: Option<String> = row.get(4)?;
        let error_message: String = row.get(5)?;
        let root_cause_hypothesis: Option<String> = row.get(6)?;
        let fix_applied: Option<String> = row.get(7)?;
        let verification_result: Option<String> = row.get(8)?;
        let status: String = row.get(9)?;
        let ev_str: String = row.get(10)?;
        let created_at_str: String = row.get(11)?;
        let updated_at_str: String = row.get(12)?;

        let created_at = DateTime::parse_from_rfc3339(&created_at_str)
            .map(|dt| dt.with_timezone(&Utc))
            .unwrap_or_else(|_| Utc::now());
        let updated_at = DateTime::parse_from_rfc3339(&updated_at_str)
            .map(|dt| dt.with_timezone(&Utc))
            .unwrap_or_else(|_| Utc::now());

        let evidence_event_ids = serde_json::from_str(&ev_str).unwrap_or_default();

        Ok(FailureRecord {
            id,
            project_id,
            task_title,
            category,
            failed_tool,
            error_message,
            root_cause_hypothesis,
            fix_applied,
            verification_result,
            status,
            evidence_event_ids,
            created_at,
            updated_at,
        })
    }

    // ==========================================
    // Preferences
    // ==========================================

    pub fn set_preference(&self, pref: &Preference) -> Result<()> {
        let conn = self.conn();
        conn.execute(
            r#"
            INSERT INTO preferences (id, key, value, scope, project_id, locked, source, created_at, updated_at)
            VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)
            ON CONFLICT(key, scope, COALESCE(project_id, '')) DO UPDATE SET
                value = excluded.value,
                locked = excluded.locked,
                source = excluded.source,
                updated_at = excluded.updated_at
            "#,
            params![
                pref.id,
                pref.key,
                pref.value.to_string(),
                pref.scope.to_string(),
                pref.project_id,
                if pref.locked { 1 } else { 0 },
                pref.source,
                pref.created_at.to_rfc3339(),
                pref.updated_at.to_rfc3339(),
            ],
        )?;
        Ok(())
    }

    pub fn get_preference(&self, key: &str, project_id: Option<&str>) -> Result<Option<Preference>> {
        let conn = self.conn();
        // Project preference overrides global preference
        let mut stmt = conn.prepare(
            r#"
            SELECT id, key, value, scope, project_id, locked, source, created_at, updated_at
            FROM preferences
            WHERE key = ?1 AND (
                (scope = 'project' AND project_id = ?2) OR
                (scope = 'global')
            )
            ORDER BY CASE WHEN scope = 'project' THEN 0 ELSE 1 END
            LIMIT 1
            "#,
        )?;
        let mut rows = stmt.query(params![key, project_id])?;
        if let Some(row) = rows.next()? {
            Ok(Some(Self::row_to_preference(row)?))
        } else {
            Ok(None)
        }
    }

    pub fn list_preferences(&self, project_id: Option<&str>) -> Result<Vec<Preference>> {
        let conn = self.conn();
        let mut sql = "SELECT id, key, value, scope, project_id, locked, source, created_at, updated_at FROM preferences WHERE 1=1".to_string();
        if let Some(pid) = project_id {
            sql.push_str(&format!(" AND (scope = 'global' OR project_id = '{}')", pid));
        }
        sql.push_str(" ORDER BY scope ASC, key ASC");

        let mut stmt = conn.prepare(&sql)?;
        let rows = stmt.query_map([], |row| Self::row_to_preference(row))?;
        let mut list = Vec::new();
        for p in rows {
            list.push(p?);
        }
        Ok(list)
    }

    pub fn lock_preference(&self, key: &str, project_id: Option<&str>, locked: bool) -> Result<bool> {
        let conn = self.conn();
        let rows = if let Some(pid) = project_id {
            conn.execute(
                "UPDATE preferences SET locked = ?1, updated_at = ?2 WHERE key = ?3 AND project_id = ?4",
                params![if locked { 1 } else { 0 }, Utc::now().to_rfc3339(), key, pid],
            )?
        } else {
            conn.execute(
                "UPDATE preferences SET locked = ?1, updated_at = ?2 WHERE key = ?3 AND scope = 'global'",
                params![if locked { 1 } else { 0 }, Utc::now().to_rfc3339(), key],
            )?
        };
        Ok(rows > 0)
    }

    fn row_to_preference(row: &rusqlite::Row) -> rusqlite::Result<Preference> {
        let id: String = row.get(0)?;
        let key: String = row.get(1)?;
        let val_str: String = row.get(2)?;
        let scope_str: String = row.get(3)?;
        let project_id: Option<String> = row.get(4)?;
        let locked_int: i32 = row.get(5)?;
        let source: String = row.get(6)?;
        let created_at_str: String = row.get(7)?;
        let updated_at_str: String = row.get(8)?;

        let value: serde_json::Value = serde_json::from_str(&val_str).unwrap_or_default();
        let scope = MemoryScope::from_str(&scope_str).unwrap_or(MemoryScope::Global);
        let created_at = DateTime::parse_from_rfc3339(&created_at_str)
            .map(|dt| dt.with_timezone(&Utc))
            .unwrap_or_else(|_| Utc::now());
        let updated_at = DateTime::parse_from_rfc3339(&updated_at_str)
            .map(|dt| dt.with_timezone(&Utc))
            .unwrap_or_else(|_| Utc::now());

        Ok(Preference {
            id,
            key,
            value,
            scope,
            project_id,
            locked: locked_int != 0,
            source,
            created_at,
            updated_at,
        })
    }

    // ==========================================
    // Tracked Documents & Revisions
    // ==========================================

    pub fn track_document(&self, doc: &TrackedDocument, initial_content: Option<&str>) -> Result<()> {
        let conn = self.conn();
        conn.execute(
            r#"
            INSERT INTO tracked_documents (id, project_id, relative_path, content_hash, summary, last_scanned_at, is_deleted)
            VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)
            ON CONFLICT(project_id, relative_path) DO UPDATE SET
                content_hash = excluded.content_hash,
                summary = excluded.summary,
                last_scanned_at = excluded.last_scanned_at,
                is_deleted = excluded.is_deleted
            "#,
            params![
                doc.id,
                doc.project_id,
                doc.relative_path,
                doc.content_hash,
                doc.summary,
                doc.last_scanned_at.to_rfc3339(),
                if doc.is_deleted { 1 } else { 0 },
            ],
        )?;

        if let Some(content) = initial_content {
            conn.execute(
                r#"
                INSERT INTO document_revisions (id, document_id, revision_num, content, content_hash, created_at)
                VALUES (?1, ?2, 1, ?3, ?4, ?5)
                "#,
                params![
                    uuid::Uuid::new_v4().to_string(),
                    doc.id,
                    content,
                    doc.content_hash,
                    Utc::now().to_rfc3339(),
                ],
            )?;
        }

        Ok(())
    }

    pub fn list_documents(&self, project_id: &str) -> Result<Vec<TrackedDocument>> {
        let conn = self.conn();
        let mut stmt = conn.prepare(
            "SELECT id, project_id, relative_path, content_hash, summary, last_scanned_at, is_deleted FROM tracked_documents WHERE project_id = ?1 AND is_deleted = 0 ORDER BY relative_path ASC",
        )?;
        let rows = stmt.query_map(params![project_id], |row| {
            let created_at_str: String = row.get(5)?;
            let last_scanned_at = DateTime::parse_from_rfc3339(&created_at_str)
                .map(|dt| dt.with_timezone(&Utc))
                .unwrap_or_else(|_| Utc::now());
            let is_del: i32 = row.get(6)?;

            Ok(TrackedDocument {
                id: row.get(0)?,
                project_id: row.get(1)?,
                relative_path: row.get(2)?,
                content_hash: row.get(3)?,
                summary: row.get(4)?,
                last_scanned_at,
                is_deleted: is_del != 0,
            })
        })?;

        let mut list = Vec::new();
        for d in rows {
            list.push(d?);
        }
        Ok(list)
    }

    pub fn get_document_by_path(&self, project_id: &str, rel_path: &str) -> Result<Option<TrackedDocument>> {
        let conn = self.conn();
        let mut stmt = conn.prepare(
            "SELECT id, project_id, relative_path, content_hash, summary, last_scanned_at, is_deleted FROM tracked_documents WHERE project_id = ?1 AND relative_path = ?2",
        )?;
        let mut rows = stmt.query(params![project_id, rel_path])?;
        if let Some(row) = rows.next()? {
            let scanned_str: String = row.get(5)?;
            let last_scanned_at = DateTime::parse_from_rfc3339(&scanned_str)
                .map(|dt| dt.with_timezone(&Utc))
                .unwrap_or_else(|_| Utc::now());
            let is_del: i32 = row.get(6)?;

            Ok(Some(TrackedDocument {
                id: row.get(0)?,
                project_id: row.get(1)?,
                relative_path: row.get(2)?,
                content_hash: row.get(3)?,
                summary: row.get(4)?,
                last_scanned_at,
                is_deleted: is_del != 0,
            }))
        } else {
            Ok(None)
        }
    }

    // ==========================================
    // Review Proposals & Audit Log
    // ==========================================

    pub fn save_proposal(&self, p: &ReviewProposal) -> Result<()> {
        let conn = self.conn();
        conn.execute(
            r#"
            INSERT INTO review_proposals (id, project_id, proposal_type, target_id, description, suggested_action, status, created_at, decided_at)
            VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)
            ON CONFLICT(id) DO UPDATE SET status = excluded.status, decided_at = excluded.decided_at
            "#,
            params![
                p.id,
                p.project_id,
                p.proposal_type,
                p.target_id,
                p.description,
                p.suggested_action,
                p.status,
                p.created_at.to_rfc3339(),
                p.decided_at.map(|d| d.to_rfc3339()),
            ],
        )?;
        Ok(())
    }

    pub fn list_pending_proposals(&self, project_id: Option<&str>) -> Result<Vec<ReviewProposal>> {
        let conn = self.conn();
        let mut sql = "SELECT id, project_id, proposal_type, target_id, description, suggested_action, status, created_at, decided_at FROM review_proposals WHERE status = 'pending'".to_string();
        if let Some(pid) = project_id {
            sql.push_str(&format!(" AND (project_id IS NULL OR project_id = '{}')", pid));
        }
        sql.push_str(" ORDER BY created_at DESC");

        let mut stmt = conn.prepare(&sql)?;
        let rows = stmt.query_map([], |row| {
            let created_at_str: String = row.get(7)?;
            let decided_at_str: Option<String> = row.get(8)?;

            let created_at = DateTime::parse_from_rfc3339(&created_at_str)
                .map(|dt| dt.with_timezone(&Utc))
                .unwrap_or_else(|_| Utc::now());
            let decided_at = decided_at_str.and_then(|s| {
                DateTime::parse_from_rfc3339(&s)
                    .map(|dt| dt.with_timezone(&Utc))
                    .ok()
            });

            Ok(ReviewProposal {
                id: row.get(0)?,
                project_id: row.get(1)?,
                proposal_type: row.get(2)?,
                target_id: row.get(3)?,
                description: row.get(4)?,
                suggested_action: row.get(5)?,
                status: row.get(6)?,
                created_at,
                decided_at,
            })
        })?;

        let mut list = Vec::new();
        for p in rows {
            list.push(p?);
        }
        Ok(list)
    }

    pub fn decide_proposal(&self, id: &str, approved: bool) -> Result<bool> {
        let conn = self.conn();
        let status = if approved { "approved" } else { "rejected" };
        let rows = conn.execute(
            "UPDATE review_proposals SET status = ?1, decided_at = ?2 WHERE id = ?3",
            params![status, Utc::now().to_rfc3339(), id],
        )?;
        Ok(rows > 0)
    }

    pub fn log_audit(
        &self,
        action: &str,
        entity_type: &str,
        entity_id: &str,
        details: &serde_json::Value,
    ) -> Result<()> {
        let conn = self.conn();
        conn.execute(
            r#"
            INSERT INTO audit_log (id, action, entity_type, entity_id, details, timestamp)
            VALUES (?1, ?2, ?3, ?4, ?5, ?6)
            "#,
            params![
                uuid::Uuid::new_v4().to_string(),
                action,
                entity_type,
                entity_id,
                details.to_string(),
                Utc::now().to_rfc3339(),
            ],
        )?;
        Ok(())
    }
}
