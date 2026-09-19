use anyhow::{Context, Result};
use automemory_core::config::Config;
use automemory_core::db::Database;
use automemory_core::models::*;
use chrono::Utc;
use std::fs;
use std::path::PathBuf;
use uuid::Uuid;

pub fn list() -> Result<()> {
    let config = Config::default_config();
    let db = Database::open(&config.db_path)
        .with_context(|| format!("Failed to open database at {:?}", config.db_path))?;

    let projects = db.list_projects()?;
    if projects.is_empty() {
        println!("No projects registered. Run 'automemory init' or 'automemory project register'.");
        return Ok(());
    }

    println!("{:<38} {:<20} {:<10} {:<30}", "ID", "SLUG", "SYNC", "NAME");
    println!("{}", "-".repeat(100));
    for p in projects {
        println!(
            "{:<38} {:<20} {:<10} {:<30}",
            p.id, p.slug, p.sync_config.mode, p.display_name
        );
        let paths = db.get_project_paths(&p.id)?;
        for path in paths {
            println!("   ↳ Path: {} (primary: {})", path.local_path, path.is_primary);
        }
    }
    Ok(())
}

pub fn register(slug: &str, display_name: &str, path: Option<String>) -> Result<()> {
    let config = Config::default_config();
    let db = Database::open(&config.db_path)?;

    let target_path = path.map(PathBuf::from).unwrap_or_else(|| {
        std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."))
    });
    let canonical = fs::canonicalize(&target_path).unwrap_or(target_path);
    let path_str = canonical.to_string_lossy().to_string();

    let proj = Project {
        id: Uuid::now_v7().to_string(),
        slug: slug.to_string(),
        display_name: display_name.to_string(),
        created_at: Utc::now(),
        updated_at: Utc::now(),
        capture_policy: CapturePolicy::default(),
        context_budget: ContextBudget::default(),
        document_config: DocumentConfig::default(),
        sync_config: SyncConfig::default(),
    };

    db.save_project(&proj)?;
    db.add_project_path(&proj.id, &path_str, true)?;

    println!("✓ Registered project '{}' ({}) at '{}'", display_name, slug, path_str);
    Ok(())
}
