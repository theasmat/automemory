use crate::db::Database;
use crate::models::ReviewProposal;
use anyhow::Result;
use chrono::{DateTime, Utc};
use rusqlite::params;

impl Database {
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

    fn row_to_proposal(row: &rusqlite::Row) -> rusqlite::Result<ReviewProposal> {
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
    }

    pub fn list_pending_proposals(&self, project_id: Option<&str>) -> Result<Vec<ReviewProposal>> {
        let conn = self.conn();
        let mut sql = "SELECT id, project_id, proposal_type, target_id, description, suggested_action, status, created_at, decided_at FROM review_proposals WHERE status = 'pending'".to_string();
        if let Some(pid) = project_id {
            sql.push_str(&format!(" AND (project_id IS NULL OR project_id = '{}')", pid));
        }
        sql.push_str(" ORDER BY created_at DESC");

        let mut stmt = conn.prepare(&sql)?;
        let rows = stmt.query_map([], |row| Self::row_to_proposal(row))?;

        let mut list = Vec::new();
        for p in rows {
            list.push(p?);
        }
        Ok(list)
    }

    pub fn get_proposal_by_id(&self, id: &str) -> Result<Option<ReviewProposal>> {
        let conn = self.conn();
        let sql = "SELECT id, project_id, proposal_type, target_id, description, suggested_action, status, created_at, decided_at FROM review_proposals WHERE id = ?1";
        let mut stmt = conn.prepare(sql)?;
        let mut rows = stmt.query([id])?;
        if let Some(row) = rows.next()? {
            Ok(Some(Self::row_to_proposal(row)?))
        } else {
            Ok(None)
        }
    }

    pub fn find_pending_proposal(
        &self,
        project_id: Option<&str>,
        target_id: &str,
        description: &str,
    ) -> Result<Option<ReviewProposal>> {
        let conn = self.conn();
        let mut sql = "SELECT id, project_id, proposal_type, target_id, description, suggested_action, status, created_at, decided_at FROM review_proposals WHERE status = 'pending' AND target_id = ?1 AND description = ?2".to_string();
        if let Some(pid) = project_id {
            sql.push_str(&format!(" AND (project_id IS NULL OR project_id = '{}')", pid));
        }
        sql.push_str(" ORDER BY created_at DESC LIMIT 1");

        let mut stmt = conn.prepare(&sql)?;
        let mut rows = stmt.query([target_id, description])?;
        if let Some(row) = rows.next()? {
            Ok(Some(Self::row_to_proposal(row)?))
        } else {
            Ok(None)
        }
    }

    pub fn find_approved_proposal(
        &self,
        project_id: Option<&str>,
        target_id: &str,
        description: &str,
    ) -> Result<Option<ReviewProposal>> {
        let conn = self.conn();
        let mut sql = "SELECT id, project_id, proposal_type, target_id, description, suggested_action, status, created_at, decided_at FROM review_proposals WHERE status = 'approved' AND target_id = ?1 AND description = ?2".to_string();
        if let Some(pid) = project_id {
            sql.push_str(&format!(" AND (project_id IS NULL OR project_id = '{}')", pid));
        }
        sql.push_str(" ORDER BY decided_at DESC LIMIT 1");

        let mut stmt = conn.prepare(&sql)?;
        let mut rows = stmt.query([target_id, description])?;
        if let Some(row) = rows.next()? {
            Ok(Some(Self::row_to_proposal(row)?))
        } else {
            Ok(None)
        }
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
