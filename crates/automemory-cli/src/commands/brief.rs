//! `automemory continuity`: see and change the brief the next agent is handed. Changes are yours (password when set).
use anyhow::{anyhow, bail, Result};
use automemory_core::config::Config;
use automemory_core::continuity::{deliver, ItemKind, ItemStatus, NewItem, Origin};
use automemory_core::db::Database;

fn open() -> Result<(Database, Option<String>)> {
    let db = Database::open(&Config::default_config().db_path)?;
    let cwd = std::env::current_dir()?;
    let project = db.find_project_by_path(&cwd.to_string_lossy()).ok().flatten().map(|p| p.id);
    Ok((db, project))
}

fn human(what: &str) -> Result<(Database, Option<String>)> {
    automemory_core::auth::require_human(automemory_core::auth::stored_password_hash().as_deref(), what)?;
    open()
}

pub fn show(all: bool) -> Result<()> {
    let (db, project) = open()?;
    let items = db.brief_list(project.as_deref(), all)?;
    if items.is_empty() {
        println!("The brief is empty. Agents fill it with continuity.brief; you can add: automemory continuity add goal \"…\"");
        return Ok(());
    }
    println!("{}\n", deliver::render_core(&db, project.as_deref(), 100_000).unwrap_or_default());
    println!("Items (id, kind, state, status):");
    for i in items {
        println!("  {}  {:<10} {:<9} {:<8} {}", i.id, i.kind.as_str(), i.state.as_str(), i.status.as_str(), i.text.chars().take(60).collect::<String>());
    }
    Ok(())
}

pub fn add(kind: &str, text: &str, reason: Option<String>) -> Result<()> {
    let kind: ItemKind = kind.parse().map_err(|e: String| anyhow!(e))?;
    let (db, project) = human("add to the brief")?;
    let (item, created) = db.brief_add(NewItem { project_id: project, kind, text: text.to_string(), reason, ref_id: None, origin: Origin::Human, author: "human".into(), session: None })?;
    println!("✓ {} {} ({}). Agents that start in this project will see it.", if created { "Added" } else { "Already there, refreshed:" }, item.kind.as_str(), item.id);
    Ok(())
}

pub fn edit(id: &str, text: &str, reason: Option<String>) -> Result<()> {
    let (db, _) = human("edit the brief")?;
    let item = db.brief_correct(id, text, reason.as_deref(), true, "human")?;
    println!("✓ Edited {} (agents can no longer change it).", item.id);
    Ok(())
}

pub fn set_status(id: &str, status: ItemStatus) -> Result<()> {
    let (db, _) = human("change the brief")?;
    if !db.brief_set_status(id, status)? {
        bail!("no such item");
    }
    println!("✓ {} is now {}.", id, status.as_str());
    Ok(())
}

pub fn confirm(id: &str) -> Result<()> {
    let (db, _) = human("confirm a brief item")?;
    let item = db.brief_confirm(id)?;
    println!("✓ {} is confirmed.", item.id);
    Ok(())
}

pub fn forget(all: bool) -> Result<()> {
    let (db, project) = human("delete the brief")?;
    if project.is_none() && !all {
        bail!("this folder is not a registered project; pass --all to delete the brief of every project");
    }
    let n = db.brief_forget(if all { None } else { project.as_deref() })?;
    println!("✓ Deleted {n} item(s).");
    Ok(())
}

// ── Chat history (opt-in per agent) ─────────────────────────────────────────────────
use automemory_core::continuity::{import, readers};

/// The folder an agent ran in for this project: the project's registered path, else where we are.
fn project_root(db: &Database, project: Option<&str>) -> std::path::PathBuf {
    project
        .and_then(|p| db.conn().query_row("SELECT local_path FROM project_paths WHERE project_id = ?1 ORDER BY is_primary DESC LIMIT 1", [p], |r| r.get::<_, String>(0)).ok())
        .map(std::path::PathBuf::from)
        .unwrap_or_else(|| std::env::current_dir().unwrap_or_default())
}

