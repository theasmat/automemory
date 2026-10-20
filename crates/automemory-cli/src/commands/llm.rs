//! `automemory llm`: an optional model that only suggests (conflicts, duplicates, candidate notes) into the Inbox.

use anyhow::{bail, Result};
use automemory_core::config::{Config, GlobalConfig, LlmProfile, LlmSettings};
use automemory_core::db::Database;
use automemory_core::llm::{self, LlmClient};
use std::sync::atomic::AtomicBool;

fn db() -> Result<Database> {
    Database::open(&Config::default_config().db_path)
}

pub fn status() -> Result<()> {
    let cfg = GlobalConfig::load()?;
    let s = cfg.llm.resolved();
    if s.model.is_empty() {
        println!("LLM helper: off (the daemon makes no LLM call). Turn on: automemory llm use <model> [--endpoint URL] [--allow-network]");
        return Ok(());
    }
    println!("profile:  {}", if cfg.llm.active.is_empty() { "default" } else { &cfg.llm.active });
    println!("model:    {} at {}", s.model, if s.endpoint.is_empty() { "https://api.openai.com/v1" } else { &s.endpoint });
    match LlmClient::from_settings(&cfg.llm) {
        Ok(c) => {
            println!("where:    {}", if c.is_local() { "this machine" } else { "sends note text off this machine (secrets removed first)" });
            println!("api key:  {}", automemory_core::secrets::status(&s.secret_name(), Some(&s.api_key_env)).label());
            println!("today:    {} of {} tokens", llm::tokens_today(&db()?), s.max_tokens_per_day);
        }
        Err(e) => println!("problem:  {e}"),
    }
    Ok(())
}

/// Sets the `default` profile and makes it the active one.
pub fn use_model(model: String, endpoint: Option<String>, api_key_env: Option<String>, allow_network: bool) -> Result<()> {
    let mut cfg = GlobalConfig::load()?;
    cfg.llm.model = model;
    if let Some(e) = endpoint { cfg.llm.endpoint = e }
    if let Some(k) = api_key_env { cfg.llm.api_key_env = k }
    cfg.llm.allow_network = allow_network;
    cfg.llm.active.clear();
    cfg.save()?;
    status()
}

pub fn off() -> Result<()> {
    let mut cfg = GlobalConfig::load()?;
    cfg.llm.model.clear();
    cfg.llm.active.clear();
    cfg.save()?;
    println!("✓ LLM helper off.");
    Ok(())
}

pub fn profile_add(name: String, model: String, endpoint: Option<String>, api_key_env: Option<String>, allow_network: bool) -> Result<()> {
    if name == "default" || name.trim().is_empty() || !name.chars().all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_') {
        bail!("profile names use letters, digits, - and _ (and `default` is the built-in one)");
    }
    let mut cfg = GlobalConfig::load()?;
    cfg.llm.profiles.insert(name.clone(), LlmProfile {
        model, endpoint: endpoint.unwrap_or_default(), api_key_env: api_key_env.unwrap_or_default(), allow_network,
        timeout_secs: cfg.llm.timeout_secs,
    });
    // Refuse a profile that cannot work at all (empty model, remote endpoint without allow_network).
    let mut probe = cfg.llm.clone();
    probe.active = name.clone();
    if let Err(e) = LlmClient::from_settings(&probe) {
        let msg = e.to_string();
        if !msg.contains("API key") {
            bail!("{msg}");
        }
    }
    cfg.save()?;
    println!("✓ Profile '{name}' saved. Cloud profiles need a key: `automemory llm key-set --profile {name}`. Switch with `automemory llm profile use {name}`.");
    Ok(())
}

pub fn profile_use(name: String) -> Result<()> {
    let mut cfg = GlobalConfig::load()?;
    if name != "default" && !cfg.llm.profiles.contains_key(&name) {
        bail!("no profile '{name}': see `automemory llm profile list`");
    }
    cfg.llm.active = if name == "default" { String::new() } else { name };
    cfg.save()?;
    status()
}

pub fn profile_list() -> Result<()> {
    let cfg = GlobalConfig::load()?;
    let mark = |n: &str| if (cfg.llm.active.is_empty() && n == "default") || cfg.llm.active == n { "*" } else { " " };
    println!("{} default   {} {}", mark("default"), if cfg.llm.model.is_empty() { "(off)" } else { &cfg.llm.model }, cfg.llm.endpoint);
    for (n, p) in &cfg.llm.profiles {
        println!("{} {:<9} {} {}", mark(n), n, p.model, p.endpoint);
    }
    Ok(())
}

pub fn profile_remove(name: String) -> Result<()> {
    let mut cfg = GlobalConfig::load()?;
    if cfg.llm.profiles.remove(&name).is_none() {
        bail!("no profile '{name}'");
    }
    if cfg.llm.active == name {
        cfg.llm.active.clear();
    }
    cfg.save()?;
    let _ = automemory_core::secrets::delete(&LlmSettings::secret_for(&name));
    println!("✓ Profile '{name}' and its key removed.");
    Ok(())
}

fn profile_or_active(profile: Option<String>) -> Result<String> {
    Ok(LlmSettings::secret_for(&profile.unwrap_or(GlobalConfig::load()?.llm.active)))
}

pub fn key_set(profile: Option<String>, stdin: bool, from_env: Option<String>) -> Result<()> {
    let secret = profile_or_active(profile)?;
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
    let src = automemory_core::secrets::set(&secret, value.trim())?;
    println!("✓ LLM key stored in the {}. It is never written to config.toml, the database or logs.", src.label());
    Ok(())
}

pub fn key_remove(profile: Option<String>) -> Result<()> {
    let removed = automemory_core::secrets::delete(&profile_or_active(profile)?)?;
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
