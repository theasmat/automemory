use anyhow::Result;
use automemory_core::config::Config;
use automemory_core::db::Database;
use automemory_core::models::*;
use chrono::Utc;
use std::str::FromStr;
use uuid::Uuid;

pub fn list(project: Option<String>, limit: usize) -> Result<()> {
    let config = Config::default_config();
    let db = Database::open(&config.db_path)?;
    let memories = db.list_memories(project.as_deref(), Some(MemoryStatus::Active), None, limit)?;

    if memories.is_empty() {
        println!("No active memories found.");
        return Ok(());
    }

    println!("{:<38} {:<12} {:<8} {:<10} {:<40}", "ID", "TYPE", "SCOPE", "CONFIDENCE", "SUMMARY");
    println!("{}", "-".repeat(110));
    for m in memories {
        println!(
            "{:<38} {:<12} {:<8} {:<10.2} {:<40}",
            m.id,
            m.memory_type.to_string(),
            m.scope.to_string(),
            m.confidence,
            m.summary
        );
    }
    Ok(())
}

pub fn search(query: &str, project: Option<String>, limit: usize) -> Result<()> {
    let config = Config::default_config();
    let db = Database::open(&config.db_path)?;
    let results = db.search_memories_fts(query, project.as_deref(), limit)?;

    if results.is_empty() {
        println!("No matching memories found for query: '{}'", query);
        return Ok(());
    }

    println!("Found {} results for '{}':", results.len(), query);
    println!("{}", "-".repeat(90));
    for (m, rank) in results {
        println!("[{}] {} (Rank: {:.3})", m.memory_type, m.summary, rank);
        println!("  ID: {}", m.id);
        println!("  Content: {}", m.content);
        println!();
    }
    Ok(())
}

pub fn save(
    content: &str,
    summary: &str,
    mem_type: Option<String>,
    scope: Option<String>,
    project_id: Option<String>,
) -> Result<()> {
    let config = Config::default_config();
    let db = Database::open(&config.db_path)?;

    let parsed_type = mem_type
        .as_deref()
        .and_then(|s| MemoryType::from_str(s).ok())
        .unwrap_or(MemoryType::Fact);

    let parsed_scope = scope
        .as_deref()
        .and_then(|s| MemoryScope::from_str(s).ok())
        .unwrap_or(MemoryScope::Project);

    let mem = Memory {
        id: Uuid::now_v7().to_string(),
        scope: parsed_scope,
        project_id,
        memory_type: parsed_type,
        content: content.to_string(),
        summary: summary.to_string(),
        confidence: 1.0,
        importance: 0.8,
        freshness: 1.0,
        status: MemoryStatus::Active,
        source_kind: MemorySourceKind::User,
        version: Some(1),
        created_at: Utc::now(),
        updated_at: Utc::now(),
        last_recalled_at: None,
        tags: Vec::new(),
        entities: Vec::new(),
        metadata: serde_json::json!({}),
    };

    db.save_memory(&mem)?;
    println!("✓ Saved memory [{}]: '{}' (ID: {})", parsed_type, summary, mem.id);
    Ok(())
}

pub fn archive(id: &str) -> Result<()> {
    let config = Config::default_config();
    let db = Database::open(&config.db_path)?;
    if db.archive_memory(id)? {
        println!("✓ Archived memory: {}", id);
    } else {
        println!("✗ Memory {} not found or already archived.", id);
    }
    Ok(())
}
