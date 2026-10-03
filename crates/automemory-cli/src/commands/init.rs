use anyhow::{Context, Result};
use automemory_core::config::Config;
use automemory_core::db::Database;
use automemory_core::models::*;
use chrono::Utc;
use std::fs;
use std::path::PathBuf;
use uuid::Uuid;

const AGY_HOOKS_TEMPLATE: &str = r#"{
  "PreInvocation": [
    {
      "type": "command",
      "command": "automemory hook --agent agy --event boot --project auto"
    }
  ],
  "PreToolUse": [
    {
      "type": "command",
      "command": "automemory hook --agent agy --event pre-tool --project auto"
    }
  ],
  "PostToolUse": [
    {
      "type": "command",
      "command": "automemory hook --agent agy --event post-tool --project auto"
    }
  ],
  "PostToolUseFailure": [
    {
      "type": "command",
      "command": "automemory hook --agent agy --event post-tool-fail --project auto"
    }
  ],
  "TaskCompleted": [
    {
      "type": "command",
      "command": "automemory hook --agent agy --event task-done --project auto"
    }
  ],
  "Stop": [
    {
      "type": "command",
      "command": "automemory hook --agent agy --event stop --project auto"
    }
  ]
}
"#;

pub fn execute(agent: Option<String>) -> Result<()> {
    let config = Config::default_config();
    config.ensure_dirs().context("Failed to create configuration directories")?;

    let agent_name = agent.unwrap_or_else(|| "agy".to_string());
    let hook_file = config.hooks_dir.join(format!("{}.json", agent_name));
    fs::write(&hook_file, AGY_HOOKS_TEMPLATE)
        .with_context(|| format!("Failed to write hook configuration to {:?}", hook_file))?;

    // Initialize database
    let db = Database::open(&config.db_path)
        .with_context(|| format!("Failed to initialize database at {:?}", config.db_path))?;

    // Attempt to register current directory as a project if in git repo
    let current_dir = std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."));
    let canonical = fs::canonicalize(&current_dir).unwrap_or(current_dir);
    let path_str = canonical.to_string_lossy().to_string();
    let folder_name = canonical
        .file_name()
        .and_then(|s| s.to_str())
        .unwrap_or("default-project");

    let slug = folder_name.to_lowercase().replace(' ', "-");
    let proj = Project {
        id: Uuid::now_v7().to_string(),
        slug: slug.clone(),
        display_name: folder_name.to_string(),
        created_at: Utc::now(),
        updated_at: Utc::now(),
        capture_policy: CapturePolicy::default(),
        context_budget: ContextBudget::default(),
        document_config: DocumentConfig::default(),
        sync_config: SyncConfig::default(),
    };

    let _ = db.save_project(&proj);
    let _ = db.add_project_path(&proj.id, &path_str, true);

    println!("✓ AutoMemory initialized successfully!");
    println!("  Database: {:?}", config.db_path);
    println!("  Hooks:    {:?}", hook_file);
    println!("  Project:  {} ({})", folder_name, path_str);
    println!("\nTo verify setup, run: automemory doctor");
    println!("To start background daemon & UI, run: automemory serve");

    Ok(())
}
