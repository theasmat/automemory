//! `automemory embeddings`: choose and check the recall mode and embedding provider.

use anyhow::{bail, Result};
use automemory_core::config::{Config, GlobalConfig};
use automemory_core::db::Database;
use automemory_core::embeddings::{eval_ranking, provider_from_settings, EvalQuery, PRESETS};
use std::path::PathBuf;

fn db() -> Result<Database> {
    Database::open(&Config::default_config().db_path)
}

pub fn status() -> Result<()> {
    let cfg = GlobalConfig::load()?;
    let eff = cfg.embeddings.resolved();
    println!("recall mode: {}", cfg.retrieval.mode);
    println!("profile:     {} ({} {})", eff.active, eff.provider, eff.model);
    match provider_from_settings(&cfg.embeddings) {
        Ok(p) => {
            let db = db()?;
            let (embedded, active, others) = db.vector_coverage(&p.model_id())?;
            println!("model id:    {} ({})", p.model_id(), if p.is_local() { "local" } else { "sends text off this machine" });
            if !p.is_local() {
                println!("api key:     {}", automemory_core::secrets::status(&eff.secret_name(), Some(&eff.api_key_env)).label());
            }
            println!("indexed:     {embedded}/{active} active memories");
            let (pending, tokens) = automemory_core::embeddings::pending_estimate(&db, p.as_ref())?;
            if pending > 0 {
                let cost = automemory_core::embeddings::price_per_million_tokens(&eff.model).map(|per| tokens as f64 / 1e6 * per);
                println!("to index:    {pending} memories, about {tokens} tokens{}", cost.map(|c| format!(" (about ${c:.4}, an estimate)")).unwrap_or_default());
            }
            for (m, n) in others {
                println!("other model: {m} ({n} vectors, unused; `automemory embeddings clean` removes them)");
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
    if all {
        automemory_core::embeddings::drop_model_vectors(&db, &p.model_id())?;
    }
    let cancel = std::sync::atomic::AtomicBool::new(false);
    let cap = cfg.embeddings.max_tokens_per_day;
    let n = automemory_core::embeddings::index_with_progress(&db, p.as_ref(), cap, &cancel, &mut |done, total| {
        eprint!("\r  embedding {done}/{total}   ");
    })?;
    eprintln!();
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
    automemory_core::embeddings::index_pending(&db, p.as_ref(), usize::MAX)?;
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

fn profile_name(cfg: &GlobalConfig, given: Option<String>) -> String {
    given.unwrap_or_else(|| cfg.embeddings.resolved().active)
}

/// `embeddings key set [profile]`: hidden prompt, `--stdin`, or `--from-env VAR`. Stored in the OS keychain.
pub fn key_set(profile: Option<String>, stdin: bool, from_env: Option<String>) -> Result<()> {
    let cfg = GlobalConfig::load()?;
    let name = profile_name(&cfg, profile);
    let value = if let Some(var) = from_env {
        std::env::var(&var).map_err(|_| anyhow::anyhow!("{var} is not set"))?
    } else if stdin {
        let mut line = String::new();
        std::io::stdin().read_line(&mut line)?;
        line.trim().to_string()
    } else {
        #[cfg(unix)]
        {
            automemory_core::auth::prompt_secure(&format!("API key for profile '{name}' (hidden): "))
                .ok_or_else(|| anyhow::anyhow!("a hidden prompt needs a terminal; use --stdin or --from-env VAR"))?
        }
        #[cfg(not(unix))]
        {
            bail!("use --stdin or --from-env VAR on this system");
        }
    };
    if value.trim().is_empty() {
        bail!("the key is empty; nothing was stored");
    }
    let src = automemory_core::secrets::set(&format!("embeddings.{name}"), value.trim())?;
    println!("✓ Key for profile '{name}' stored in the {}. It is never written to config.toml, the database or logs.", src.label());
    Ok(())
}

pub fn key_status(profile: Option<String>) -> Result<()> {
    let cfg = GlobalConfig::load()?;
    let name = profile_name(&cfg, profile);
    let env = cfg.embeddings.profiles.get(&name).map(|p| p.api_key_env.clone()).unwrap_or(cfg.embeddings.api_key_env.clone());
    println!("profile '{name}': {}", automemory_core::secrets::status(&format!("embeddings.{name}"), Some(&env)).label());
    Ok(())
}

pub fn key_remove(profile: Option<String>) -> Result<()> {
    let cfg = GlobalConfig::load()?;
    let name = profile_name(&cfg, profile);
    let removed = automemory_core::secrets::delete(&format!("embeddings.{name}"))?;
    println!("{}", if removed { format!("✓ Key for profile '{name}' removed.") } else { format!("No stored key for profile '{name}'.") });
    Ok(())
}

pub fn models() -> Result<()> {
    println!("Ready-made setups (use with `automemory embeddings profile add <name> --preset <id>`):\n");
    for p in PRESETS {
        let dims = p.dimensions.map(|d| format!("{d}-dim")).unwrap_or_else(|| "any size".into());
        println!("  {:<14} {:<40} {:<9} {}", p.id, p.label, dims, if p.local { "local" } else { "cloud (needs a key)" });
        println!("  {:<14} {}", "", p.note);
    }
    // Models installed in a local Ollama, when it is running.
    if let Ok(client) = reqwest::blocking::Client::builder().timeout(std::time::Duration::from_secs(2)).build() {
        if let Ok(resp) = client.get("http://127.0.0.1:11434/api/tags").send() {
            if let Ok(v) = resp.json::<serde_json::Value>() {
                let names: Vec<String> = v["models"].as_array().into_iter().flatten().filter_map(|m| m["name"].as_str().map(String::from)).collect();
                if !names.is_empty() {
                    println!("\nInstalled in your Ollama: {}", names.join(", "));
                }
            }
        }
    }
    Ok(())
}

pub fn profile_list() -> Result<()> {
    let cfg = GlobalConfig::load()?;
    let active = cfg.embeddings.resolved().active;
    println!("{:<2} {:<14} {:<8} {:<30} {}", "", "profile", "provider", "model", "key");
    let flat = &cfg.embeddings;
    let mut rows: Vec<(String, String, String, String)> = vec![("default".into(), flat.provider.clone(), flat.model.clone(), flat.api_key_env.clone())];
    for (n, p) in &cfg.embeddings.profiles {
        rows.push((n.clone(), p.provider.clone(), p.model.clone(), p.api_key_env.clone()));
    }
    for (n, prov, model, env) in rows {
        let key = if prov == "ngram" || prov == "ollama" || prov == "exec" { "-".to_string() } else { automemory_core::secrets::status(&format!("embeddings.{n}"), Some(&env)).label() };
        println!("{:<2} {:<14} {:<8} {:<30} {}", if n == active { "*" } else { "" }, n, prov, model, key);
    }
    Ok(())
}

pub fn profile_add(name: String, preset: Option<String>, provider: Option<String>, model: Option<String>, endpoint: Option<String>, dimensions: Option<usize>, allow_network: bool) -> Result<()> {
    use automemory_core::config::EmbeddingProfile;
    if name == "default" || name.trim().is_empty() || !name.chars().all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_') {
        bail!("profile names use letters, digits, - and _ (and `default` is the built-in one)");
    }
    let mut p = EmbeddingProfile { provider: "ngram".into(), model: String::new(), endpoint: String::new(), api_key_env: String::new(), allow_network, dimensions, timeout_secs: 8 };
    if let Some(id) = preset {
        let pr = PRESETS.iter().find(|x| x.id == id).ok_or_else(|| anyhow::anyhow!("unknown preset '{id}': see `automemory embeddings models`"))?;
        p.provider = pr.provider.into();
        p.model = pr.model.into();
        p.endpoint = pr.endpoint.into();
        p.dimensions = dimensions.or(pr.dimensions);
    }
    if let Some(v) = provider { p.provider = v; }
    if let Some(v) = model { p.model = v; }
    if let Some(v) = endpoint { p.endpoint = v; }
    let mut cfg = GlobalConfig::load()?;
    cfg.embeddings.profiles.insert(name.clone(), p);
    // Refuse a profile that cannot work at all (missing model, remote endpoint without allow_network).
    let mut probe = cfg.embeddings.clone();
    probe.active = name.clone();
    if let Err(e) = provider_from_settings(&probe) {
        let msg = e.to_string();
        if !msg.contains("embeddings key set") {
            bail!("{msg}");
        }
    }
    cfg.save()?;
    println!("✓ Profile '{name}' saved. Cloud profiles need a key: `automemory embeddings key set {name}`. Switch with `automemory embeddings profile use {name}`.");
    Ok(())
}

pub fn profile_use(name: String) -> Result<()> {
    let mut cfg = GlobalConfig::load()?;
    if name != "default" && !cfg.embeddings.profiles.contains_key(&name) {
        bail!("no profile '{name}': see `automemory embeddings profile list`");
    }
    cfg.embeddings.active = if name == "default" { String::new() } else { name.clone() };
    cfg.retrieval.mode = "hybrid".into();
    cfg.save()?;
    println!("✓ Using profile '{name}' (hybrid recall). Run `automemory embeddings reindex` to index with it.");
    Ok(())
}

pub fn profile_remove(name: String) -> Result<()> {
    let mut cfg = GlobalConfig::load()?;
    if cfg.embeddings.profiles.remove(&name).is_none() {
        bail!("no profile '{name}'");
    }
    if cfg.embeddings.active == name {
        cfg.embeddings.active.clear();
    }
    cfg.save()?;
    let _ = automemory_core::secrets::delete(&format!("embeddings.{name}"));
    println!("✓ Profile '{name}' and its stored key removed.");
    Ok(())
}

pub fn clean() -> Result<()> {
    let cfg = GlobalConfig::load()?;
    let p = provider_from_settings(&cfg.embeddings)?;
    let n = automemory_core::embeddings::remove_unused_vectors(&db()?, &p.model_id())?;
    println!("removed {n} unused vectors (kept those of {})", p.model_id());
    Ok(())
}
