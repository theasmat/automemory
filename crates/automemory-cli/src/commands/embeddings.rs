//! `automemory embeddings`: choose and check the recall mode and embedding provider.

use anyhow::{bail, Result};
use automemory_core::config::{Config, GlobalConfig};
use automemory_core::db::Database;
use automemory_core::embeddings::{eval_ranking, index_pending, provider_from_settings, reindex_all, EvalQuery};
use std::path::PathBuf;

fn db() -> Result<Database> {
    Database::open(&Config::default_config().db_path)
}

pub fn status() -> Result<()> {
    let cfg = GlobalConfig::load()?;
    println!("recall mode: {}", cfg.retrieval.mode);
    println!("provider:    {} {}", cfg.embeddings.provider, cfg.embeddings.model);
    match provider_from_settings(&cfg.embeddings) {
        Ok(p) => {
            let (embedded, active, others) = db()?.vector_coverage(&p.model_id())?;
            println!("model id:    {} ({})", p.model_id(), if p.is_local() { "local" } else { "sends text off this machine" });
            println!("indexed:     {embedded}/{active} active memories");
            for (m, n) in others {
                println!("other model: {m} ({n} vectors, unused)");
            }
        }
        Err(e) => println!("provider problem: {e}\n(recall falls back to keyword)"),
    }
    Ok(())
}

pub fn test() -> Result<()> {
    let cfg = GlobalConfig::load()?;
    let p = provider_from_settings(&cfg.embeddings)?;
    let start = std::time::Instant::now();
    let v = p.embed("automemory embedding self-test")?;
    println!("ok: {} gave a {}-dimension vector in {} ms", p.model_id(), v.len(), start.elapsed().as_millis());
    Ok(())
}

pub fn reindex(all: bool) -> Result<()> {
    let cfg = GlobalConfig::load()?;
    let p = provider_from_settings(&cfg.embeddings)?;
    let db = db()?;
    let n = if all { reindex_all(&db, p.as_ref())? } else { index_pending(&db, p.as_ref(), usize::MAX)? };
    println!("embedded {n} memories with {}", p.model_id());
    Ok(())
}

/// Sets provider settings and switches to hybrid mode. Hosted endpoints stay off until `--allow-network`.
pub fn use_provider(provider: String, model: Option<String>, endpoint: Option<String>, api_key_env: Option<String>, allow_network: bool) -> Result<()> {
    let mut cfg = GlobalConfig::load()?;
    cfg.embeddings.provider = provider;
    if let Some(m) = model { cfg.embeddings.model = m; }
    if let Some(e) = endpoint { cfg.embeddings.endpoint = e; }
    if let Some(k) = api_key_env { cfg.embeddings.api_key_env = k; }
    cfg.embeddings.allow_network = allow_network;
    provider_from_settings(&cfg.embeddings)?; // refuse a config that cannot work
    cfg.retrieval.mode = "hybrid".into();
    cfg.save()?;
    println!("hybrid recall on, provider {}. Run `automemory embeddings reindex` to index existing memories.", cfg.embeddings.provider);
    Ok(())
}

pub fn off() -> Result<()> {
    let mut cfg = GlobalConfig::load()?;
    cfg.retrieval.mode = "keyword".into();
    cfg.save()?;
    println!("keyword recall (no model, no network). Stored vectors are kept.");
    Ok(())
}

/// Compares keyword and hybrid recall on a JSON file: `[{"query": "...", "expect": ["id-or-summary-fragment"]}]`.
pub fn eval(file: PathBuf) -> Result<()> {
    let queries: Vec<EvalQuery> = serde_json::from_str(&std::fs::read_to_string(&file)?)?;
    if queries.is_empty() {
        bail!("{} has no queries", file.display());
    }
    let cfg = GlobalConfig::load()?;
    let p = provider_from_settings(&cfg.embeddings)?;
    let db = db()?;
    index_pending(&db, p.as_ref(), usize::MAX)?;
    let kw = eval_ranking(&queries, |q| {
        Ok(db.search_memories_fts(q, None, 10)?.into_iter().map(|(m, _)| (m.id, m.summary)).collect())
    })?;
    let hy = eval_ranking(&queries, |q| {
        let mut fused: Vec<(String, String, f64)> = Vec::new();
        let add = |fused: &mut Vec<(String, String, f64)>, list: Vec<(String, String)>| {
            for (rank, (id, s)) in list.into_iter().enumerate() {
                let w = 1.0 / (60.0 + rank as f64);
                match fused.iter_mut().find(|f| f.0 == id) {
                    Some(f) => f.2 += w,
                    None => fused.push((id, s, w)),
                }
            }
        };
        add(&mut fused, db.search_memories_fts(q, None, 10)?.into_iter().map(|(m, _)| (m.id, m.summary)).collect());
        let emb = p.embed(q)?;
        add(&mut fused, db.search_memories_vector(&emb, &p.model_id(), None, 10)?.into_iter().map(|(m, _)| (m.id, m.summary)).collect());
        fused.sort_by(|a, b| b.2.total_cmp(&a.2));
        Ok(fused.into_iter().map(|(i, s, _)| (i, s)).collect())
    })?;
    println!("{:<10} {:>6} {:>8} {:>8}", "mode", "P@3", "MRR", "ms/query");
    println!("{:<10} {:>6.2} {:>8.2} {:>8.1}", "keyword", kw.precision_at_3, kw.mrr, kw.mean_ms);
    println!("{:<10} {:>6.2} {:>8.2} {:>8.1}", "hybrid", hy.precision_at_3, hy.mrr, hy.mean_ms);
    Ok(())
}