pub fn sources() -> Result<()> {
    let (db, project) = open()?;
    let cfg = automemory_core::config::GlobalConfig::load()?;
    let home = dirs::home_dir().unwrap_or_default();
    let root = project_root(&db, project.as_deref());
    println!("Chat history is off for an agent until you turn it on (nothing is read, nothing leaves this computer).\n");
    for r in readers::registry() {
        let on = cfg.continuity.read_chats.iter().any(|a| a == r.id());
        let found = r.sessions(&home, &root).len();
        let stored = db.chat_sessions(project.as_deref(), Some(r.id()))?;
        println!("{} ({})  {}", r.label(), r.id(), if on { "ON" } else { "off" });
        println!("   reads: {}", r.locations(&home).iter().map(|p| p.display().to_string()).collect::<Vec<_>>().join(", "));
        println!("   chats of this project found: {found}; imported: {} session(s), {} message(s)", stored.len(), stored.iter().map(|s| s.messages).sum::<i64>());
    }
    println!("\nTurn on:  automemory continuity enable <agent>    Import:  automemory continuity import    Search:  automemory continuity search \"words\"");
    Ok(())
}

fn set_enabled(agent: &str, on: bool) -> Result<()> {
    if readers::reader(agent).is_none() {
        bail!("no reader for '{agent}': known: {}", readers::registry().iter().map(|r| r.id()).collect::<Vec<_>>().join(", "));
    }
    automemory_core::auth::require_human(automemory_core::auth::stored_password_hash().as_deref(), "change which agents' chats are read")?;
    let mut cfg = automemory_core::config::GlobalConfig::load()?;
    cfg.continuity.read_chats.retain(|a| a != agent);
    if on {
        cfg.continuity.read_chats.push(agent.to_string());
    }
    cfg.save()?;
    Ok(())
}

pub fn enable(agent: &str) -> Result<()> {
    set_enabled(agent, true)?;
    let r = readers::reader(agent).ok_or_else(|| anyhow!("unknown agent"))?;
    println!("✓ AutoMemory may now read {}'s chats of your registered projects ({}). Secrets are removed and nothing is copied raw. Run: automemory continuity import",
        r.label(), r.locations(&dirs::home_dir().unwrap_or_default()).iter().map(|p| p.display().to_string()).collect::<Vec<_>>().join(", "));
    Ok(())
}

pub fn disable(agent: &str) -> Result<()> {
    set_enabled(agent, false)?;
    println!("✓ {agent} chats are no longer read. Already imported history stays until: automemory continuity history-forget --agent {agent}");
    Ok(())
}

/// Imports this project's new chat messages for every enabled agent (or just `only`).
pub fn import_chats(only: Option<&str>) -> Result<()> {
    let (db, project) = open()?;
    let cfg = automemory_core::config::GlobalConfig::load()?;
    let home = dirs::home_dir().unwrap_or_default();
    let root = project_root(&db, project.as_deref());
    let agents: Vec<String> = match only {
        Some(a) if !cfg.continuity.read_chats.iter().any(|x| x == a) => bail!("{a} is off: turn it on first with `automemory continuity enable {a}`"),
        Some(a) => vec![a.to_string()],
        None => cfg.continuity.read_chats.clone(),
    };
    if agents.is_empty() {
        println!("No agent is turned on. See: automemory continuity sources");
        return Ok(());
    }
    for a in agents {
        let Some(r) = readers::reader(&a) else { continue };
        let rep = import::import_project(&db, r.as_ref(), &home, project.as_deref(), &root)?;
        println!("✓ {}: {} session(s) with new messages, {} new message(s) stored.", r.label(), rep.sessions, rep.new_messages);
    }
    Ok(())
}

pub fn search(query: &str, agent: Option<&str>, limit: usize) -> Result<()> {
    let (db, project) = open()?;
    let hits = db.chat_search(query, project.as_deref(), agent, limit.clamp(1, 50))?;
    if hits.is_empty() {
        println!("Nothing found. (Is an agent turned on and imported? automemory continuity sources)");
    }
    for h in hits {
        println!("{}  #{}  {}  {}\n   {}", h.session, h.seq, h.ts.unwrap_or_default().chars().take(16).collect::<String>(), h.title.unwrap_or_default(), h.snippet.replace('\n', " "));
    }
    Ok(())
}

pub fn history_forget(agent: Option<&str>, all: bool) -> Result<()> {
    let (db, project) = human("delete imported chat history")?;
    if project.is_none() && !all {
        bail!("this folder is not a registered project; pass --all to delete history of every project");
    }
    let n = db.chat_forget(if all { None } else { project.as_deref() }, agent)?;
    println!("✓ Deleted {n} message(s) of imported chat history.");
    Ok(())
}
