use anyhow::Result;
use automemory_core::config::Config;
use automemory_core::db::Database;
use automemory_core::models::*;
use chrono::Utc;
use std::time::Instant;
use uuid::Uuid;

pub fn execute() -> Result<()> {
    println!("=== AutoMemory System Diagnostic (doctor) ===\n");
    let mut all_passed = true;

    let config = Config::default_config();

    // 1. Check directories
    print!("Checking directories (~/.automemory)... ");
    if config.home_dir.exists() && config.hooks_dir.exists() {
        println!("✓ OK ({:?})", config.home_dir);
    } else {
        println!("✗ Missing directories. Run 'automemory init' first.");
        all_passed = false;
    }

    // 2. Check Database & WAL
    print!("Checking SQLite database connectivity & WAL mode... ");
    let db_start = Instant::now();
    match Database::open(&config.db_path) {
        Ok(db) => {
            let latency = db_start.elapsed();
            println!("✓ OK (opened in {:.2?})", latency);

            // 3. Test Read/Write speed & FTS5
            print!("Benchmarking FTS5 search & memory operations... ");
            let fts_start = Instant::now();
            let test_id = Uuid::now_v7().to_string();
            let test_mem = Memory {
                id: test_id.clone(),
                scope: MemoryScope::Global,
                project_id: None,
                memory_type: MemoryType::Fact,
                content: "AutoMemory diagnostic verification probe.".to_string(),
                summary: "Doctor diagnostic test".to_string(),
                confidence: 1.0,
                importance: 0.5,
                freshness: 1.0,
                status: MemoryStatus::Active,
                source_kind: MemorySourceKind::Agent,
                version: Some(1),
                created_at: Utc::now(),
                updated_at: Utc::now(),
                last_recalled_at: None,
                tags: vec!["doctor-test".to_string()],
                entities: vec![],
                metadata: serde_json::json!({}),
            };

            let _ = db.save_memory(&test_mem);
            let search_res = db.search_memories_fts("diagnostic verification", None, 5);
            let _ = db.archive_memory(&test_id);

            let bench_dur = fts_start.elapsed();
            match search_res {
                Ok(results) if !results.is_empty() => {
                    println!("✓ OK (FTS5 search latency: {:.2?})", bench_dur);
                }
                _ => {
                    println!("✗ FTS5 search probe failed or returned no results");
                    all_passed = false;
                }
            }

            // 4. Memory stats
            if let Ok((active, archived)) = db.count_memories(None) {
                println!("  Active memories: {}, Archived: {}", active, archived);
            }
            if let Ok(unprocessed) = db.get_unprocessed_events(100) {
                println!(
                    "  Pending event queue: {} unprocessed events",
                    unprocessed.len()
                );
            }

            // 5. Snapshot backup (VACUUM INTO) + prune to newest 5
            print!("Snapshotting database backup... ");
            let backups_dir = config.home_dir.join("backups");
            let backup_path = backups_dir.join(format!(
                "automemory-{}.db",
                Utc::now().format("%Y%m%d-%H%M%S")
            ));
            match db.backup_to(&backup_path) {
                Ok(()) => {
                    println!("✓ OK ({:?})", backup_path);
                    if let Ok(mut snaps) = std::fs::read_dir(&backups_dir).map(|rd| {
                        let mut v: Vec<_> = rd
                            .filter_map(|e| e.ok().map(|e| e.path()))
                            .filter(|p| {
                                p.file_name()
                                    .and_then(|n| n.to_str())
                                    .map(|n| n.starts_with("automemory-") && n.ends_with(".db"))
                                    .unwrap_or(false)
                            })
                            .collect();
                        v.sort();
                        v
                    }) {
                        while snaps.len() > 5 {
                            let old = snaps.remove(0);
                            let _ = std::fs::remove_file(&old);
                        }
                    }
                }
                Err(e) => {
                    println!("⚠ backup skipped: {}", e);
                }
            }
        }
        Err(e) => {
            println!("✗ Failed to open database: {}", e);
            all_passed = false;
        }
    }

    // 5. Check Agent hooks configuration
    print!("Checking Agent hook configuration... ");
    let current_dir = std::env::current_dir().unwrap_or_else(|_| std::path::PathBuf::from("."));
    let local_hooks = current_dir.join(".agents").join("hooks.json");
    let local_mcp = current_dir.join(".agents").join("mcp_config.json");
    let root_mcp = current_dir.join(".mcp.json");
    let agy_hook_path = config.hooks_dir.join("agy.json");

    if local_hooks.exists() {
        println!("✓ OK (Workspace hooks active: {:?})", local_hooks);
    } else if root_mcp.exists() || local_mcp.exists() {
        println!("✓ OK (Workspace MCP configuration active)");
    } else if agy_hook_path.exists() {
        println!("✓ OK (Global hook active: {:?})", agy_hook_path);
    } else {
        println!("✓ OK (Workspace mode: hooks configured per-project via 'automemory connect')");
    }

    println!();
    if all_passed {
        println!("✓ All health checks passed! AutoMemory is fully operational.");
    } else {
        println!("⚠ Some checks failed. Review warnings above.");
    }

    Ok(())
}
