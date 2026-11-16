//! `automemory resume` and `automemory handoff`: picking up where a session (on this or another
//! machine) left off.

use anyhow::Result;
use automemory_core::config::Config;
use automemory_core::db::Database;
use std::path::Path;

/// The project for a folder: registered path first, else the repository key synced from
/// another machine.
pub(crate) fn project_for(db: &Database, cwd: &Path) -> Option<String> {
    if let Ok(Some(p)) = db.find_project_by_path(&cwd.to_string_lossy()) {
        return Some(p.id);
    }
    let key = automemory_core::sync::identity::project_key_for_dir(cwd)?;
    db.project_id_for_key(&key)
}

/// `automemory note "text"`: a human-written, trusted project memory (shown to every agent).
pub fn note(text: &str, kind: &str) -> Result<()> {
    use automemory_core::models::*;
    let db = Database::open(&Config::default_config().db_path)?;
    let cwd = std::env::current_dir()?;
    let root = super::rules_export::repo_root(&cwd);
    let project = project_for(&db, &root);
    let now = chrono::Utc::now();
    let summary: String = text.lines().next().unwrap_or(text).chars().take(120).collect();
    // `automemory note` tells every agent something "in plain words": trusted only when a person wrote it (see memory save).
    let who = automemory_core::actor::current(automemory_core::auth::stored_password_hash().as_deref());
    let person = who.is_human();
    if !person {
        eprintln!("note: saved as an unverified note written by '{}' (no terminal or human password here), so it is not exported into instruction files.", who.name);
    }
    let mem = Memory {
        id: uuid::Uuid::now_v7().to_string(),
        scope: if project.is_some() { MemoryScope::Project } else { MemoryScope::Global },
        project_id: project.clone(),
        memory_type: kind.parse().map_err(|_| anyhow::anyhow!("type must be one of: fact, lesson, pattern, warning, preference"))?,
        content: text.to_string(), summary, confidence: if person { 1.0 } else { 0.9 }, importance: 0.8, freshness: 1.0,
        status: MemoryStatus::Active, source_kind: if person { MemorySourceKind::User } else { MemorySourceKind::Agent }, version: Some(1),
        created_at: now, updated_at: now, last_recalled_at: None, tags: vec![], entities: vec![],
        metadata: if person { serde_json::json!({}) } else { serde_json::json!({ "saved_by": who.name }) },
    };
    db.save_memory(&mem)?;
    super::rules_export::refresh_existing(&db, project.as_deref(), &root);
    println!("✓ Noted. Every agent on this project will see it.");
    Ok(())
}

pub fn resume() -> Result<()> {
    let db = Database::open(&Config::default_config().db_path)?;
    let cwd = std::env::current_dir()?;
    let project = project_for(&db, &cwd);
    match automemory_core::resume::build_resume(&db, project.as_deref(), "cli-resume", &cwd) {
        Some(pack) => println!("{pack}"),
        None => println!("Nothing to resume yet for this folder (no hand-off, checkpoint or decision recorded)."),
    }
    Ok(())
}

pub async fn handoff(note: Option<String>) -> Result<()> {
    let db = Database::open(&Config::default_config().db_path)?;
    let cwd = std::env::current_dir()?;
    let project = project_for(&db, &cwd);
    let session = format!("manual-{}", chrono::Utc::now().timestamp());
    let baton = automemory_core::resume::build_handoff(&db, project.as_deref(), &session, "cli", &cwd, note.as_deref())?;
    println!("{}", baton.packet);
    let (_, remote) = automemory_core::sync::auto::vault_settings();
    if remote.is_some() {
        println!();
        super::sync::auto(true, true)?;
    } else {
        println!("\n(No git remote configured: this hand-off stays on this machine. `automemory sync remote <url>` shares it.)");
    }
    Ok(())
}
