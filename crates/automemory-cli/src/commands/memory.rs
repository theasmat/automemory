use anyhow::Result;
use automemory_core::config::Config;
use automemory_core::db::Database;
use automemory_core::models::*;
use chrono::Utc;
use std::str::FromStr;
use uuid::Uuid;

pub fn list(project: Option<String>, limit: usize, group_by: Option<String>) -> Result<()> {
    let config = Config::default_config();
    let db = Database::open(&config.db_path)?;
    let memories = db.list_memories(project.as_deref(), Some(MemoryStatus::Active), None, limit)?;

    if memories.is_empty() {
        println!("No active memories found.");
        return Ok(());
    }

    if let Some(group_field) = group_by.as_deref() {
        match group_field.to_lowercase().as_str() {
            "type" | "category" => {
                let mut grouped: std::collections::BTreeMap<String, Vec<&Memory>> =
                    std::collections::BTreeMap::new();
                for m in &memories {
                    grouped
                        .entry(m.memory_type.to_string())
                        .or_default()
                        .push(m);
                }
                for (mtype, mems) in grouped {
                    println!("\n🏷️  [{}] ({} memories)", mtype.to_uppercase(), mems.len());
                    println!("{}", "─".repeat(95));
                    for m in mems {
                        println!("  • [{}] {} (v{})", m.id, m.summary, m.version.unwrap_or(1));
                    }
                }
                return Ok(());
            }
            "scope" => {
                let mut grouped: std::collections::BTreeMap<String, Vec<&Memory>> =
                    std::collections::BTreeMap::new();
                for m in &memories {
                    grouped.entry(m.scope.to_string()).or_default().push(m);
                }
                for (scope, mems) in grouped {
                    println!("\n🌐 [{}] ({} memories)", scope.to_uppercase(), mems.len());
                    println!("{}", "─".repeat(95));
                    for m in mems {
                        println!("  • [{:<10}] [{}] {}", m.memory_type, m.id, m.summary);
                    }
                }
                return Ok(());
            }
            other => {
                eprintln!("Unknown group-by option '{}'. Supported options: 'type', 'scope'. Falling back to table view.", other);
            }
        }
    }

    println!(
        "{:<38} {:<12} {:<8} {:<10} {:<40}",
        "ID", "TYPE", "SCOPE", "CONFIDENCE", "SUMMARY"
    );
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
        let vector_engine = automemory_core::retrieval::vector::VectorEngine;
        let query_vector = vector_engine.embed_text(query)?;

        if let Some(ref endpoint) = global_cfg.cloud.endpoint {
            let search_url = format!("{}/api/v1/vector/search", endpoint.trim_end_matches('/'));
            println!(
                "🔍 Querying Edge Vector Index (Zero-Knowledge): {}",
                search_url
            );

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
                                let master_key =
                                    automemory_core::crypto::BlindCrypto::get_or_create_master_key(
                                        None,
                                    )?;
                                println!(
                                    "Found {} semantic vector matches for '{}':",
                                    matches.len(),
                                    query
                                );
                                println!("{}", "-".repeat(90));

                                for m_val in matches {
                                    let wire_res: Result<
                                        automemory_core::crypto::BlindMemoryWire,
                                        _,
                                    > = serde_json::from_value(m_val.clone());
                                    let score =
                                        m_val.get("score").and_then(|s| s.as_f64()).unwrap_or(0.0);
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
            } else {
                println!("ℹ Edge endpoint unreachable; falling back to local zero-knowledge vector similarity.");
            }
        }

        // Local Zero-Knowledge Semantic Vector Search Fallback
        let active_mems = db.list_memories(
            project.as_deref(),
            Some(automemory_core::models::MemoryStatus::Active),
            None,
            1000,
        )?;
        let mut scored_mems = Vec::new();
        for mem in active_mems {
            let mem_vec = vector_engine.embed_text(&format!("{} {}", mem.summary, mem.content))?;
            let sim = automemory_core::retrieval::vector::VectorEngine::cosine_similarity(
                &query_vector,
                &mem_vec,
            );
            if sim > 0.05 {
                scored_mems.push((mem, sim));
            }
        }
        scored_mems.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));
        if !scored_mems.is_empty() {
            println!(
                "Found {} semantic vector matches for '{}':",
                scored_mems.len().min(limit),
                query
            );
            println!("{}", "-".repeat(90));
            for (m, sim) in scored_mems.into_iter().take(limit) {
                println!(
                    "[{}] {} (Semantic Similarity: {:.3})",
                    m.memory_type, m.summary, sim
                );
                println!("  ID: {}", m.id);
                println!("  Content: {}", m.content);
                println!();
            }
            return Ok(());
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

pub fn get(id: &str) -> Result<()> {
    let config = Config::default_config();
    let db = Database::open(&config.db_path)?;
    if let Some(mem) = db.get_memory_by_id(id)? {
        println!("=== Memory Record: {} ===", mem.id);
        println!("Summary:     {}", mem.summary);
        println!("Type:        {}", mem.memory_type);
        println!("Scope:       {}", mem.scope);
        if let Some(ref pid) = mem.project_id {
            println!("Project:     {}", pid);
        }
        println!("Confidence:  {:.2}", mem.confidence);
        println!("Version:     {}", mem.version.unwrap_or(1));
        println!("Status:      {}", mem.status);
        println!("Created At:  {}", mem.created_at.to_rfc3339());
        println!("Updated At:  {}", mem.updated_at.to_rfc3339());
        if !mem.tags.is_empty() {
            println!("Tags:        {}", mem.tags.join(", "));
        }
        if !mem.metadata.is_null() && mem.metadata != serde_json::json!({}) {
            if let Ok(meta_str) = serde_json::to_string_pretty(&mem.metadata) {
                println!("Metadata:\n{}", meta_str);
            }
        }
        println!("\nContent:\n{}", mem.content);
    } else {
        println!("✗ Memory record not found: {}", id);
    }
    Ok(())
}

pub fn digest(span_str: Option<String>, project: Option<String>, save: bool) -> Result<()> {
    let config = Config::default_config();
    let db = Database::open(&config.db_path)?;
    let span_arg = span_str.unwrap_or_else(|| "1h".to_string());

    let (start_time, end_time) = automemory_core::models::timespan::parse_time_span_str(&span_arg)?;
    let engine = automemory_core::extraction::TimeSpanDigestEngine::new(&db);
    let digest = engine.generate_digest(project.as_deref(), start_time, end_time)?;

    let proj_label = project.as_deref().unwrap_or("All Projects / Global");
    println!(
        "⏱️ Time Span: {} ({:.1} min, {} → {}) | Project: {}",
        span_arg,
        digest.duration_minutes,
        digest.start_time.format("%H:%M:%S"),
        digest.end_time.format("%H:%M:%S"),
        proj_label
    );
    println!("{}", "─".repeat(80));

    println!("🛠️ Activity & Velocity:");
    println!(
        "  • {} tool calls ({} edits, {} reads, {} commands)",
        digest.metrics.total_tool_calls,
        digest.metrics.edits_count,
        digest.metrics.reads_count,
        digest.metrics.commands_count
    );
    if !digest.active_agents.is_empty() {
        println!("  • Active Agents: {}", digest.active_agents.join(", "));
    }
    if !digest.files_modified.is_empty() {
        println!("  • Files Modified ({}):", digest.files_modified.len());
        for f in &digest.files_modified {
            println!("    - {}", f);
        }
    }
    if !digest.files_inspected.is_empty() {
        println!("  • Files Inspected ({}):", digest.files_inspected.len());
        for f in digest.files_inspected.iter().take(5) {
            println!("    - {}", f);
        }
        if digest.files_inspected.len() > 5 {
            println!("    ... and {} more", digest.files_inspected.len() - 5);
        }
    }

    if digest.metrics.tests_run > 0 {
        println!();
        println!("🧪 Tests & Verifications:");
        println!(
            "  • {} passed, {} failed ({} total runs)",
            digest.metrics.tests_passed, digest.metrics.tests_failed, digest.metrics.tests_run
        );
    }

    if !digest.achievements.is_empty() {
        println!();
        println!("🏆 Achievements:");
        for ach in &digest.achievements {
            println!("  ✓ {}", ach);
        }
    }

    if !digest.failures.is_empty() {
        println!();
        println!("⚠️ Failures & Blockers:");
        for f in &digest.failures {
            if f.is_resolved {
                println!(
                    "  ✓ [Resolved] {}: {}",
                    f.tool_or_command,
                    f.fix_applied.as_deref().unwrap_or("resolved")
                );
            } else {
                println!("  ✗ [Open] {}: {}", f.tool_or_command, f.error_snippet);
            }
        }
    }

    if save {
        let mem = engine.synthesize_milestone_memory(&digest, None)?;
        db.save_memory(&mem)?;
        println!();
        println!(
            "💾 Saved checkpoint memory: '{}' (ID: {})",
            mem.summary, mem.id
        );
    }

    Ok(())
}

#[derive(Debug, Default)]
pub struct MemorySaveArgs {
    pub content: String,
    pub summary: String,
    pub mem_type: Option<String>,
    pub scope: Option<String>,
    pub project_id: Option<String>,
    pub category: Option<String>,
    pub achieved: Vec<String>,
    pub failed: Vec<String>,
    pub files: Option<String>,
}

pub fn save(args: MemorySaveArgs) -> Result<()> {
    let config = Config::default_config();
    let db = Database::open(&config.db_path)?;

    let parsed_type = args
        .mem_type
        .as_deref()
        .and_then(|s| MemoryType::from_str(s).ok())
        .unwrap_or(MemoryType::Fact);

    let parsed_scope = args
        .scope
        .as_deref()
        .and_then(|s| MemoryScope::from_str(s).ok())
        .unwrap_or(MemoryScope::Project);

    let mut metadata_map = serde_json::Map::new();
    if let Some(cat) = args.category {
        metadata_map.insert("category".to_string(), serde_json::json!(cat));
    }
    if !args.achieved.is_empty() {
        metadata_map.insert("achievements".to_string(), serde_json::json!(args.achieved));
    }
    if !args.failed.is_empty() {
        metadata_map.insert(
            "failures_resolved".to_string(),
            serde_json::json!(args.failed),
        );
    }
    if let Some(ref fls) = args.files {
        let fls_vec: Vec<String> = fls
            .split(',')
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty())
            .collect();
        metadata_map.insert("files_modified".to_string(), serde_json::json!(fls_vec));
    }

    let mem = Memory {
        id: Uuid::now_v7().to_string(),
        scope: parsed_scope,
        project_id: args.project_id,
        memory_type: parsed_type,
        content: args.content,
        summary: args.summary.clone(),
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
        metadata: serde_json::Value::Object(metadata_map),
    };

    db.save_memory(&mem)?;
    println!(
        "✓ Saved memory [{}]: '{}' (ID: {})",
        parsed_type, args.summary, mem.id
    );
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
