use crate::db::Database;
use crate::models::{LeaseAcquireArgs, LeaseScopeType, TaskState, WorkTask};
use anyhow::Result;
use chrono::{DateTime, Utc};
use rusqlite::params;
use std::str::FromStr;
use uuid::Uuid;

const TASK_COLS: &str = "id, session_id, project_id, title, spec, symbols, owner_session, result, status, created_at, updated_at, room_id, position, assigned_agent, files, check_cmd, check_status, cancelled";

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
            room_id: None,
            position: 0,
            assigned_agent: None,
            files: vec![],
            check_cmd: None,
            check_status: None,
            cancelled: false,
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

    #[allow(clippy::too_many_arguments)]
    pub fn create_room_task(
        &self,
        room_id: &str,
        project_id: Option<&str>,
        position: i64,
        title: &str,
        spec: &str,
        assigned_agent: Option<&str>,
        files: &[String],
        check_cmd: Option<&str>,
    ) -> Result<WorkTask> {
        let now = Utc::now();
        let assigned_clean = assigned_agent.map(str::trim).filter(|s| !s.is_empty()).map(str::to_string);
        let check_clean = check_cmd.map(str::trim).filter(|s| !s.is_empty()).map(str::to_string);
        let files_clean: Vec<String> = files
            .iter()
            .map(|f| f.trim().to_string())
            .filter(|f| !f.is_empty())
            .collect();
        let task = WorkTask {
            id: format!("task-{}", Uuid::now_v7().simple()),
            session_id: format!("room:{room_id}"),
            project_id: project_id.map(str::to_string),
            title: title.to_string(),
            spec: spec.to_string(),
            symbols: files_clean.clone(),
            owner_session: None,
            result: None,
            status: TaskState::Open,
            room_id: Some(room_id.to_string()),
            position,
            assigned_agent: assigned_clean,
            files: files_clean,
            check_cmd: check_clean,
            check_status: None,
            cancelled: false,
            created_at: now,
            updated_at: now,
        };
        self.conn().execute(
            "INSERT INTO tasks (id, session_id, project_id, title, spec, symbols, owner_session, result, status, room_id, position, assigned_agent, files, check_cmd, check_status, cancelled, created_at, updated_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, NULL, NULL, 'open', ?7, ?8, ?9, ?10, ?11, NULL, 0, ?12, ?12)",
            params![
                task.id,
                task.session_id,
                task.project_id,
                task.title,
                task.spec,
                serde_json::to_string(&task.symbols)?,
                task.room_id,
                task.position,
                task.assigned_agent,
                serde_json::to_string(&task.files)?,
                task.check_cmd,
                now.to_rfc3339(),
            ],
        )?;
        Ok(task)
    }

    fn session_agent_id(&self, session_id: &str) -> Option<String> {
        self.sync_kv_get(&format!("session_agent:{session_id}"))
            .filter(|n| !n.trim().is_empty())
            .or_else(|| {
                self.conn()
                    .query_row(
                        "SELECT agent_adapter FROM tool_events WHERE session_id = ?1 LIMIT 1",
                        [session_id],
                        |r| r.get::<_, String>(0),
                    )
                    .ok()
            })
            .or_else(|| {
                let low = session_id.to_ascii_lowercase();
                if low.starts_with("claude") {
                    Some("claude".to_string())
                } else if low.starts_with("agy") || low.starts_with("antigravity") {
                    Some("agy".to_string())
                } else if low.starts_with("codex") {
                    Some("codex".to_string())
                } else {
                    None
                }
            })
    }

    /// Claims the oldest open task for `session_id`. The conditional UPDATE
    /// makes double-claims lose cleanly under concurrency. When a claimed task
    /// has scoped `files`, file leases are acquired for the claiming session.
    pub fn claim_next_task(
        &self,
        project_id: Option<&str>,
        session_id: &str,
    ) -> Result<Option<WorkTask>> {
        // Resume path: work this session already holds.
        if let Some(mine) = self.my_claimed_task(project_id, session_id)? {
            return Ok(Some(mine));
        }
        let my_agent = self.session_agent_id(session_id);
        let mut open: Vec<WorkTask> = self.list_tasks(project_id, Some(TaskState::Open), 20)?;
        open.sort_by_key(|t| {
            let is_mine = t.assigned_agent.as_deref().is_some_and(|a| {
                a == session_id || my_agent.as_deref() == Some(a)
            });
            !is_mine
        });
        for candidate in open {
            if candidate.cancelled {
                continue;
            }
            if let Some(ref assigned) = candidate.assigned_agent {
                if !assigned.is_empty() && assigned != session_id {
                    if let Some(ref me) = my_agent {
                        if me != assigned {
                            continue;
                        }
                    }
                }
            }
            let now = Utc::now().to_rfc3339();
            let claimed = self.conn().execute(
                "UPDATE tasks SET status = 'claimed', owner_session = ?1, updated_at = ?2
                 WHERE id = ?3 AND status = 'open' AND cancelled = 0",
                params![session_id, now, candidate.id],
            )?;
            if claimed == 1 {
                let agent_label = my_agent
                    .clone()
                    .or_else(|| candidate.assigned_agent.clone())
                    .unwrap_or_else(|| session_id.to_string());
                let goal = format!("Task: {}", candidate.title);
                for file in &candidate.files {
                    let _ = self.acquire_lease(LeaseAcquireArgs {
                        project_id: candidate.project_id.as_deref().or(project_id),
                        agent: &agent_label,
                        session_id,
                        scope: file,
                        scope_type: LeaseScopeType::File,
                        goal: &goal,
                        ttl_seconds: 600,
                    });
                }
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
            .find(|t| !t.cancelled && t.owner_session.as_deref() == Some(session_id)))
    }

    /// Hands claimed work to review. Only the owning session can submit.
    pub fn submit_task(&self, id: &str, session_id: &str, result: Option<&str>) -> Result<bool> {
        self.submit_task_with_check(id, session_id, result, None)
    }

    pub fn submit_task_with_check(
        &self,
        id: &str,
        session_id: &str,
        result: Option<&str>,
        check_status: Option<&str>,
    ) -> Result<bool> {
        let existing = self.get_task(id)?;
        let inferred_check = check_status.map(str::to_string).or_else(|| {
            let task = existing.as_ref()?;
            task.check_cmd.as_ref()?;
            let r = result?
                .to_lowercase()
                .replace("0 failed", "")
                .replace("0 failures", "")
                .replace("0 errors", "");
            if r.contains("fail") || r.contains("error") {
                Some("failed".to_string())
            } else {
                Some("passed".to_string())
            }
        });
        let now = Utc::now().to_rfc3339();
        let rows = self.conn().execute(
            "UPDATE tasks SET status = 'submitted', result = ?1, check_status = COALESCE(?2, check_status), updated_at = ?3
             WHERE id = ?4 AND owner_session = ?5 AND status = 'claimed'",
            params![result, inferred_check, now, id, session_id],
        )?;
        if rows == 1 {
            if let Some(task) = existing {
                for file in &task.files {
                    let _ = self.release_leases(session_id, Some(file));
                }
            }
            Ok(true)
        } else {
            Ok(false)
        }
    }

    pub fn get_task(&self, id: &str) -> Result<Option<WorkTask>> {
        let conn = self.conn();
        let sql = format!("SELECT {TASK_COLS} FROM tasks WHERE id = ?1");
        let mut stmt = conn.prepare(&sql)?;
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
        let sql = format!(
            "SELECT {TASK_COLS}
             FROM tasks
             WHERE (?1 IS NULL OR project_id = ?1 OR project_id IS NULL)
               AND (?2 IS NULL OR status = ?2)
               AND cancelled = 0
             ORDER BY position ASC, created_at ASC
             LIMIT ?3"
        );
        let mut stmt = conn.prepare(&sql)?;
        let tasks = stmt
            .query_map(
                params![project_id, status_str, limit as i64],
                Self::row_to_task,
            )?
            .collect::<std::result::Result<Vec<_>, _>>()?;
        Ok(tasks)
    }

    pub fn list_room_tasks(&self, room_id: &str) -> Result<Vec<WorkTask>> {
        let conn = self.conn();
        let sql = format!(
            "SELECT {TASK_COLS}
             FROM tasks
             WHERE room_id = ?1 OR session_id = ?2
             ORDER BY position ASC, created_at ASC"
        );
        let mut stmt = conn.prepare(&sql)?;
        let session_tag = format!("room:{room_id}");
        let tasks = stmt
            .query_map(params![room_id, session_tag], Self::row_to_task)?
            .collect::<std::result::Result<Vec<_>, _>>()?;
        Ok(tasks)
    }

    pub fn update_task(
        &self,
        id: &str,
        title: Option<&str>,
        assigned_agent: Option<&str>,
    ) -> Result<Option<WorkTask>> {
        let Some(existing) = self.get_task(id)? else {
            return Ok(None);
        };
        let new_title = title
            .map(str::trim)
            .filter(|t| !t.is_empty())
            .unwrap_or(&existing.title);
        let new_agent = match assigned_agent {
            Some(a) if a.trim().is_empty() => None,
            Some(a) => Some(a.trim().to_string()),
            None => existing.assigned_agent.clone(),
        };
        let now = Utc::now().to_rfc3339();
        self.conn().execute(
            "UPDATE tasks SET title = ?1, assigned_agent = ?2, updated_at = ?3 WHERE id = ?4",
            params![new_title, new_agent, now, id],
        )?;
        self.get_task(id)
    }

    pub fn cancel_task(&self, id: &str) -> Result<bool> {
        let existing = self.get_task(id)?;
        let now = Utc::now().to_rfc3339();
        let updated = self.conn().execute(
            "UPDATE tasks SET cancelled = 1, updated_at = ?1 WHERE id = ?2 AND cancelled = 0",
            params![now, id],
        )?;
        if updated == 1 {
            if let Some(t) = existing {
                if let Some(owner) = t.owner_session.as_deref() {
                    for f in &t.files {
                        let _ = self.release_leases(owner, Some(f));
                    }
                }
            }
            Ok(true)
        } else {
            Ok(false)
        }
    }

    /// Cancels all unclaimed (`open`) tasks of `room_id` and returns `(cancelled_count, claimed_tasks)`.
    pub fn cancel_unclaimed_room_tasks(&self, room_id: &str) -> Result<(usize, Vec<WorkTask>)> {
        let tasks = self.list_room_tasks(room_id)?;
        let now = Utc::now().to_rfc3339();
        let session_tag = format!("room:{room_id}");
        let cancelled = self.conn().execute(
            "UPDATE tasks SET cancelled = 1, updated_at = ?1
             WHERE (room_id = ?2 OR session_id = ?3) AND status = 'open' AND cancelled = 0",
            params![now, room_id, session_tag],
        )?;
        let claimed: Vec<WorkTask> = tasks
            .into_iter()
            .filter(|t| !t.cancelled && t.status == TaskState::Claimed)
            .collect();
        Ok((cancelled, claimed))
    }

    fn row_to_task(row: &rusqlite::Row) -> rusqlite::Result<WorkTask> {
        let syms: String = row.get(5)?;
        let status_str: String = row.get(8)?;
        let created_str: String = row.get(9)?;
        let updated_str: String = row.get(10)?;
        let files_str: String = row
            .get::<_, Option<String>>(14)?
            .unwrap_or_else(|| "[]".into());
        let cancelled_int: i64 = row.get::<_, Option<i64>>(17)?.unwrap_or(0);
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
            room_id: row.get(11)?,
            position: row.get::<_, Option<i64>>(12)?.unwrap_or(0),
            assigned_agent: row.get(13)?,
            files: serde_json::from_str(&files_str).unwrap_or_default(),
            check_cmd: row.get(15)?,
            check_status: row.get(16)?,
            cancelled: cancelled_int != 0,
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
