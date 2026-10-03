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

pub async fn search(
    query: &str,
    project: Option<String>,
    limit: usize,
    semantic: bool,
) -> Result<()> {
    let config = Config::default_config();
    let db = Database::open(&config.db_path)?;

    if semantic {
        let global_cfg = automemory_core::config::GlobalConfig::load().unwrap_or_default();
        let vector_engine = automemory_core::retrieval::vector::VectorEngine::default();
        let query_vector = vector_engine.embed_text(query)?;

        if let Some(ref endpoint) = global_cfg.cloud.endpoint {
            let search_url = format!("{}/api/v1/vector/search", endpoint.trim_end_matches('/'));
            println!("🔍 Querying Edge Vector Index (Zero-Knowledge): {}", search_url);

            let client = reqwest::Client::builder()
                .timeout(std::time::Duration::from_secs(10))
                .build()?;

            let mut req = client.post(&search_url).json(&serde_json::json!({
                "vector": query_vector,
                "blind_project_id": project.as_deref(),
                "top_k": limit,
            }));

            if let Some(ref tok) = global_cfg.cloud.auth_token {
                req = req.header("Authorization", format!("Bearer {}", tok));
            }

            if let Ok(resp) = req.send().await {
                if resp.status().is_success() {
                    if let Ok(body) = resp.json::<serde_json::Value>().await {
                        if let Some(matches) = body.get("matches").and_then(|m| m.as_array()) {
                            if !matches.is_empty() {
                                let master_key = automemory_core::crypto::BlindCrypto::get_or_create_master_key(None)?;
                                println!("Found {} semantic vector matches for '{}':", matches.len(), query);
                                println!("{}", "-".repeat(90));

                                for m_val in matches {
                                    let wire_res: Result<automemory_core::crypto::BlindMemoryWire, _> =
                                        serde_json::from_value(m_val.clone());
                                    let score = m_val.get("score").and_then(|s| s.as_f64()).unwrap_or(0.0);
                                    if let Ok(wire) = wire_res {
                                        match automemory_core::crypto::BlindCrypto::decrypt_memory(
                                            &master_key,
                                            &wire,
                                            project.as_deref(),
                                        ) {
                                            Ok(mem) => {
                                                println!(
                                                    "[{}] {} (Semantic Similarity: {:.3})",
                                                    mem.memory_type, mem.summary, score
                                                );
                                                println!("  ID: {}", mem.id);
                                                println!("  Content: {}", mem.content);
                                                println!("  [Decrypted locally via AES-256-GCM Master Key]");
                                                println!();
                                            }
                                            Err(e) => {
                                                println!(
                                                    "[ENCRYPTED MATCH] ID: {} (Score: {:.3}, Decrypt err: {})",
                                                    wire.id, score, e
                                                );
                                            }
                                        }
                                    }
                                }
                                return Ok(());
                            }
                        }
                    }
                }
            }
        }
    }

    // Default or Fallback: Local FTS5 lexical search
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
