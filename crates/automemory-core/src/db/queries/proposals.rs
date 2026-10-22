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

    /// Records a person's decision on one proposal and does what that kind of proposal means. Every way of deciding (one, many, CLI, MCP, dashboard) comes through here.
    pub fn decide_proposal(&self, id: &str, approved: bool) -> Result<bool> {
        Ok(self.decide_proposal_counted(id, approved)?.0)
    }

    /// (changed, memories archived by this decision)
    fn decide_proposal_counted(&self, id: &str, approved: bool) -> Result<(bool, usize)> {
        let proposal = self.get_proposal_by_id(id)?;
        let status = if approved { "approved" } else { "rejected" };
        let rows = self.conn().execute(
            "UPDATE review_proposals SET status = ?1, decided_at = ?2 WHERE id = ?3",
            params![status, Utc::now().to_rfc3339(), id],
        )?;
        let mut archived = 0;
        if rows > 0 {
            if let Some(p) = &proposal {
                archived = self.apply_decision(p, approved)?;
            }
        }
        Ok((rows > 0, archived))
    }

    /// The effect of a decision, by proposal type. Returns how many memories it archived.
    fn apply_decision(&self, p: &crate::models::ReviewProposal, approved: bool) -> Result<usize> {
        match p.proposal_type.as_str() {
            // An agent's suggested edit to a library file: applied only when a person approves it.
            "library_edit" => {
                if approved { self.library_apply_proposal(&p.id)?; } else { self.library_drop_proposal(&p.id); }
            }
            // A note an LLM suggested: approval makes it a user-confirmed fact, rejection shelves it.
            "llm_candidate" => {
                if let Ok(Some(mut m)) = self.get_memory_by_id(&p.target_id) {
                    if approved {
                        m.status = crate::models::MemoryStatus::Active;
                        m.set_trust_tier(crate::models::TrustTier::UserStatement);
                    } else {
                        m.status = crate::models::MemoryStatus::Archived;
                    }
                    m.updated_at = Utc::now();
                    self.update_memory(&m, Some("LLM suggestion decided by a human"), &[])?;
                }
            }
            // A teammate's pin: approving puts back their exact record, identical to theirs, so nothing is sent back or conflicts.
            "team_pin" if approved => {
                if let Some(mem) = self.sync_kv_get(&format!("teampin:{}", p.target_id)).filter(|j| !j.is_empty()).and_then(|j| serde_json::from_str::<crate::models::Memory>(&j).ok()) {
                    self.save_memory_direct(&mem)?;
                    self.sync_kv_set(&format!("clamped:{}", p.target_id), "");
                }
            }
            // An unused note offered for archiving (retrieval::fade): yes archives it, no keeps it and counts as a use so it is not offered or faded again for a while.
            "stale_note" => {
                let now = Utc::now().to_rfc3339();
                let n = if approved {
                    self.conn().execute("UPDATE memories SET status = 'archived', updated_at = ?1 WHERE id = ?2", params![now, p.target_id])?
                } else {
                    self.conn().execute("UPDATE memories SET last_recalled_at = ?1 WHERE id = ?2", params![now, p.target_id])?;
                    0
                };
                return Ok(n);
            }
            "duplicate_merge" if approved && !p.target_id.is_empty() => {
                let n = self.conn().execute("UPDATE memories SET status = 'archived', updated_at = ?1 WHERE id = ?2", params![Utc::now().to_rfc3339(), p.target_id])?;
                return Ok(n);
            }
            _ => {}
        }
        Ok(0)
    }

    /// Decides every pending proposal that matches (optionally one project and one type), each through the same path as a single decision.
    /// Suggested library edits are never approved in bulk: they change a document agents trust, so a person reads each one.
    pub fn decide_all_proposals(
        &self,
        project_id: Option<&str>,
        proposal_type: Option<&str>,
        approved: bool,
    ) -> Result<crate::models::ProposalBatchResult> {
        let ids: Vec<String> = self
            .list_pending_proposals(project_id)?
            .into_iter()
            .filter(|p| proposal_type.is_none_or(|t| t == "all" || p.proposal_type == t))
            .filter(|p| !(approved && p.proposal_type == "library_edit"))
            .map(|p| p.id)
            .collect();
        let (mut decided, mut archived) = (0, 0);
        for id in ids {
            let (changed, n) = self.decide_proposal_counted(&id, approved)?;
            decided += changed as usize;
            archived += n;
        }
        Ok(crate::models::ProposalBatchResult {
            decided_count: decided,
            approved_count: if approved { decided } else { 0 },
            rejected_count: if !approved { decided } else { 0 },
            archived_memories_count: archived,
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

#[cfg(test)]
mod decision_tests {
    use super::*;
    use crate::models::*;

    fn candidate(db: &Database, id: &str) {
        let mut m = Memory {
            id: id.into(), scope: MemoryScope::Global, project_id: None, memory_type: MemoryType::Fact, content: "c".into(), summary: "s".into(),
            confidence: 0.5, importance: 0.5, freshness: 1.0, status: MemoryStatus::Candidate, source_kind: MemorySourceKind::Agent, version: Some(1),
            created_at: Utc::now(), updated_at: Utc::now(), last_recalled_at: None, tags: vec![], entities: vec![], metadata: serde_json::json!({}),
        };
        m.set_trust_tier(TrustTier::AgentClaim);
        db.save_memory(&m).unwrap();
    }

    fn proposal(db: &Database, id: &str, kind: &str, target: &str) {
        db.save_proposal(&ReviewProposal {
            id: id.into(), project_id: None, proposal_type: kind.into(), target_id: target.into(), description: "d".into(), suggested_action: "a".into(),
            status: "pending".into(), created_at: Utc::now(), decided_at: None,
        }).unwrap();
    }

    #[test]
    fn approving_many_does_what_approving_one_does() {
        let db = Database::open_in_memory().unwrap();
        candidate(&db, "m-llm");
        candidate(&db, "m-dup");
        proposal(&db, "p-llm", "llm_candidate", "m-llm");
        proposal(&db, "p-dup", "duplicate_merge", "m-dup");
        let r = db.decide_all_proposals(None, None, true).unwrap();
        assert_eq!((r.decided_count, r.approved_count, r.archived_memories_count), (2, 2, 1), "{r:?}");
        let llm = db.get_memory_by_id("m-llm").unwrap().unwrap();
        assert_eq!(llm.status, MemoryStatus::Active, "approve-all used to leave an LLM suggestion as a candidate");
        assert_eq!(db.get_memory_by_id("m-dup").unwrap().unwrap().status, MemoryStatus::Archived);
        assert!(db.list_pending_proposals(None).unwrap().is_empty());
    }

    #[test]
    fn rejecting_many_shelves_llm_suggestions() {
        let db = Database::open_in_memory().unwrap();
        candidate(&db, "m-llm");
        proposal(&db, "p-llm", "llm_candidate", "m-llm");
        proposal(&db, "p-other", "sync_conflict", "memory:x");
        let r = db.decide_all_proposals(None, Some("llm_candidate"), false).unwrap();
        assert_eq!((r.decided_count, r.rejected_count), (1, 1));
        assert_eq!(db.get_memory_by_id("m-llm").unwrap().unwrap().status, MemoryStatus::Archived);
        assert_eq!(db.list_pending_proposals(None).unwrap().len(), 1, "only the chosen type was decided");
    }
}
