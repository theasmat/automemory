use crate::models::Memory;
use std::collections::HashMap;

pub const DEFAULT_RRF_K: f64 = 60.0;
pub const DEFAULT_ALPHA_FTS: f64 = 0.5;
pub const DEFAULT_BETA_VEC: f64 = 0.5;

#[derive(Debug, Clone)]
pub struct ScoredCandidate {
    pub memory: Memory,
    pub fts_rank: Option<usize>,
    pub vec_similarity: Option<f32>,
    pub rrf_score: f64,
    pub final_score: f64,
}

pub struct HybridScorer {
    k: f64,
    alpha: f64,
    beta: f64,
}

impl Default for HybridScorer {
    fn default() -> Self {
        Self {
            k: DEFAULT_RRF_K,
            alpha: DEFAULT_ALPHA_FTS,
            beta: DEFAULT_BETA_VEC,
        }
    }
}

impl HybridScorer {
    pub fn new(k: f64, alpha: f64, beta: f64) -> Self {
        Self { k, alpha, beta }
    }

    /// Fuses FTS lexical search results and Vector semantic search results via Reciprocal Rank Fusion (RRF).
    pub fn fuse_and_rank(
        &self,
        fts_results: Vec<(Memory, f64)>,
        vec_results: Vec<(Memory, f32)>,
        limit: usize,
    ) -> Vec<ScoredCandidate> {
        let mut candidate_map: HashMap<String, (Memory, Option<usize>, Option<f32>)> = HashMap::new();

        // 1. Ingest FTS results (0-indexed rank)
        for (rank, (mem, _score)) in fts_results.into_iter().enumerate() {
            let entry = candidate_map.entry(mem.id.clone()).or_insert_with(|| (mem.clone(), None, None));
            entry.1 = Some(rank);
        }

        // 2. Ingest Vector results
        for (mem, sim) in vec_results {
            let entry = candidate_map.entry(mem.id.clone()).or_insert_with(|| (mem.clone(), None, None));
            entry.2 = Some(sim);
        }

        // Sort vector candidates to compute vector rank
        let mut vec_ranked: Vec<(String, f32)> = candidate_map
            .iter()
            .filter_map(|(id, (_, _, sim))| sim.map(|s| (id.clone(), s)))
            .collect();
        vec_ranked.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));

        let vec_rank_map: HashMap<String, usize> = vec_ranked
            .into_iter()
            .enumerate()
            .map(|(rank, (id, _))| (id, rank))
            .collect();

        // 3. Compute RRF Score and confidence-weighted final score
        let mut candidates: Vec<ScoredCandidate> = candidate_map
            .into_iter()
            .map(|(id, (memory, fts_rank, vec_sim))| {
                let mut rrf = 0.0;

                if let Some(rank) = fts_rank {
                    rrf += self.alpha / (self.k + (rank as f64) + 1.0);
                }

                if let Some(&v_rank) = vec_rank_map.get(&id) {
                    rrf += self.beta / (self.k + (v_rank as f64) + 1.0);
                }

                // Final score weights RRF by memory confidence and importance
                let confidence_factor = memory.confidence.clamp(0.1, 1.0);
                let importance_factor = memory.importance.clamp(0.1, 1.0);
                let final_score = rrf * (0.6 * confidence_factor + 0.4 * importance_factor);

                ScoredCandidate {
                    memory,
                    fts_rank,
                    vec_similarity: vec_sim,
                    rrf_score: rrf,
                    final_score,
                }
            })
            .collect();

        // Sort descending by final score
        candidates.sort_by(|a, b| {
            b.final_score
                .partial_cmp(&a.final_score)
                .unwrap_or(std::cmp::Ordering::Equal)
        });

        candidates.truncate(limit);
        candidates
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::{MemoryScope, MemorySourceKind, MemoryStatus, MemoryType};
    use chrono::Utc;

    fn make_test_memory(id: &str, summary: &str, confidence: f64) -> Memory {
        Memory {
            id: id.to_string(),
            scope: MemoryScope::Project,
            project_id: Some("proj-1".to_string()),
            memory_type: MemoryType::Fact,
            content: summary.to_string(),
            summary: summary.to_string(),
            confidence,
            importance: 0.9,
            freshness: 1.0,
            status: MemoryStatus::Active,
            source_kind: MemorySourceKind::Agent,
            version: Some(1),
            created_at: Utc::now(),
            updated_at: Utc::now(),
            last_recalled_at: None,
            tags: vec![],
            entities: vec![],
            metadata: serde_json::json!({}),
        }
    }

    #[test]
    fn test_rrf_fusion() {
        let scorer = HybridScorer::default();
        let m1 = make_test_memory("m1", "Rust memory safety", 0.95);
        let m2 = make_test_memory("m2", "SQLite WAL mode", 0.8);
        let m3 = make_test_memory("m3", "TypeScript React build", 0.7);

        let fts_results = vec![(m1.clone(), -1.5), (m2.clone(), -2.0)];
        let vec_results = vec![(m1.clone(), 0.88), (m3.clone(), 0.75)];

        let ranked = scorer.fuse_and_rank(fts_results, vec_results, 5);

        assert!(!ranked.is_empty());
        assert_eq!(ranked[0].memory.id, "m1", "m1 is in both FTS and Vec so must rank first");
        assert!(ranked[0].final_score > 0.0);
    }
}
