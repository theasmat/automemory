use anyhow::Result;
use chrono::Utc;
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use tracing::info;
use uuid::Uuid;

use crate::db::Database;
use crate::models::{Memory, MemoryStatus, MemoryType, ReviewProposal};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConsolidationReport {
    pub scanned_memories: usize,
    pub duplicates_merged: usize,
    pub contradictions_resolved: usize,
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
        let memories = self
            .db
            .list_memories(project_id, Some(MemoryStatus::Active), None, 1000)?;
        let total = memories.len();

        let mut duplicates_merged = 0;
        let mut contradictions_resolved = 0;
        let mut proposals_created = 0;
        let mut archived_stale = 0;

        let mut existing_targets: std::collections::HashSet<String> = self
            .db
            .list_pending_proposals(project_id)
            .unwrap_or_default()
            .into_iter()
            .map(|p| p.target_id)
            .collect();

        for i in 0..memories.len() {
            for j in (i + 1)..memories.len() {
                let m1 = &memories[i];
                let m2 = &memories[j];

                // 1. Identical or near-identical summary check (Duplicate merge)
                let s1 = m1.summary.trim().to_lowercase();
                let s2 = m2.summary.trim().to_lowercase();

                if s1 == s2 && !s1.is_empty() {
                    let (winner, loser) = if m1.confidence >= m2.confidence {
                        (m1, m2)
                    } else {
                        (m2, m1)
                    };

                    if !existing_targets.contains(&loser.id) {
                        existing_targets.insert(loser.id.clone());
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
                    }
                    duplicates_merged += 1;
                    continue;
                }

                // 2. Semantic & Architectural Contradiction Check
                if let Some(reason) = detect_heuristic_contradiction(m1, m2) {
                    let (newer, older) = if m1.updated_at >= m2.updated_at {
                        (m1, m2)
                    } else {
                        (m2, m1)
                    };

                    if !existing_targets.contains(&older.id) {
                        existing_targets.insert(older.id.clone());
                        let prop = ReviewProposal {
                            id: Uuid::now_v7().to_string(),
                            project_id: project_id.map(String::from),
                            proposal_type: "contradiction_resolution".to_string(),
                            target_id: older.id.clone(),
                            description: format!(
                                "Architectural contradiction detected:\nActive (newer): {} ({})\nActive (older): {} ({})\nReason: {}",
                                newer.summary, newer.id, older.summary, older.id, reason
                            ),
                            suggested_action: format!("archive {}", older.id),
                            status: "pending".to_string(),
                            created_at: Utc::now(),
                            decided_at: None,
                        };

                        let _ = self.db.save_proposal(&prop);

                        // Penalize the older memory's confidence by 0.25x
                        let mut penalized = older.clone();
                        penalized.confidence = (penalized.confidence * 0.25).clamp(0.05, 1.0);
                        let _ = self.db.update_memory(
                            &penalized,
                            Some("Contradiction penalty applied"),
                            &[],
                        );

                        proposals_created += 1;
                    }
                    contradictions_resolved += 1;
                }
            }

            // 3. Freshness decay check for very old low-confidence memories
            let mem = &memories[i];
            let age_days = (Utc::now() - mem.updated_at).num_days();
            if age_days > 90 && mem.confidence < 0.4 {
                let _ = self.db.archive_memory(&mem.id);
                archived_stale += 1;
            }
        }

        info!(
            "Consolidation pass completed for {:?}: scanned {}, merged {}, contradictions {}, proposals {}, archived {}",
            project_id, total, duplicates_merged, contradictions_resolved, proposals_created, archived_stale
        );

        Ok(ConsolidationReport {
            scanned_memories: total,
            duplicates_merged,
            contradictions_resolved,
            proposals_created,
            archived_stale,
        })
    }
}

