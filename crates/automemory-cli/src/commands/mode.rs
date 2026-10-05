use anyhow::{Context, Result};
use automemory_core::config::Config;
use automemory_core::db::Database;
use automemory_core::models::AutopilotMode;
use std::str::FromStr;

pub fn get(project_slug: Option<String>) -> Result<()> {
    let config = Config::default_config();
    let db = Database::open(&config.db_path)
        .with_context(|| format!("Failed to open database at {:?}", config.db_path))?;

    let slug = project_slug.unwrap_or_else(detect_current_project_slug);

    if let Some(project) = db.get_project_by_slug(&slug)? {
        println!("┌────────────────────────────────────────────────────────┐");
        println!("│              AutoMemory Autopilot Mode                 │");
        println!("├────────────────────────────────────────────────────────┤");
        println!("│  Project: {:<44} │", project.slug);
        println!(
            "│  Active Mode: {:<40} │",
            format!("{:?}", project.capture_policy.mode).to_uppercase()
        );
        println!("└────────────────────────────────────────────────────────┘");
        print_mode_description(project.capture_policy.mode);
    } else {
        println!(
            "Project '{}' not found in database. Defaulting to VIBE mode.",
            slug
        );
        print_mode_description(AutopilotMode::Vibe);
    }

    Ok(())
}

pub fn set(mode_str: &str, project_slug: Option<String>) -> Result<()> {
    let mode = AutopilotMode::from_str(mode_str)?;
    let config = Config::default_config();
    let db = Database::open(&config.db_path)
        .with_context(|| format!("Failed to open database at {:?}", config.db_path))?;

    let slug = project_slug.unwrap_or_else(detect_current_project_slug);

    if let Some(mut project) = db.get_project_by_slug(&slug)? {
        project.capture_policy.mode = mode;
        project.updated_at = chrono::Utc::now();
        db.save_project(&project)?;
        println!(
            "✓ Updated project '{}' autopilot mode to '{}'",
            project.slug, mode
        );
    } else {
        // Create new project if doesn't exist
        let new_project = automemory_core::models::Project {
            id: uuid::Uuid::new_v4().to_string(),
            slug: slug.clone(),
            display_name: slug.clone(),
            created_at: chrono::Utc::now(),
            updated_at: chrono::Utc::now(),
            capture_policy: automemory_core::models::CapturePolicy {
                mode,
                ..Default::default()
            },
            context_budget: Default::default(),
            document_config: Default::default(),
            sync_config: Default::default(),
        };
        db.save_project(&new_project)?;
        println!(
            "✓ Created project '{}' and set autopilot mode to '{}'",
            slug, mode
        );
    }

    print_mode_description(mode);
    Ok(())
}

fn detect_current_project_slug() -> String {
    std::env::current_dir()
        .ok()
        .and_then(|p| p.file_name().map(|n| n.to_string_lossy().to_string()))
        .unwrap_or_else(|| "default".to_string())
        .to_lowercase()
        .replace(' ', "-")
}

fn print_mode_description(mode: AutopilotMode) {
    match mode {
        AutopilotMode::Vibe => {
            println!("💡 [Vibe Mode]: Max velocity. Non-critical warnings are inline notifications; only destructive catastrophic actions stop execution.");
        }
        AutopilotMode::Strict => {
            println!("🔒 [Strict Mode]: High safety. Database migrations, cloud teardowns, and known failure repetitions require explicit confirmation.");
        }
        AutopilotMode::Review => {
            println!("🔍 [Review Mode]: Proactive briefing. Sessions surface relevant historical decisions and unresolved failures before task execution.");
        }
    }
}
