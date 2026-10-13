use crate::db::Database;
use crate::models::{TaskState, WorkTask};
use anyhow::Result;
use chrono::{DateTime, Utc};
use rusqlite::params;
use std::str::FromStr;
use uuid::Uuid;

impl Database {
    pub fn create_task(
        &self,
        session_id: &str,
        project_id: Option<&str>,
        title: &str,
        spec: &str,
        symbols: &[String],
    ) -> Result<WorkTask> {
        let now = Utc::now();
        let task = WorkTask {
            id: format!("task-{}", Uuid::now_v7().simple()),
            session_id: session_id.to_string(),
            project_id: project_id.map(str::to_string),
            title: title.to_string(),
            spec: spec.to_string(),
            symbols: symbols.to_vec(),
            owner_session: None,
            result: None,
            status: TaskState::Open,
            created_at: now,
            updated_at: now,
        };
        self.conn().execute(
            "INSERT INTO tasks (id, session_id, project_id, title, spec, symbols, owner_session, result, status, created_at, updated_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, NULL, NULL, 'open', ?7, ?7)",
            params![
                task.id,
                task.session_id,
                task.project_id,
                task.title,
                task.spec,
                serde_json::to_string(&task.symbols)?,
                now.to_rfc3339(),
            ],
        )?;
        Ok(task)
    }

    /// Claims the oldest open task for `session_id`. The conditional UPDATE
    /// makes double-claims lose cleanly under concurrency.
    pub fn claim_next_task(
        &self,
        project_id: Option<&str>,
        session_id: &str,
    ) -> Result<Option<WorkTask>> {
        // Resume path: work this session already holds.
        if let Some(mine) = self.my_claimed_task(project_id, session_id)? {
            return Ok(Some(mine));
        }
        let open: Vec<WorkTask> = self.list_tasks(project_id, Some(TaskState::Open), 5)?;
        for candidate in open {
            let now = Utc::now().to_rfc3339();
            let claimed = self.conn().execute(
                "UPDATE tasks SET status = 'claimed', owner_session = ?1, updated_at = ?2
                 WHERE id = ?3 AND status = 'open'",
                params![session_id, now, candidate.id],
            )?;
            if claimed == 1 {
                return self.get_task(&candidate.id);
            }
        }
        Ok(None)
    }

    fn my_claimed_task(
        &self,
        project_id: Option<&str>,
        session_id: &str,
    ) -> Result<Option<WorkTask>> {
        let tasks = self.list_tasks(project_id, Some(TaskState::Claimed), 10)?;
        Ok(tasks
            .into_iter()
            .find(|t| t.owner_session.as_deref() == Some(session_id)))
    }

    /// Hands claimed work to review. Only the owning session can submit.
    pub fn submit_task(&self, id: &str, session_id: &str, result: Option<&str>) -> Result<bool> {
        let now = Utc::now().to_rfc3339();
        let rows = self.conn().execute(
            "UPDATE tasks SET status = 'submitted', result = ?1, updated_at = ?2
             WHERE id = ?3 AND owner_session = ?4 AND status = 'claimed'",
            params![result, now, id, session_id],
        )?;
        Ok(rows == 1)
    }

    pub fn get_task(&self, id: &str) -> Result<Option<WorkTask>> {
        let conn = self.conn();
        let mut stmt = conn.prepare(
            "SELECT id, session_id, project_id, title, spec, symbols, owner_session, result, status, created_at, updated_at
             FROM tasks WHERE id = ?1",
        )?;
        let mut rows = stmt.query(params![id])?;
        if let Some(row) = rows.next()? {
            Ok(Some(Self::row_to_task(row)?))
        } else {
            Ok(None)
        }
    }

    pub fn list_tasks(
        &self,
        project_id: Option<&str>,
        status: Option<TaskState>,
        limit: usize,
    ) -> Result<Vec<WorkTask>> {
        let status_str = status.map(|s| s.to_string());
        let conn = self.conn();
        let mut stmt = conn.prepare(
            "SELECT id, session_id, project_id, title, spec, symbols, owner_session, result, status, created_at, updated_at
             FROM tasks
             WHERE (?1 IS NULL OR project_id = ?1 OR project_id IS NULL)
               AND (?2 IS NULL OR status = ?2)
             ORDER BY created_at ASC
             LIMIT ?3",
        )?;
        let tasks = stmt
            .query_map(
                params![project_id, status_str, limit as i64],
                Self::row_to_task,
            )?
            .collect::<std::result::Result<Vec<_>, _>>()?;
        Ok(tasks)
    }

    fn row_to_task(row: &rusqlite::Row) -> rusqlite::Result<WorkTask> {
        let syms: String = row.get(5)?;
        let status_str: String = row.get(8)?;
        let created_str: String = row.get(9)?;
        let updated_str: String = row.get(10)?;
        Ok(WorkTask {
            id: row.get(0)?,
            session_id: row.get(1)?,
            project_id: row.get(2)?,
            title: row.get(3)?,
            spec: row.get(4).unwrap_or_default(),
            symbols: serde_json::from_str(&syms).unwrap_or_default(),
            owner_session: row.get(6)?,
            result: row.get(7)?,
            status: TaskState::from_str(&status_str).unwrap_or(TaskState::Open),
            created_at: DateTime::parse_from_rfc3339(&created_str)
                .map(|d| d.with_timezone(&Utc))
                .unwrap_or_else(|_| Utc::now()),
            updated_at: DateTime::parse_from_rfc3339(&updated_str)
                .map(|d| d.with_timezone(&Utc))
                .unwrap_or_else(|_| Utc::now()),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn seed(db: &Database, title: &str) {
        db.create_task("creator", None, title, "spec", &["auth::login".to_string()])
            .unwrap();
    }

    #[test]
    fn claim_is_fifo_and_exclusive() {
        let db = Database::open_in_memory().unwrap();
        seed(&db, "first");
        seed(&db, "second");

        let a = db.claim_next_task(None, "sess-a").unwrap().expect("task");
        assert_eq!(a.title, "first");
        assert_eq!(a.owner_session.as_deref(), Some("sess-a"));

        // Same session resumes its own claim instead of taking the next.
        let again = db.claim_next_task(None, "sess-a").unwrap().expect("task");
        assert_eq!(again.id, a.id);

        let b = db.claim_next_task(None, "sess-b").unwrap().expect("task");
        assert_eq!(b.title, "second");

        assert!(db.claim_next_task(None, "sess-c").unwrap().is_none());
    }

    #[test]
    fn submit_requires_ownership() {
        let db = Database::open_in_memory().unwrap();
        seed(&db, "work");
        let claimed = db.claim_next_task(None, "sess-a").unwrap().expect("task");

        assert!(!db.submit_task(&claimed.id, "sess-b", None).unwrap());
        assert!(db.submit_task(&claimed.id, "sess-a", Some("done")).unwrap());
        let fetched = db.get_task(&claimed.id).unwrap().expect("task");
        assert_eq!(fetched.status, TaskState::Submitted);
        assert_eq!(fetched.result.as_deref(), Some("done"));
    }
}
