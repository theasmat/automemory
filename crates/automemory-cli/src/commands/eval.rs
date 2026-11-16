use anyhow::Result;

/// Runs the local deterministic eval suites (retrieval precision, stale-flag
/// precision, contradiction detection, lease races, hook latency).
/// Exit code is the judge: 0 when every suite passes, 1 otherwise.
pub fn run(json: bool) -> Result<()> {
    let report = automemory_core::eval::run_local_suites();
    if json {
        println!("{}", serde_json::to_string_pretty(&report)?);
    } else {
        println!("{}", report.summary());
    }
    if !report.passed() {
        std::process::exit(1);
    }
    Ok(())
}

/// The starter set ships inside the binary, so `eval recall` works from any folder.
const STARTER_SET: &str = include_str!("../../../../tests/eval/recall/shop.json");

/// `automemory eval recall [file]`: recall@5, nDCG@10 and MRR over a labelled set, overall and per query kind.
pub fn recall(file: Option<std::path::PathBuf>) -> Result<()> {
    let (text, label) = match &file {
        Some(f) => (std::fs::read_to_string(f).map_err(|e| anyhow::anyhow!("cannot read {}: {e}", f.display()))?, f.display().to_string()),
        None => (STARTER_SET.to_string(), "built-in synthetic starter set".to_string()),
    };
    let set: automemory_core::recall_eval::RecallSet = serde_json::from_str(&text)?;
    let report = automemory_core::recall_eval::evaluate(&set)?;
    println!("{} notes, {} queries ({label})", set.notes.len(), report.overall.queries);
    println!("{:<12} {:>7} {:>9} {:>9} {:>6}", "kind", "queries", "recall@5", "nDCG@10", "MRR");
    let row = |name: &str, s: &automemory_core::recall_eval::RecallScore| {
        println!("{:<12} {:>7} {:>9.3} {:>9.3} {:>6.3}", name, s.queries, s.recall_at_5, s.ndcg_at_10, s.mrr)
    };
    for (k, s) in &report.by_kind {
        row(if k.is_empty() { "(none)" } else { k }, s);
    }
    row("ALL", &report.overall);
    println!("mean recall time {:.2} ms", report.mean_query_ms);
    Ok(())
}
