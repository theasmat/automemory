use crate::db::Database;
use crate::models::{
    Decision, DecisionQueryResult, DecisionRecordArgs, DecisionStatus, Memory, MemoryScope,
    MemorySourceKind, MemoryStatus, MemoryType, TrustTier,
};
use anyhow::{Context, Result};
use chrono::{DateTime, Utc};
use rusqlite::params;
use std::str::FromStr;
use uuid::Uuid;

impl Database {
    /// Records an architectural decision. If a prior active decision exists for the same topic,
    /// it is marked as 'superseded' with a pointer to the new decision ID.
    pub fn record_decision(&self, args: DecisionRecordArgs<'_>) -> Result<Decision> {
        let now = Utc::now();
        let now_str = now.to_rfc3339();
        let id = format!("dec-{}", Uuid::new_v4().simple());
        let conn = self.conn();

        // 1. Check for prior active decision for this topic
        let prior_id: Option<String> = conn
            .query_row(
                "SELECT id FROM decisions
                 WHERE (project_id IS ?1 OR project_id IS NULL OR ?1 IS NULL)
                   AND LOWER(TRIM(topic)) = LOWER(TRIM(?2))
                   AND status = 'active'
                 LIMIT 1",
                params![args.project_id, args.topic],
                |r| r.get(0),
            )
            .ok();

        // 2. If prior decision exists, mark it as superseded
        if let Some(ref pid) = prior_id {
            conn.execute(
                "UPDATE decisions
                 SET status = 'superseded',
                     superseded_by = ?1,
                     updated_at = ?2
                 WHERE id = ?3",
                params![id, now_str, pid],
            )
            .context("Failed to supersede prior decision")?;
        }

        // 3. Insert new decision
        let alternatives_json = serde_json::to_string(args.alternatives)?;
        let symbols_json = serde_json::to_string(args.symbols)?;
        let evidence_json = serde_json::to_string(args.evidence)?;

        conn.execute(
            "INSERT INTO decisions (
                id, project_id, topic, decision, reason, alternatives, dissent, agent,
                symbols, evidence, status, superseded_by, created_at, updated_at
             ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, 'active', NULL, ?11, ?11)",
            params![
                id,
                args.project_id,
                args.topic.trim(),
                args.decision,
                args.reason,
                alternatives_json,
                args.dissent,
                args.agent,
                symbols_json,
                evidence_json,
                now_str,
            ],
        )
        .context("Failed to insert decision")?;
        drop(conn);

        let decision = Decision {
            id: id.clone(),
            project_id: args.project_id.map(ToString::to_string),
            topic: args.topic.trim().to_string(),
            decision: args.decision.to_string(),
            reason: args.reason.to_string(),
            alternatives: args.alternatives.to_vec(),
            dissent: args.dissent.map(ToString::to_string),
            agent: args.agent.to_string(),
            symbols: args.symbols.to_vec(),
            evidence: args.evidence.to_vec(),
            status: DecisionStatus::Active,
            superseded_by: None,
            created_at: now,
            updated_at: now,
        };

        // 4. Synchronize with canonical memory engine as Tier 2 Verified fact
        let memory_summary = format!("Decision ({}): {}", args.topic, args.decision);
        let mut memory_content = format!(
            "### Architectural Decision: {}\n\n**Decision**: {}\n\n**Rationale**: {}\n",
            args.topic, args.decision, args.reason
        );
        if let Some(dissent_text) = args.dissent {
            memory_content.push_str(&format!("\n**Preserved Dissent**: {}\n", dissent_text));
        }

        let mut mem = Memory {
            id: format!("mem-{}", Uuid::new_v4().simple()),
            scope: if args.project_id.is_some() {
                MemoryScope::Project
            } else {
                MemoryScope::Global
            },
            project_id: args.project_id.map(ToString::to_string),
            memory_type: MemoryType::Decision,
            content: memory_content,
            summary: memory_summary,
            confidence: 0.98,
            importance: 0.95,
            freshness: 1.0,
            status: MemoryStatus::Active,
            source_kind: MemorySourceKind::Agent,
            version: Some(1),
            created_at: now,
            updated_at: now,
            last_recalled_at: None,
            tags: vec!["decision".to_string(), args.topic.to_string()],
            entities: args.symbols.to_vec(),
            metadata: serde_json::json!({
                "decision_id": id,
                "trust_tier": TrustTier::Verified.as_u8(),
                "agent": args.agent,
                "alternatives": args.alternatives,
            }),
        };
        mem.set_trust_tier(TrustTier::Verified);
        let _ = self.save_memory(&mem);

