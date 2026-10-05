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
        self.list_proposals(project_id, Some("pending"), None, None, None)
    }

    pub fn list_proposals(
        &self,
        project_id: Option<&str>,
        status: Option<&str>,
        proposal_type: Option<&str>,
        limit: Option<usize>,
        offset: Option<usize>,
    ) -> Result<Vec<ReviewProposal>> {
        let conn = self.conn();
        let mut sql = "SELECT id, project_id, proposal_type, target_id, description, suggested_action, status, created_at, decided_at FROM review_proposals WHERE 1=1".to_string();
        if let Some(st) = status {
            if st != "all" {
                sql.push_str(&format!(" AND status = '{}'", st.replace('\'', "''")));
            }
        } else {
            sql.push_str(" AND status = 'pending'");
        }
        if let Some(pid) = project_id {
            sql.push_str(&format!(
                " AND (project_id IS NULL OR project_id = '{}')",
                pid.replace('\'', "''")
            ));
        }
        if let Some(pt) = proposal_type {
            if pt != "all" {
                sql.push_str(&format!(
                    " AND proposal_type = '{}'",
                    pt.replace('\'', "''")
                ));
            }
        }
        sql.push_str(" ORDER BY created_at DESC");
        if let Some(l) = limit {
            sql.push_str(&format!(" LIMIT {}", l));
            if let Some(o) = offset {
                sql.push_str(&format!(" OFFSET {}", o));
            }
        }

        let mut stmt = conn.prepare(&sql)?;
        let rows = stmt.query_map([], Self::row_to_proposal)?;

        let mut list = Vec::new();
        for p in rows {
            list.push(p?);
        }
        Ok(list)
    }

    pub fn get_proposals_summary(
        &self,
        project_id: Option<&str>,
    ) -> Result<crate::models::ProposalsSummary> {
        let conn = self.conn();
        let mut sql = "SELECT proposal_type, status, COUNT(*) FROM review_proposals".to_string();
        if let Some(pid) = project_id {
            sql.push_str(&format!(
                " WHERE (project_id IS NULL OR project_id = '{}')",
                pid.replace('\'', "''")
            ));
        }
        sql.push_str(" GROUP BY proposal_type, status");

        let mut stmt = conn.prepare(&sql)?;
        let mut rows = stmt.query([])?;

        let mut type_map: std::collections::HashMap<String, (usize, usize, usize)> =
            std::collections::HashMap::new();
        let mut total_pending = 0;
        let mut total_approved = 0;
        let mut total_rejected = 0;

        while let Some(row) = rows.next()? {
            let ptype: String = row.get(0)?;
            let status: String = row.get(1)?;
            let count: usize = row.get(2)?;

            let entry = type_map.entry(ptype).or_insert((0, 0, 0));
            match status.as_str() {
                "pending" => {
                    entry.0 += count;
                    total_pending += count;
                }
                "approved" => {
                    entry.1 += count;
                    total_approved += count;
                }
                "rejected" => {
                    entry.2 += count;
                    total_rejected += count;
                }
                _ => {}
            }
        }

        let mut by_type = Vec::new();
        for (ptype, (pend, app, rej)) in type_map {
            by_type.push(crate::models::ProposalSummaryItem {
                proposal_type: ptype,
                pending: pend,
                approved: app,
                rejected: rej,
                total: pend + app + rej,
            });
        }
        by_type.sort_by_key(|a| std::cmp::Reverse(a.total));

        Ok(crate::models::ProposalsSummary {
            total_pending,
            total_approved,
            total_rejected,
            total: total_pending + total_approved + total_rejected,
            by_type,
        })
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
            sql.push_str(&format!(
                " AND (project_id IS NULL OR project_id = '{}')",
                pid.replace('\'', "''")
            ));
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
            sql.push_str(&format!(
                " AND (project_id IS NULL OR project_id = '{}')",
                pid.replace('\'', "''")
            ));
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
        let proposal = self.get_proposal_by_id(id)?;
        let conn = self.conn();
        let status = if approved { "approved" } else { "rejected" };
        let now_str = Utc::now().to_rfc3339();

        let rows = conn.execute(
            "UPDATE review_proposals SET status = ?1, decided_at = ?2 WHERE id = ?3",
            params![status, now_str, id],
        )?;

        if approved && rows > 0 {
            if let Some(p) = proposal {
                if p.proposal_type == "duplicate_merge" && !p.target_id.is_empty() {
                    let _ = conn.execute(
                        "UPDATE memories SET status = 'archived', updated_at = ?1 WHERE id = ?2",
                        params![now_str, p.target_id],
                    );
                }
            }
        }

        Ok(rows > 0)
    }

    pub fn decide_all_proposals(
        &self,
        project_id: Option<&str>,
        proposal_type: Option<&str>,
        approved: bool,
    ) -> Result<crate::models::ProposalBatchResult> {
        let conn = self.conn();
        let status = if approved { "approved" } else { "rejected" };
        let now_str = Utc::now().to_rfc3339();

        let mut where_clause = "WHERE status = 'pending'".to_string();
        if let Some(pid) = project_id {
            where_clause.push_str(&format!(
                " AND (project_id IS NULL OR project_id = '{}')",
                pid.replace('\'', "''")
            ));
        }
        if let Some(pt) = proposal_type {
            if pt != "all" {
                where_clause.push_str(&format!(
                    " AND proposal_type = '{}'",
                    pt.replace('\'', "''")
                ));
            }
        }

        conn.execute("BEGIN TRANSACTION", [])?;

        let update_sql = format!(
            "UPDATE review_proposals SET status = ?1, decided_at = ?2 {}",
            where_clause
        );
        let decided_count = match conn.execute(&update_sql, params![status, now_str]) {
            Ok(c) => c,
            Err(e) => {
                let _ = conn.execute("ROLLBACK", []);
                return Err(e.into());
            }
        };

        let mut archived_memories_count = 0;
        if approved {
            let archive_sql = format!(
                r#"
                UPDATE memories
                   SET status = 'archived', updated_at = ?1
                 WHERE id IN (
                     SELECT DISTINCT target_id
                       FROM review_proposals
                      {}
                        AND decided_at = ?1
                        AND proposal_type = 'duplicate_merge'
                        AND target_id IS NOT NULL
                        AND target_id != ''
                 )
                "#,
                where_clause.replace("WHERE status = 'pending'", "WHERE status = 'approved'")
            );

            archived_memories_count = match conn.execute(&archive_sql, params![now_str]) {
                Ok(c) => c,
                Err(e) => {
                    let _ = conn.execute("ROLLBACK", []);
                    return Err(e.into());
                }
            };
        }

        conn.execute("COMMIT", [])?;

        Ok(crate::models::ProposalBatchResult {
            decided_count,
            approved_count: if approved { decided_count } else { 0 },
            rejected_count: if !approved { decided_count } else { 0 },
            archived_memories_count,
        })
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
