use crate::db::Database;
use anyhow::Result;
use rusqlite::params;

impl Database {
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

    pub fn get_token_stats(&self, project_id: Option<&str>) -> Result<(usize, usize)> {
        let conn = self.conn();
        let mem_chars: i64 = if let Some(pid) = project_id {
            conn.query_row(
                "SELECT COALESCE(SUM(LENGTH(content) + LENGTH(summary)), 0) FROM memories WHERE status = 'active' AND (scope = 'global' OR project_id = ?1)",
                params![pid],
                |r| r.get(0),
            ).unwrap_or(0)
        } else {
            conn.query_row(
                "SELECT COALESCE(SUM(LENGTH(content) + LENGTH(summary)), 0) FROM memories WHERE status = 'active'",
                [],
                |r| r.get(0),
            ).unwrap_or(0)
        };
        let mem_tokens = (mem_chars as usize).div_ceil(4);

        let event_chars: i64 = if let Some(pid) = project_id {
            conn.query_row(
                "SELECT COALESCE(SUM(LENGTH(COALESCE(tool_input, '')) + LENGTH(COALESCE(tool_output, ''))), 0) FROM tool_events WHERE project_id = ?1",
                params![pid],
                |r| r.get(0),
            ).unwrap_or(0)
        } else {
            conn.query_row(
                "SELECT COALESCE(SUM(LENGTH(COALESCE(tool_input, '')) + LENGTH(COALESCE(tool_output, ''))), 0) FROM tool_events",
                [],
                |r| r.get(0),
            ).unwrap_or(0)
        };
        let event_tokens = (event_chars as usize).div_ceil(4);

        Ok((mem_tokens, event_tokens))
    }

    pub fn pulse_counts(&self, project_id: Option<&str>) -> Result<(usize, usize, usize)> {
        let conn = self.conn();
        let traces: i64 = if let Some(pid) = project_id {
            conn.query_row("SELECT COUNT(*) FROM success_traces WHERE project_id = ?1", params![pid], |r| r.get(0)).unwrap_or(0)
        } else {
            conn.query_row("SELECT COUNT(*) FROM success_traces", [], |r| r.get(0)).unwrap_or(0)
        };
        let failures: i64 = if let Some(pid) = project_id {
            conn.query_row("SELECT COUNT(*) FROM failure_records WHERE status = 'unresolved' AND project_id = ?1", params![pid], |r| r.get(0)).unwrap_or(0)
        } else {
            conn.query_row("SELECT COUNT(*) FROM failure_records WHERE status = 'unresolved'", [], |r| r.get(0)).unwrap_or(0)
        };
        let proposals: i64 = if let Some(pid) = project_id {
            conn.query_row("SELECT COUNT(*) FROM review_proposals WHERE status = 'pending' AND (project_id IS NULL OR project_id = ?1)", params![pid], |r| r.get(0)).unwrap_or(0)
        } else {
            conn.query_row("SELECT COUNT(*) FROM review_proposals WHERE status = 'pending'", [], |r| r.get(0)).unwrap_or(0)
        };
        Ok((traces as usize, failures as usize, proposals as usize))
    }
}
