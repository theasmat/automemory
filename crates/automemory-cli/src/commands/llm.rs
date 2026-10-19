//! `automemory llm`: an optional model that only suggests (conflicts, duplicates, candidate notes) into the Inbox.

use anyhow::{bail, Result};
use automemory_core::config::{Config, GlobalConfig};
use automemory_core::db::Database;
use automemory_core::llm::{self, LlmClient};
use std::sync::atomic::AtomicBool;

fn db() -> Result<Database> {
    Database::open(&Config::default_config().db_path)
}

pub fn status() -> Result<()> {
    let cfg = GlobalConfig::load()?;
    let s = &cfg.llm;
    if s.model.is_empty() {
        println!("LLM helper: off (the daemon makes no LLM call). Turn on: automemory llm use <model> [--endpoint URL] [--allow-network]");
        return Ok(());
    }
    println!("model:    {} at {}", s.model, if s.endpoint.is_empty() { "https://api.openai.com/v1" } else { &s.endpoint });
    match LlmClient::from_settings(s) {
        Ok(c) => {
            println!("where:    {}", if c.is_local() { "this machine" } else { "sends note text off this machine (secrets removed first)" });
            println!("api key:  {}", automemory_core::secrets::status(llm::SECRET_NAME, Some(&s.api_key_env)).label());
            println!("today:    {} of {} tokens", llm::tokens_today(&db()?), s.max_tokens_per_day);
        }
        Err(e) => println!("problem:  {e}"),
    }
    Ok(())
}

pub fn use_model(model: String, endpoint: Option<String>, api_key_env: Option<String>, allow_network: bool) -> Result<()> {
    let mut cfg = GlobalConfig::load()?;
    cfg.llm.model = model;
    if let Some(e) = endpoint { cfg.llm.endpoint = e }
    if let Some(k) = api_key_env { cfg.llm.api_key_env = k }
    cfg.llm.allow_network = allow_network;
    cfg.save()?;
    status()
}

pub fn off() -> Result<()> {
    let mut cfg = GlobalConfig::load()?;
    cfg.llm.model.clear();
    cfg.save()?;
    println!("✓ LLM helper off.");
    Ok(())
}

pub fn key_set(stdin: bool, from_env: Option<String>) -> Result<()> {
    let value = if let Some(var) = from_env {
        std::env::var(&var).map_err(|_| anyhow::anyhow!("{var} is not set"))?
    } else if stdin {
        let mut line = String::new();
        std::io::stdin().read_line(&mut line)?;
        line.trim().to_string()
    } else {
        #[cfg(unix)]
        {
            automemory_core::auth::prompt_secure("LLM API key (hidden): ")
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
    let src = automemory_core::secrets::set(llm::SECRET_NAME, value.trim())?;
    println!("✓ LLM key stored in the {}. It is never written to config.toml, the database or logs.", src.label());
    Ok(())
}

pub fn key_remove() -> Result<()> {
    let removed = automemory_core::secrets::delete(llm::SECRET_NAME)?;
    println!("{}", if removed { "✓ LLM key removed." } else { "No stored LLM key." });
    Ok(())
}

/// Runs one job to completion and prints what it suggested. `what`: `pairs` (conflicts and duplicates) or `distill`.
pub fn run_job(what: &str, cancel: &AtomicBool, progress: &mut dyn FnMut(usize, usize)) -> Result<String> {
    let cfg = GlobalConfig::load()?;
    let client = LlmClient::from_settings(&cfg.llm)?;
    let db = db()?;
    let cwd = std::env::current_dir()?;
    let project = db.find_project_by_path(&cwd.to_string_lossy()).ok().flatten().map(|p| p.id);
    let mut complete = llm::metered(&db, &client);
    match what {
        "pairs" => {
            let r = llm::analyze_pairs(&db, project.as_deref(), 40, cancel, &mut complete, progress)?;
            Ok(format!("Checked {} pair(s): {} conflict(s) and {} duplicate(s) suggested. Review them in the Inbox.", r.checked, r.conflicts, r.duplicates))
        }
        "distill" => {
            let n = llm::distill_sessions(&db, project.as_deref(), &mut complete)?;
            Ok(format!("{n} candidate note(s) suggested from recent sessions. Approve them in the Inbox to add them."))
        }
        other => bail!("unknown job '{other}': use pairs or distill"),
    }
}

pub fn run(what: &str) -> Result<()> {
    println!("{}", run_job(what, &AtomicBool::new(false), &mut |_, _| {})?);
    Ok(())
}
