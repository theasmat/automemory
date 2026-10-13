use crate::db::Database;
use crate::models::{
    AgentInfo, ArtifactRef, NormalizedEvent, ProjectRef, SessionSummary, ToolInfo,
};
use anyhow::Result;
use chrono::{DateTime, Utc};
use rusqlite::params;

impl Database {
    pub fn append_event(&self, event: &NormalizedEvent) -> Result<()> {
        let conn = self.conn();
        let (
            tool_name,
            tool_category,
            tool_input,
            tool_input_hash,
            tool_output,
            tool_output_hash,
            status,
            exit_code,
            duration_ms,
        ) = if let Some(ref t) = event.tool {
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
            (
                None,
                None,
                None,
                None,
                None,
                None,
                "success".to_string(),
                None,
                None,
            )
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

            let artifacts: Vec<ArtifactRef> =
                serde_json::from_str(&artifacts_str).unwrap_or_default();
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
        conn.execute(
            "UPDATE tool_events SET processed = 1 WHERE event_id = ?1",
            params![event_id],
        )?;
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

            let artifacts: Vec<ArtifactRef> =
                serde_json::from_str(&artifacts_str).unwrap_or_default();
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

    pub fn list_sessions(
        &self,
        project_id: Option<&str>,
        limit: usize,
    ) -> Result<Vec<SessionSummary>> {
        let conn = self.conn();
        let res = if let Some(pid) = project_id {
            let mut stmt = conn.prepare(
                r#"
                SELECT session_id, project_id, agent_adapter, COUNT(event_id) as event_count,
                       MIN(timestamp) as started_at, MAX(timestamp) as last_event_at
                FROM tool_events
                WHERE project_id = ?1
                GROUP BY session_id
                ORDER BY last_event_at DESC
                LIMIT ?2
                "#,
            )?;
            let rows = stmt.query_map(params![pid, limit as i64], Self::row_to_session_summary)?;
            let mut items = Vec::new();
            for r in rows {
                items.push(r?);
            }
            items
        } else {
            let mut stmt = conn.prepare(
                r#"
                SELECT session_id, project_id, agent_adapter, COUNT(event_id) as event_count,
                       MIN(timestamp) as started_at, MAX(timestamp) as last_event_at
                FROM tool_events
                GROUP BY session_id
                ORDER BY last_event_at DESC
                LIMIT ?1
                "#,
            )?;
            let rows = stmt.query_map(params![limit as i64], Self::row_to_session_summary)?;
            let mut items = Vec::new();
            for r in rows {
                items.push(r?);
            }
            items
        };
        Ok(res)
    }

    fn row_to_session_summary(row: &rusqlite::Row) -> rusqlite::Result<SessionSummary> {
        let session_id: String = row.get(0)?;
        let project_id: Option<String> = row.get(1)?;
        let agent_adapter: String = row.get(2)?;
        let event_count: i64 = row.get(3)?;
        let started_at_str: String = row.get(4)?;
        let last_event_at_str: String = row.get(5)?;

        let started_at = DateTime::parse_from_rfc3339(&started_at_str)
            .map(|dt| dt.with_timezone(&Utc))
            .unwrap_or_else(|_| Utc::now());
        let last_event_at = DateTime::parse_from_rfc3339(&last_event_at_str)
            .map(|dt| dt.with_timezone(&Utc))
            .unwrap_or_else(|_| Utc::now());

        Ok(SessionSummary {
            session_id,
            project_id,
            agent_adapter,
            event_count: event_count as usize,
            started_at,
            last_event_at,
            status: "completed".to_string(),
        })
    }

    pub fn get_session_events(&self, session_id: &str) -> Result<Vec<NormalizedEvent>> {
        let conn = self.conn();
        let mut stmt = conn.prepare(
            r#"
            SELECT event_id, event_type, session_id, task_id, project_id,
                   agent_adapter, tool_name, tool_category, tool_input, tool_input_hash,
                   tool_output, tool_output_hash, status, exit_code, duration_ms,
                   artifacts, sensitivity, timestamp, raw_payload
            FROM tool_events
            WHERE session_id = ?1
            ORDER BY timestamp ASC
            "#,
        )?;

        let rows = stmt.query_map(params![session_id], |row| {
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

            let artifacts: Vec<ArtifactRef> =
                serde_json::from_str(&artifacts_str).unwrap_or_default();
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
        for r in rows {
            events.push(r?);
        }
        Ok(events)
    }

    pub fn get_events_between(
        &self,
        project_id: Option<&str>,
        start_time: DateTime<Utc>,
        end_time: DateTime<Utc>,
    ) -> Result<Vec<NormalizedEvent>> {
        let conn = self.conn();
        let start_str = start_time.to_rfc3339();
        let end_str = end_time.to_rfc3339();

        let mut events = Vec::new();
        if let Some(pid) = project_id {
            let mut stmt = conn.prepare(
                r#"
                SELECT event_id, event_type, session_id, task_id, project_id,
                       agent_adapter, tool_name, tool_category, tool_input, tool_input_hash,
                       tool_output, tool_output_hash, status, exit_code, duration_ms,
                       artifacts, sensitivity, timestamp, raw_payload
                FROM tool_events
                WHERE (project_id = ?1 OR project_id IS NULL)
                  AND timestamp >= ?2 AND timestamp <= ?3
                ORDER BY timestamp ASC
                "#,
            )?;
            let rows = stmt.query_map(params![pid, start_str, end_str], parse_event_row)?;
            for r in rows {
                events.push(r?);
            }
        } else {
            let mut stmt = conn.prepare(
                r#"
                SELECT event_id, event_type, session_id, task_id, project_id,
                       agent_adapter, tool_name, tool_category, tool_input, tool_input_hash,
                       tool_output, tool_output_hash, status, exit_code, duration_ms,
                       artifacts, sensitivity, timestamp, raw_payload
                FROM tool_events
                WHERE timestamp >= ?1 AND timestamp <= ?2
                ORDER BY timestamp ASC
                "#,
            )?;
            let rows = stmt.query_map(params![start_str, end_str], parse_event_row)?;
            for r in rows {
                events.push(r?);
            }
        }

        Ok(events)
    }

    pub fn get_session_events_in_range(
        &self,
        session_id: &str,
        start_time: Option<DateTime<Utc>>,
        end_time: Option<DateTime<Utc>>,
    ) -> Result<Vec<NormalizedEvent>> {
        let conn = self.conn();
        let mut events = Vec::new();

        match (start_time, end_time) {
            (Some(st), Some(et)) => {
                let mut stmt = conn.prepare(
                    r#"
                    SELECT event_id, event_type, session_id, task_id, project_id,
                           agent_adapter, tool_name, tool_category, tool_input, tool_input_hash,
                           tool_output, tool_output_hash, status, exit_code, duration_ms,
                           artifacts, sensitivity, timestamp, raw_payload
                    FROM tool_events
                    WHERE session_id = ?1 AND timestamp >= ?2 AND timestamp <= ?3
                    ORDER BY timestamp ASC
                    "#,
                )?;
                let rows = stmt.query_map(
                    params![session_id, st.to_rfc3339(), et.to_rfc3339()],
                    parse_event_row,
                )?;
                for r in rows {
                    events.push(r?);
                }
            }
            (Some(st), None) => {
                let mut stmt = conn.prepare(
                    r#"
                    SELECT event_id, event_type, session_id, task_id, project_id,
                           agent_adapter, tool_name, tool_category, tool_input, tool_input_hash,
                           tool_output, tool_output_hash, status, exit_code, duration_ms,
                           artifacts, sensitivity, timestamp, raw_payload
                    FROM tool_events
                    WHERE session_id = ?1 AND timestamp >= ?2
                    ORDER BY timestamp ASC
                    "#,
                )?;
                let rows = stmt.query_map(params![session_id, st.to_rfc3339()], parse_event_row)?;
                for r in rows {
                    events.push(r?);
                }
            }
            (None, Some(et)) => {
                let mut stmt = conn.prepare(
                    r#"
                    SELECT event_id, event_type, session_id, task_id, project_id,
                           agent_adapter, tool_name, tool_category, tool_input, tool_input_hash,
                           tool_output, tool_output_hash, status, exit_code, duration_ms,
                           artifacts, sensitivity, timestamp, raw_payload
                    FROM tool_events
                    WHERE session_id = ?1 AND timestamp <= ?2
                    ORDER BY timestamp ASC
                    "#,
                )?;
                let rows = stmt.query_map(params![session_id, et.to_rfc3339()], parse_event_row)?;
                for r in rows {
                    events.push(r?);
                }
            }
            (None, None) => {
                return self.get_session_events(session_id);
            }
        }

        Ok(events)
    }
}

fn parse_event_row(row: &rusqlite::Row) -> rusqlite::Result<NormalizedEvent> {
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
}