        Ok(decision)
    }

    /// Fetches the active decision for a topic along with full historical decisions and dissent.
    pub fn get_decision(
        &self,
        project_id: Option<&str>,
        topic: &str,
    ) -> Result<DecisionQueryResult> {
        let conn = self.conn();
        let clean_topic = topic.trim();

        // 1. Fetch active decision
        let mut active_stmt = conn.prepare(
            "SELECT id, project_id, topic, decision, reason, alternatives, dissent, agent,
                    symbols, evidence, status, superseded_by, created_at, updated_at
             FROM decisions
             WHERE (project_id IS ?1 OR project_id IS NULL OR ?1 IS NULL)
               AND LOWER(TRIM(topic)) = LOWER(TRIM(?2))
               AND status = 'active'
             ORDER BY created_at DESC
             LIMIT 1",
        )?;

        let active: Option<Decision> = active_stmt
            .query_row(params![project_id, clean_topic], Self::row_to_decision)
            .ok();

        // 2. Fetch history (superseded or deprecated decisions for topic)
        let mut hist_stmt = conn.prepare(
            "SELECT id, project_id, topic, decision, reason, alternatives, dissent, agent,
                    symbols, evidence, status, superseded_by, created_at, updated_at
             FROM decisions
             WHERE (project_id IS ?1 OR project_id IS NULL OR ?1 IS NULL)
               AND LOWER(TRIM(topic)) = LOWER(TRIM(?2))
               AND status != 'active'
             ORDER BY created_at DESC",
        )?;

        let history = hist_stmt
            .query_map(params![project_id, clean_topic], Self::row_to_decision)?
            .collect::<std::result::Result<Vec<_>, _>>()?;

        Ok(DecisionQueryResult {
            topic: clean_topic.to_string(),
            active,
            history,
        })
    }

    /// Lists decisions filtered by project and optional status.
    pub fn list_decisions(
        &self,
        project_id: Option<&str>,
        status: Option<DecisionStatus>,
        limit: usize,
    ) -> Result<Vec<Decision>> {
        let conn = self.conn();
        let status_str = status.map(|s| s.to_string());

        let mut stmt = conn.prepare(
            "SELECT id, project_id, topic, decision, reason, alternatives, dissent, agent,
                    symbols, evidence, status, superseded_by, created_at, updated_at
             FROM decisions
             WHERE (?1 IS NULL OR project_id = ?1)
               AND (?2 IS NULL OR status = ?2)
             ORDER BY updated_at DESC
             LIMIT ?3",
        )?;

        let decisions = stmt
            .query_map(
                params![project_id, status_str, limit as i64],
                Self::row_to_decision,
            )?
            .collect::<std::result::Result<Vec<_>, _>>()?;

        Ok(decisions)
    }

    fn row_to_decision(row: &rusqlite::Row) -> rusqlite::Result<Decision> {
        let alts_str: String = row.get(5)?;
        let syms_str: String = row.get(8)?;
        let evid_str: String = row.get(9)?;
        let status_str: String = row.get(10)?;
        let created_str: String = row.get(12)?;
        let updated_str: String = row.get(13)?;

        Ok(Decision {
            id: row.get(0)?,
            project_id: row.get(1)?,
            topic: row.get(2)?,
            decision: row.get(3)?,
            reason: row.get(4)?,
            alternatives: serde_json::from_str(&alts_str).unwrap_or_default(),
            dissent: row.get(6)?,
            agent: row.get(7)?,
            symbols: serde_json::from_str(&syms_str).unwrap_or_default(),
            evidence: serde_json::from_str(&evid_str).unwrap_or_default(),
            status: DecisionStatus::from_str(&status_str).unwrap_or(DecisionStatus::Active),
            superseded_by: row.get(11)?,
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

    #[test]
    fn test_record_and_supersede_decision() {
        let db = Database::open_in_memory().expect("in-memory db open");

        // Record first decision
        let d1 = db
            .record_decision(DecisionRecordArgs {
                project_id: Some("proj-test"),
                topic: "authentication",
                decision: "Use HTTP-only JWT cookies",
                reason: "Mitigates XSS attack vector",
                alternatives: &["Server-side sessions".to_string()],
                dissent: Some("Cookie size overhead with complex claims"),
                agent: "claude-code",
                symbols: &["auth::jwt".to_string()],
                evidence: &["SEC-ADR-01".to_string()],
            })
            .expect("record d1");

        assert_eq!(d1.status, DecisionStatus::Active);
        assert_eq!(d1.topic, "authentication");
        assert!(d1.dissent.is_some());

        // Query active decision
        let query1 = db
            .get_decision(Some("proj-test"), "authentication")
            .expect("get_decision");
        assert!(query1.active.is_some());
        assert_eq!(query1.active.unwrap().id, d1.id);
        assert_eq!(query1.history.len(), 0);

        // Record second decision on same topic (superseding d1)
        let d2 = db
            .record_decision(DecisionRecordArgs {
                project_id: Some("proj-test"),
                topic: "authentication",
                decision: "Use JWT in HTTP-only cookies with refresh token rotation",
                reason: "Adds rotation to limit blast radius of stolen tokens",
                alternatives: &["OAuth2 PKCE".to_string()],
                dissent: Some("Increased DB writes for token rotation tracking"),
                agent: "antigravity",
                symbols: &["auth::jwt".to_string(), "auth::refresh".to_string()],
                evidence: &["SEC-ADR-02".to_string()],
            })
            .expect("record d2");

        assert_eq!(d2.status, DecisionStatus::Active);

        // Query decision again: active is d2, history contains d1
        let query2 = db
            .get_decision(Some("proj-test"), "authentication")
            .expect("get_decision 2");
        let active2 = query2.active.expect("active decision must exist");
        assert_eq!(active2.id, d2.id);
        assert_eq!(
            active2.decision,
            "Use JWT in HTTP-only cookies with refresh token rotation"
        );

        assert_eq!(query2.history.len(), 1);
        let prior = &query2.history[0];
        assert_eq!(prior.id, d1.id);
        assert_eq!(prior.status, DecisionStatus::Superseded);
        assert_eq!(prior.superseded_by.as_deref(), Some(d2.id.as_str()));
        assert_eq!(
            prior.dissent.as_deref(),
            Some("Cookie size overhead with complex claims")
        );
    }
}
