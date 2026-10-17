use super::sql_str;
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
                " AND (project_id = '{0}' OR project_id IN (SELECT slug FROM projects WHERE id = '{0}') OR project_id IN (SELECT id FROM projects WHERE slug = '{0}'))",
                sql_str(pid)
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

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Utc;

    fn trace(id: &str, project: Option<&str>) -> SuccessTrace {
        SuccessTrace {
            id: id.into(),
            project_id: project.map(Into::into),
            title: id.into(),
            category: "testing".into(),
            status: "active".into(),
            preconditions: vec![],
            steps: vec![],
            reproducibility_score: 0.9,
            caveats: vec![],
            source_task_id: None,
            evidence_event_ids: vec![],
            created_at: Utc::now(),
            updated_at: Utc::now(),
        }
    }

    #[test]
    fn project_filter_survives_quotes_and_does_not_leak_other_projects() {
        let db = Database::open_in_memory().unwrap();
        db.save_trace(&trace("mine", Some("bob's-app"))).unwrap();
        db.save_trace(&trace("other", Some("other-app"))).unwrap();
        db.save_trace(&trace("orphan", None)).unwrap();

        let ids = |p: &str| -> Vec<String> {
            db.list_traces(Some(p), 10)
                .unwrap()
                .into_iter()
                .map(|t| t.id)
                .collect()
        };
        assert_eq!(ids("bob's-app"), ["mine"]); // apostrophe is data, not syntax
        assert!(ids("x' OR '1'='1").is_empty()); // injection returns nothing
        assert_eq!(db.list_traces(None, 10).unwrap().len(), 3); // unscoped listing still sees all
    }
}
