//! Recall quality on a labelled set: does a question find the note that answers it? (docs/roadmap.md, memory quality plan, item 1.)
//! The set is a JSON file of notes and queries (`tests/eval/recall/shop.json` is a synthetic starter); each query lists the note ids
//! that should rank high and a `kind` (paraphrase, identifier, ...) so a change can be judged per kind, not only on average.

use crate::db::Database;
use crate::models::{Memory, MemoryScope, MemorySourceKind, MemoryStatus, MemoryType};
use anyhow::Result;
use chrono::Utc;
use serde::Deserialize;
use std::collections::BTreeMap;

#[derive(Debug, Deserialize)]
pub struct RecallSet {
    pub notes: Vec<RecallNote>,
    pub queries: Vec<RecallQuery>,
}

#[derive(Debug, Deserialize)]
pub struct RecallNote {
    pub id: String,
    #[serde(rename = "type", default)]
    pub kind: String,
    pub summary: String,
    pub content: String,
    #[serde(default)]
    pub tags: Vec<String>,
}

#[derive(Debug, Deserialize)]
pub struct RecallQuery {
    pub query: String,
    pub expect: Vec<String>,
    #[serde(default)]
    pub kind: String,
}

#[derive(Debug, Default, Clone, PartialEq)]
pub struct RecallScore {
    pub queries: usize,
    pub recall_at_5: f64,
    pub ndcg_at_10: f64,
    pub mrr: f64,
}

#[derive(Debug, Default)]
pub struct RecallReport {
    /// Mean time of one recall call, queries only (not seeding).
    pub mean_query_ms: f64,
    pub overall: RecallScore,
    pub by_kind: BTreeMap<String, RecallScore>,
}

/// Fraction of the expected ids in the top 5, and nDCG@10 with binary relevance, for one ranking.
fn score_one(expect: &[String], ranked: &[String]) -> (f64, f64, f64) {
    let rel = |id: &String| expect.contains(id);
    let found = ranked.iter().take(5).filter(|id| rel(id)).count();
    let recall = found as f64 / expect.len().max(1) as f64;
    let dcg: f64 = ranked.iter().take(10).enumerate().filter(|(_, id)| rel(id)).map(|(i, _)| 1.0 / (i as f64 + 2.0).log2()).sum();
    let ideal: f64 = (0..expect.len().min(10)).map(|i| 1.0 / (i as f64 + 2.0).log2()).sum();
    let mrr = ranked.iter().position(rel).map_or(0.0, |p| 1.0 / (p as f64 + 1.0));
    (recall, if ideal > 0.0 { dcg / ideal } else { 0.0 }, mrr)
}

