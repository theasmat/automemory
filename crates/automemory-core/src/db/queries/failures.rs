use crate::db::Database;
use crate::models::FailureRecord;
use anyhow::Result;
use chrono::{DateTime, Utc};
use rusqlite::params;

impl Database {
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
            sql.push_str(&format!(
                " AND (project_id IS NULL OR project_id = '{}')",
                pid
            ));
        }
        if let Some(s) = status {
            sql.push_str(&format!(" AND status = '{}'", s));
        }
        sql.push_str(" ORDER BY updated_at DESC LIMIT ?1");

        let mut stmt = conn.prepare(&sql)?;
        let rows = stmt.query_map(params![limit as i64], Self::row_to_failure)?;
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
}
