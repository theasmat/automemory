//! `automemory resume` and `automemory handoff`: picking up where a session (on this or another
//! machine) left off.

use anyhow::Result;
use automemory_core::config::Config;
use automemory_core::db::Database;
use std::path::Path;

/// The project for a folder: registered path first, else the repository key synced from
/// another machine.
fn project_for(db: &Database, cwd: &Path) -> Option<String> {
    if let Ok(Some(p)) = db.find_project_by_path(&cwd.to_string_lossy()) {
        return Some(p.id);
    }
    let key = automemory_core::sync::identity::project_key_for_dir(cwd)?;
    db.project_id_for_key(&key)
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