/// Seeds `set.notes` into a fresh in-memory database and runs every query through the same labelled recall `memory.recall` uses.
pub fn evaluate(set: &RecallSet) -> Result<RecallReport> {
    let db = Database::open_in_memory()?;
    let now = Utc::now();
    let mut ids = BTreeMap::new();
    for n in &set.notes {
        let mem = Memory {
            id: uuid::Uuid::now_v7().to_string(),
            scope: MemoryScope::Project,
            project_id: None,
            memory_type: match n.kind.as_str() {
                "decision" => MemoryType::Decision,
                "lesson" => MemoryType::Lesson,
                "warning" => MemoryType::Warning,
                _ => MemoryType::Fact,
            },
            content: n.content.clone(),
            summary: n.summary.clone(),
            confidence: 0.9,
            importance: 0.8,
            freshness: 1.0,
            status: MemoryStatus::Active,
            source_kind: MemorySourceKind::Agent,
            version: Some(1),
            created_at: now,
            updated_at: now,
            last_recalled_at: None,
            tags: n.tags.clone(),
            entities: vec![],
            metadata: serde_json::json!({}),
        };
        ids.insert(mem.id.clone(), n.id.clone());
        db.save_memory_direct(&mem)?;
    }
    let mut report = RecallReport::default();
    let add = |score: &mut RecallScore, (r, n, m): (f64, f64, f64)| {
        score.queries += 1;
        score.recall_at_5 += r;
        score.ndcg_at_10 += n;
        score.mrr += m;
    };
    let mut spent = std::time::Duration::ZERO;
    for q in &set.queries {
        let start = std::time::Instant::now();
        let found = db.search_memories_labelled(&q.query, None, 10)?;
        spent += start.elapsed();
        let ranked: Vec<String> =
            found.into_iter().map(|(m, _)| ids.get(&m.id).cloned().unwrap_or(m.id)).collect();
        let s = score_one(&q.expect, &ranked);
        if s.0 < 1.0 && std::env::var("RECALL_EVAL_MISSES").is_ok() {
            eprintln!("miss [{}] {:?} -> {:?} (want {:?})", q.kind, q.query, &ranked[..ranked.len().min(3)], q.expect);
        }
        add(&mut report.overall, s);
        add(report.by_kind.entry(q.kind.clone()).or_default(), s);
    }
    report.mean_query_ms = spent.as_secs_f64() * 1000.0 / set.queries.len().max(1) as f64;
    for s in std::iter::once(&mut report.overall).chain(report.by_kind.values_mut()) {
        let n = s.queries.max(1) as f64;
        s.recall_at_5 /= n;
        s.ndcg_at_10 /= n;
        s.mrr /= n;
    }
    Ok(report)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn scoring_counts_hits_and_ranks() {
        let e = vec!["a".to_string(), "b".to_string()];
        let (r, n, m) = score_one(&e, &["x".into(), "a".into(), "y".into(), "b".into()]);
        assert!((r - 1.0).abs() < 1e-9 && (m - 0.5).abs() < 1e-9 && n > 0.6 && n < 0.8, "{r} {n} {m}");
        assert_eq!(score_one(&e, &["x".into()]), (0.0, 0.0, 0.0));
    }

    /// The starter set must load and score; the floor is the number measured when it was added (regression guard, raise it as recall improves).
    #[test]
    fn starter_set_scores_above_its_floor() {
        let path = concat!(env!("CARGO_MANIFEST_DIR"), "/../../tests/eval/recall/shop.json");
        let set: RecallSet = serde_json::from_str(&std::fs::read_to_string(path).unwrap()).unwrap();
        let report = evaluate(&set).unwrap();
        assert_eq!(report.overall.queries, set.queries.len());
        // Measured with synonyms on: all 0.982, paraphrase 0.917 (0.946 and 0.750 without them).
        assert!(report.overall.recall_at_5 >= 0.97, "{:?}", report.overall);
        assert!(report.by_kind["paraphrase"].recall_at_5 >= 0.9, "{:?}", report.by_kind["paraphrase"]);
    }
}

#[cfg(test)]
mod perf {
    use super::*;

    /// `cargo test -p automemory-core recall_latency -- --ignored --nocapture`: the hook budget is 5 ms; synonyms must not eat it.
    #[test]
    #[ignore]
    fn recall_latency_at_5000_notes() {
        let path = concat!(env!("CARGO_MANIFEST_DIR"), "/../../tests/eval/recall/shop.json");
        let mut set: RecallSet = serde_json::from_str(&std::fs::read_to_string(path).unwrap()).unwrap();
        let base = set.notes.len();
        for i in 0..5000 {
            let n = &set.notes[i % base];
            set.notes.push(RecallNote { id: format!("x{i}"), kind: n.kind.clone(), summary: format!("{} {i}", n.summary), content: format!("{} variant {i}", n.content), tags: n.tags.clone() });
        }
        set.queries.truncate(48);
        let r = evaluate(&set).unwrap();
        eprintln!("{} queries over {} notes: {:.2} ms per recall", r.overall.queries, set.notes.len(), r.mean_query_ms);
    }
}
