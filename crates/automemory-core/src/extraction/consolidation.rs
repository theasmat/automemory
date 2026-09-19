use anyhow::Result;
use chrono::Utc;
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use tracing::info;
use uuid::Uuid;

use crate::db::Database;
use crate::models::{MemoryStatus, ReviewProposal};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConsolidationReport {
    pub scanned_memories: usize,
    pub duplicates_merged: usize,
    pub proposals_created: usize,
    pub archived_stale: usize,
}

pub struct ConsolidationWorker {
    db: Arc<Database>,
}

impl ConsolidationWorker {
    pub fn new(db: Arc<Database>) -> Self {
        Self { db }
    }

    /// Run a consolidation pass over project or global memories.
    pub fn consolidate(&self, project_id: Option<&str>) -> Result<ConsolidationReport> {
        let memories = self.db.list_memories(project_id, Some(MemoryStatus::Active), None, 1000)?;
        let total = memories.len();

        let mut duplicates_merged = 0;
        let mut proposals_created = 0;
        let mut archived_stale = 0;

        for i in 0..memories.len() {
            for j in (i + 1)..memories.len() {
                let m1 = &memories[i];
                let m2 = &memories[j];

                // 1. Identical or near-identical summary check
                let s1 = m1.summary.trim().to_lowercase();
                let s2 = m2.summary.trim().to_lowercase();

                if s1 == s2 && !s1.is_empty() {
                    // Check if one has higher confidence
                    let (winner, loser) = if m1.confidence >= m2.confidence {
                        (m1, m2)
                    } else {
                        (m2, m1)
                    };

                    // Mark loser as superseded or create a proposal
                    let prop = ReviewProposal {
                        id: Uuid::now_v7().to_string(),
                        project_id: project_id.map(String::from),
                        proposal_type: "duplicate_merge".to_string(),
                        target_id: loser.id.clone(),
                        description: format!(
                            "Duplicate memory found:\nPrimary: {} ({})\nDuplicate: {} ({})",
                            winner.summary, winner.id, loser.summary, loser.id
                        ),
                        suggested_action: format!("archive {}", loser.id),
                        status: "pending".to_string(),
                        created_at: Utc::now(),
                        decided_at: None,
                    };

                    let _ = self.db.save_proposal(&prop);
                    proposals_created += 1;
                    duplicates_merged += 1;
                }
            }

            // 2. Freshness decay check for very old low-confidence memories
            let mem = &memories[i];
            let age_days = (Utc::now() - mem.updated_at).num_days();
            if age_days > 90 && mem.confidence < 0.4 {
                let _ = self.db.archive_memory(&mem.id);
                archived_stale += 1;
            }
        }

        info!(
            "Consolidation pass completed for {:?}: scanned {}, merged {}, proposals {}, archived {}",
            project_id, total, duplicates_merged, proposals_created, archived_stale
        );

        Ok(ConsolidationReport {
            scanned_memories: total,
            duplicates_merged,
            proposals_created,
            archived_stale,
        })
    }
}
