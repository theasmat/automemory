use anyhow::{Context, Result};
use automemory_core::config::{
    default_trackable_files, Config, GlobalConfig, ProjectRegistryEntry,
};
use automemory_core::db::Database;
use automemory_core::models::*;
use chrono::Utc;
use std::fs;
use std::path::{Path, PathBuf};
use uuid::Uuid;

pub fn list() -> Result<()> {
    let global_cfg = GlobalConfig::load()?;
    let config = Config::default_config();

    println!("==========================================================================");
    println!("             AutoMemory — Central Project Registry                        ");
    println!(
        "           Global Config: {}                                              ",
        GlobalConfig::config_path().display()
    );
    println!("==========================================================================\n");

    if let Some(ref remote) = global_cfg.vault.remote_url {
        println!("  🌐 Global GitHub Vault: {}", remote);
    } else {
        println!("  🌐 Global GitHub Vault: Local Vault (No remote git URL set)");
    }
    println!(
        "  📁 Local Vault Store:   {}",
        global_cfg
            .vault
            .local_vault_dir
            .as_deref()
            .unwrap_or(&config.vault_dir)
            .display()
    );
    println!(
        "  ⚙️  Default Autopilot:  {}",
        global_cfg.defaults.autopilot_mode.to_uppercase()
    );
    println!();

    if global_cfg.projects.is_empty() {
        println!("  No projects registered in global config yet.");
        println!("  To register this workspace: 'automemory project register'");
        return Ok(());
    }

    println!(
        "{:<20} {:<45} {:<12}",
        "PROJECT NAME", "LOCAL DIRECTORY", "AUTO-RESTORE"
    );
    println!("{}", "-".repeat(80));

    for p in &global_cfg.projects {
        let exists = Path::new(&p.local_dir).exists();
        let status_mark = if exists { "✓" } else { "⚠ (Missing)" };
        println!(
            "{:<20} {:<45} {:<12}",
            p.name,
            format!("{} {}", p.local_dir, status_mark),
            if p.restore_to_workspace {
                "Enabled"
            } else {
                "Disabled"
            }
        );
        if let Some(ref repo) = p.vault_repo {
            println!("   ↳ Dedicated Vault: {}", repo);
        }
        println!("   ↳ Trackable:       {}", p.trackable_files.join(", "));
    }

    Ok(())
}

pub fn register(
    name_opt: Option<String>,
    path_opt: Option<String>,
    track_opt: Option<String>,
    vault_repo: Option<String>,
    restore_to_workspace: bool,
) -> Result<()> {
    let cwd = std::env::current_dir().context("Failed to get current working directory")?;
    let target_path = path_opt.map(PathBuf::from).unwrap_or(cwd);
    let canonical = fs::canonicalize(&target_path).unwrap_or(target_path);
    let local_dir = canonical.to_string_lossy().to_string();

    let name = name_opt.unwrap_or_else(|| {
        canonical
            .file_name()
            .map(|s| s.to_string_lossy().to_string())
            .unwrap_or_else(|| "default-project".to_string())
    });

    let trackable_files = if let Some(track) = track_opt {
        track
            .split(',')
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty())
            .collect()
    } else {
        default_trackable_files()
    };

    let mut global_cfg = GlobalConfig::load()?;
    let entry = ProjectRegistryEntry {
        name: name.clone(),
        local_dir: local_dir.clone(),
        trackable_files: trackable_files.clone(),
        exclude_patterns: vec![
            "**/node_modules/**".to_string(),
            "**/target/**".to_string(),
            "**/.git/**".to_string(),
            "**/.env*".to_string(),
        ],
        vault_repo: vault_repo.clone(),
        restore_to_workspace,
    };

    global_cfg.register_project(entry)?;

    // Also register in SQLite database for local indexed queries
    let config = Config::default_config();
    if let Ok(db) = Database::open(&config.db_path) {
        let proj = Project {
            id: Uuid::now_v7().to_string(),
            slug: name.clone(),
            display_name: name.clone(),
            created_at: Utc::now(),
            updated_at: Utc::now(),
            capture_policy: CapturePolicy::default(),
            context_budget: ContextBudget::default(),
            document_config: DocumentConfig {
                include_patterns: trackable_files,
                exclude_patterns: vec!["**/.env*".to_string()],
            },
            sync_config: SyncConfig::default(),
        };
        let _ = db.save_project(&proj);
        let _ = db.add_project_path(&proj.id, &local_dir, true);
    }

    println!(
        "✓ Successfully registered project '{}' in global config!",
        name
    );
    println!("  📁 Local Directory: {}", local_dir);
    println!(
        "  📄 Trackable Files: {} patterns configured",
        default_trackable_files().len()
    );
    println!(
        "  🌐 Global Config:   {}",
        GlobalConfig::config_path().display()
    );
    println!("  🛡️  Zero Footprint:  No .automemory folder created in workspace.");

    Ok(())
}

pub fn remove(name_or_path: &str) -> Result<()> {
    let mut global_cfg = GlobalConfig::load()?;
    let removed = global_cfg.remove_project(name_or_path)?;
    if removed {
        println!("✓ Removed project '{}' from global registry.", name_or_path);
    } else {
        println!(
            "⚠ Project '{}' was not found in global registry. Try: `automemory project list`",
            name_or_path
        );
    }
    Ok(())
}
