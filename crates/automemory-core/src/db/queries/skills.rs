use crate::db::Database;
use crate::models::ProceduralSkill;
use anyhow::Result;
use chrono::{DateTime, Utc};
use rusqlite::params;

impl Database {
    pub fn save_skill(&self, skill: &ProceduralSkill) -> Result<()> {
        let conn = self.conn();
        conn.execute(
            r#"
            INSERT INTO procedural_skills (
                id, project_id, name, description, trigger_patterns, preconditions,
                steps, caveats, success_count, failure_count, source_trace_ids,
                created_at, updated_at
            ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13)
            ON CONFLICT(id) DO UPDATE SET
                name = excluded.name,
                description = excluded.description,
                trigger_patterns = excluded.trigger_patterns,
                preconditions = excluded.preconditions,
                steps = excluded.steps,
                caveats = excluded.caveats,
                success_count = excluded.success_count,
                failure_count = excluded.failure_count,
                source_trace_ids = excluded.source_trace_ids,
                updated_at = excluded.updated_at
            "#,
            params![
                skill.id,
                skill.project_id,
                skill.name,
                skill.description,
                serde_json::to_string(&skill.trigger_patterns)?,
                serde_json::to_string(&skill.preconditions)?,
                serde_json::to_string(&skill.steps)?,
                serde_json::to_string(&skill.caveats)?,
                skill.success_count,
                skill.failure_count,
                serde_json::to_string(&skill.source_trace_ids)?,
                skill.created_at.to_rfc3339(),
                skill.updated_at.to_rfc3339(),
            ],
        )?;
        Ok(())
    }

    pub fn get_skill_by_id(&self, id: &str) -> Result<Option<ProceduralSkill>> {
        let conn = self.conn();
        let mut stmt = conn.prepare(
            "SELECT id, project_id, name, description, trigger_patterns, preconditions, steps, caveats, success_count, failure_count, source_trace_ids, created_at, updated_at FROM procedural_skills WHERE id = ?1",
        )?;
        let mut rows = stmt.query(params![id])?;
        if let Some(row) = rows.next()? {
            Ok(Some(Self::row_to_skill(row)?))
        } else {
            Ok(None)
        }
    }

    pub fn list_skills(&self, project_id: Option<&str>, limit: usize) -> Result<Vec<ProceduralSkill>> {
        let conn = self.conn();
        let mut sql = "SELECT id, project_id, name, description, trigger_patterns, preconditions, steps, caveats, success_count, failure_count, source_trace_ids, created_at, updated_at FROM procedural_skills WHERE 1=1".to_string();
        if let Some(pid) = project_id {
            sql.push_str(&format!(" AND (project_id IS NULL OR project_id = '{}')", pid));
        }
        sql.push_str(" ORDER BY success_count DESC, updated_at DESC LIMIT ?1");

        let mut stmt = conn.prepare(&sql)?;
        let rows = stmt.query_map(params![limit as i64], |row| Self::row_to_skill(row))?;
        let mut list = Vec::new();
        for s in rows {
            list.push(s?);
        }
        Ok(list)
    }

    pub fn delete_skill(&self, id: &str) -> Result<bool> {
        let conn = self.conn();
        let rows = conn.execute("DELETE FROM procedural_skills WHERE id = ?1", params![id])?;
        Ok(rows > 0)
    }

    fn row_to_skill(row: &rusqlite::Row) -> rusqlite::Result<ProceduralSkill> {
        let id: String = row.get(0)?;
        let project_id: Option<String> = row.get(1)?;
        let name: String = row.get(2)?;
        let description: String = row.get(3)?;
        let triggers_str: String = row.get(4)?;
        let preconds_str: String = row.get(5)?;
        let steps_str: String = row.get(6)?;
        let caveats_str: String = row.get(7)?;
        let success_count: u32 = row.get(8)?;
        let failure_count: u32 = row.get(9)?;
        let traces_str: String = row.get(10)?;
        let created_at_str: String = row.get(11)?;
        let updated_at_str: String = row.get(12)?;

        let created_at = DateTime::parse_from_rfc3339(&created_at_str)
            .map(|dt| dt.with_timezone(&Utc))
            .unwrap_or_else(|_| Utc::now());
        let updated_at = DateTime::parse_from_rfc3339(&updated_at_str)
            .map(|dt| dt.with_timezone(&Utc))
            .unwrap_or_else(|_| Utc::now());

        let trigger_patterns = serde_json::from_str(&triggers_str).unwrap_or_default();
        let preconditions = serde_json::from_str(&preconds_str).unwrap_or_default();
        let steps = serde_json::from_str(&steps_str).unwrap_or_default();
        let caveats = serde_json::from_str(&caveats_str).unwrap_or_default();
        let source_trace_ids = serde_json::from_str(&traces_str).unwrap_or_default();

        Ok(ProceduralSkill {
            id,
            project_id,
            name,
            description,
            trigger_patterns,
            preconditions,
            steps,
            caveats,
            success_count,
            failure_count,
            source_trace_ids,
            created_at,
            updated_at,
        })
    }
}
