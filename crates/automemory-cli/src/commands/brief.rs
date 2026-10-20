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
