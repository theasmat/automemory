use anyhow::{Context, Result};
use automemory_core::changed::{changed_since, parse_since};
use automemory_core::config::Config;
use automemory_core::db::Database;
use chrono::Utc;

/// What changed in the project's decisions and memories since `since` (`30m`, `2h`, `7d`, `2w`).
pub fn run(project_slug: Option<String>, since: &str, json: bool) -> Result<()> {
    let span = parse_since(since).with_context(|| format!("'{since}' is not a time span; use 30m, 2h, 7d or 2w"))?;
    let config = Config::default_config();
    let db = Database::open(&config.db_path).with_context(|| format!("Failed to open database at {:?}", config.db_path))?;
    let slug = project_slug.unwrap_or_else(|| db.project_slug_for_dir(&std::env::current_dir().unwrap_or_default()));
    let project = db.get_project_by_slug(&slug)?;
    let project_id = project.as_ref().map(|p| p.id.as_str());
    let c = changed_since(&db, project_id, Utc::now() - span)?;

    if json {
        let out = serde_json::json!({
            "since": since,
            "decisions_recorded": c.decisions_recorded.iter().map(|d| d.dated_json()).collect::<Vec<_>>(),
            "decisions_superseded": c.decisions_superseded.iter().map(|d| d.dated_json()).collect::<Vec<_>>(),
            "memories_added": c.memories_added.iter().map(|m| serde_json::json!({"id": m.id, "summary": m.summary, "created_at": m.created_at})).collect::<Vec<_>>(),
            "memories_inactive": c.memories_inactive.iter().map(|m| serde_json::json!({"id": m.id, "summary": m.summary, "status": m.status.to_string(), "changed_at": m.updated_at})).collect::<Vec<_>>(),
        });
        println!("{}", serde_json::to_string_pretty(&out)?);
        return Ok(());
    }

    println!("Changed in '{slug}' in the last {since}\n");
    println!("Decisions recorded ({})", c.decisions_recorded.len());
    for d in &c.decisions_recorded {
        println!("  + {} — {} ({})", d.topic, d.decision, d.created_at.format("%Y-%m-%d %H:%M"));
    }
    println!("Decisions superseded ({})", c.decisions_superseded.len());
    for d in &c.decisions_superseded {
        let end = d.invalid_at().map(|t| t.format("%Y-%m-%d %H:%M").to_string()).unwrap_or_default();
        println!("  - {} — {} (valid {} to {})", d.topic, d.decision, d.valid_from().format("%Y-%m-%d"), end);
    }
    println!("Memories added ({})", c.memories_added.len());
    for m in &c.memories_added {
        println!("  + {} ({})", m.summary, m.created_at.format("%Y-%m-%d %H:%M"));
    }
    println!("Memories no longer active ({})", c.memories_inactive.len());
    for m in &c.memories_inactive {
        println!("  - [{}] {} ({})", m.status, m.summary, m.updated_at.format("%Y-%m-%d %H:%M"));
    }
    Ok(())
}