/// Deterministic architectural and semantic contradiction detector.
pub fn detect_heuristic_contradiction(m1: &Memory, m2: &Memory) -> Option<String> {
    let t1 = format!("{} {}", m1.summary, m1.content).to_lowercase();
    let t2 = format!("{} {}", m2.summary, m2.content).to_lowercase();

    // 1. Direct migration / replacement phrases
    let replacement_cues = [
        "instead of",
        "deprecated in favor of",
        "migrated from",
        "migrating from",
        "replaced with",
        "replacing",
        "switched from",
        "switching from",
        "do not use",
        "don't use",
        "deprecated",
    ];

    for cue in &replacement_cues {
        if t1.contains(cue) || t2.contains(cue) {
            let mut shared_terms = Vec::new();
            for tag in &m1.tags {
                let tag_lower = tag.to_lowercase();
                if t2.contains(&tag_lower) && tag_lower.len() > 2 {
                    shared_terms.push(tag.clone());
                }
            }
            for entity in &m1.entities {
                let ent_lower = entity.to_lowercase();
                if t2.contains(&ent_lower) && ent_lower.len() > 2 && !shared_terms.contains(entity) {
                    shared_terms.push(entity.clone());
                }
            }
            for tag in &m2.tags {
                let tag_lower = tag.to_lowercase();
                if t1.contains(&tag_lower) && tag_lower.len() > 2 && !shared_terms.contains(tag) {
                    shared_terms.push(tag.clone());
                }
            }
            for entity in &m2.entities {
                let ent_lower = entity.to_lowercase();
                if t1.contains(&ent_lower) && ent_lower.len() > 2 && !shared_terms.contains(entity) {
                    shared_terms.push(entity.clone());
                }
            }
            if !shared_terms.is_empty() {
                return Some(format!(
                    "Replacement or deprecation cue '{}' found with overlapping terms: {:?}",
                    cue, shared_terms
                ));
            }
        }
    }

    // 2. Mutually exclusive software categories
    let exclusive_groups: &[&[&str]] = &[
        // Frontend frameworks
        &["react", "vue", "svelte", "angular", "solidjs"],
        // Vue major versions
        &["vue 2", "vue 3", "vue2", "vue3"],
        // State management
        &["redux", "zustand", "pinia", "vuex", "mobx"],
        // JavaScript runtimes
        &["node.js", "deno", "bun"],
        // Build tools
        &["vite", "webpack", "turbopack", "parcel", "rollup"],
        // Databases
        &["postgresql", "mysql", "mongodb", "sqlite", "dynamodb"],
        // ORMs
        &["prisma", "drizzle", "typeorm", "diesel", "sea-orm"],
        // Package managers
        &["pnpm", "npm", "yarn", "bun"],
    ];

    for group in exclusive_groups {
        let mut m1_matches = Vec::new();
        let mut m2_matches = Vec::new();

        for &tech in *group {
            if t1.contains(tech) {
                m1_matches.push(tech);
            }
            if t2.contains(tech) {
                m2_matches.push(tech);
            }
        }

        if !m1_matches.is_empty() && !m2_matches.is_empty() {
            let conflict = m1_matches.iter().any(|a| !m2_matches.contains(a))
                && m2_matches.iter().any(|b| !m1_matches.contains(b));
            if conflict {
                let share_domain = m1.tags.iter().any(|t| m2.tags.contains(t))
                    || (m1.memory_type == MemoryType::Decision
                        && m2.memory_type == MemoryType::Decision);
                if share_domain {
                    return Some(format!(
                        "Mutually exclusive technology choices in domain {:?}: {:?} vs {:?}",
                        group[0], m1_matches, m2_matches
                    ));
                }
            }
        }
    }

    // 3. Opposing directives: "always use X" vs "never use X" / "avoid X"
    for tag in &m1.tags {
        let t_clean = tag.to_lowercase();
        if t_clean.len() < 3 {
            continue;
        }
        let pos_m1 =
            t1.contains(&format!("use {}", t_clean)) || t1.contains(&format!("always {}", t_clean));
        let neg_m1 = t1.contains(&format!("avoid {}", t_clean))
            || t1.contains(&format!("never {}", t_clean))
            || t1.contains(&format!("do not use {}", t_clean));

        let pos_m2 =
            t2.contains(&format!("use {}", t_clean)) || t2.contains(&format!("always {}", t_clean));
        let neg_m2 = t2.contains(&format!("avoid {}", t_clean))
            || t2.contains(&format!("never {}", t_clean))
            || t2.contains(&format!("do not use {}", t_clean));

        if (pos_m1 && neg_m2) || (neg_m1 && pos_m2) {
            return Some(format!("Direct directive conflict regarding '{}'", tag));
        }
    }

    None
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::{MemoryScope, MemorySourceKind};
    use chrono::Duration;

    #[test]
    fn test_contradiction_detection_and_penalty() {
        let db = Arc::new(Database::open_in_memory().unwrap());
        let worker = ConsolidationWorker::new(db.clone());

        // Memory 1: Older decision to use Vue 2
        let m1 = Memory {
            id: "mem_vue2".to_string(),
            scope: MemoryScope::Project,
            project_id: Some("proj_web".to_string()),
            memory_type: MemoryType::Decision,
            content: "We use Vue 2 for frontend components.".to_string(),
            summary: "Frontend Framework: Vue 2".to_string(),
            confidence: 0.90,
            importance: 0.8,
            freshness: 0.6,
            status: MemoryStatus::Active,
            source_kind: MemorySourceKind::TaskReflection,
            version: Some(1),
            created_at: Utc::now() - Duration::days(10),
            updated_at: Utc::now() - Duration::days(10),
            last_recalled_at: None,
            tags: vec!["frontend".to_string(), "vue".to_string()],
            entities: vec!["Vue".to_string()],
            metadata: serde_json::json!({}),
        };

        // Memory 2: Newer decision migrating to Vue 3
        let m2 = Memory {
            id: "mem_vue3".to_string(),
            scope: MemoryScope::Project,
            project_id: Some("proj_web".to_string()),
            memory_type: MemoryType::Decision,
            content: "Migrated from Vue 2 to Vue 3 for composition API and TypeScript support."
                .to_string(),
            summary: "Frontend Framework: Migrated to Vue 3 instead of Vue 2".to_string(),
            confidence: 0.95,
            importance: 0.9,
            freshness: 1.0,
            status: MemoryStatus::Active,
            source_kind: MemorySourceKind::TaskReflection,
            version: Some(1),
            created_at: Utc::now(),
            updated_at: Utc::now(),
            last_recalled_at: None,
            tags: vec!["frontend".to_string(), "vue".to_string()],
            entities: vec!["Vue".to_string()],
            metadata: serde_json::json!({}),
        };

        db.save_memory(&m1).unwrap();
        db.save_memory(&m2).unwrap();

        let report = worker.consolidate(Some("proj_web")).unwrap();
        assert_eq!(report.contradictions_resolved, 1);
        assert_eq!(report.proposals_created, 1);

        // Verify older memory received confidence penalty (0.90 * 0.25 = 0.225)
        let older_updated = db.get_memory_by_id("mem_vue2").unwrap().unwrap();
        assert!((older_updated.confidence - 0.225).abs() < 0.01);

        // Verify proposal was created
        let proposals = db.list_pending_proposals(Some("proj_web")).unwrap();
        assert_eq!(proposals.len(), 1);
        assert_eq!(proposals[0].proposal_type, "contradiction_resolution");
        assert_eq!(proposals[0].target_id, "mem_vue2");
    }

    #[test]
    fn test_unrelated_memory_with_replacement_cue_does_not_conflict() {
        let m1 = Memory {
            id: "m_rustls".to_string(),
            scope: MemoryScope::Project,
            project_id: Some("proj_p".to_string()),
            memory_type: MemoryType::Decision,
            content: "Use rustls instead of openssl for networking.".to_string(),
            summary: "Networking: rustls instead of openssl".to_string(),
            confidence: 0.9,
            importance: 0.8,
            freshness: 0.8,
            status: MemoryStatus::Active,
            source_kind: MemorySourceKind::TaskReflection,
            version: Some(1),
            created_at: Utc::now(),
            updated_at: Utc::now(),
            last_recalled_at: None,
            tags: vec!["tls".to_string(), "rustls".to_string()],
            entities: vec!["rustls".to_string()],
            metadata: serde_json::json!({}),
        };
        let m2 = Memory {
            id: "m_db".to_string(),
            scope: MemoryScope::Project,
            project_id: Some("proj_p".to_string()),
            memory_type: MemoryType::Fact,
            content: "Database migrations are in sql/ migrations folder.".to_string(),
            summary: "Database migrations path".to_string(),
            confidence: 0.9,
            importance: 0.8,
            freshness: 0.8,
            status: MemoryStatus::Active,
            source_kind: MemorySourceKind::TaskReflection,
            version: Some(1),
            created_at: Utc::now() - Duration::days(5),
            updated_at: Utc::now() - Duration::days(5),
            last_recalled_at: None,
            tags: vec!["database".to_string(), "migrations".to_string()],
            entities: vec!["SQL".to_string()],
            metadata: serde_json::json!({}),
        };
        assert!(detect_heuristic_contradiction(&m1, &m2).is_none());
    }
}
