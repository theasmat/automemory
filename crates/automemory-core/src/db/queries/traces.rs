use crate::db::Database;
use crate::models::SuccessTrace;
use anyhow::Result;
use chrono::{DateTime, Utc};
use rusqlite::params;

impl Database {
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
            sql.push_str(&format!(
                " AND (project_id IS NULL OR project_id = '{}')",
                pid
            ));
        }
        sql.push_str(" ORDER BY reproducibility_score DESC, updated_at DESC LIMIT ?1");

        let mut stmt = conn.prepare(&sql)?;
        let rows = stmt.query_map(params![limit as i64], Self::row_to_trace)?;
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
}
