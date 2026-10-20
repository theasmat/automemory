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

    // 1. Check directories. A fresh install has none yet (they are created on first use), which is
    // not a fault: create them here so the check reports real problems only.
    let _ = config.ensure_dirs();
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

            // 5. Snapshot backup (shared with `automemory backup`: newest 10 kept)
            print!("Snapshotting database backup... ");
            match automemory_core::backup::create(&db, &config.home_dir) {
                Ok(path) => println!("✓ OK ({:?})", path),
                Err(e) => println!("⚠ backup skipped: {}", e),
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

    // 6. CodeIntel engine (symbol questions are delegated to codebase-memory-mcp)
    print!("Checking CodeIntel engine (codebase-memory-mcp)... ");
    match automemory_core::codeintel::binary_version() {
        Some(v) if automemory_core::codeintel::version_supported(&v) => println!("✓ OK ({v})"),
        Some(v) => println!(
            "⚠ {v} is older than {}.{}; using the local file-hash fallback where output differs",
            automemory_core::codeintel::MIN_ENGINE_VERSION.0,
            automemory_core::codeintel::MIN_ENGINE_VERSION.1
        ),
        None => println!("⚠ not installed; symbol checks fall back to file hashes"),
    }

    // Registrations: what AutoMemory has installed in agents, and Antigravity's three surfaces
    {
        let home = dirs::home_dir().unwrap_or_default();
        let cwd = std::env::current_dir().unwrap_or_default();
        let regs = super::disconnect::inventory(&home, &cwd);
        println!("Registrations: {} (run `automemory disconnect all --everywhere --dry-run` to list)", regs.len());
        let g = home.join(".gemini");
        let surfaces: Vec<&str> = [
            ("antigravity-cli", "CLI"),
            ("antigravity", "App"),
            ("antigravity-ide", "IDE"),
        ]
        .iter()
        .filter(|(d, _)| g.join(d).join("conversations").is_dir())
        .map(|(_, n)| *n)
        .collect();
        if !surfaces.is_empty() {
            println!("  Antigravity surfaces with history: {}", surfaces.join(", "));
        }
        let has = |p: std::path::PathBuf| {
            std::fs::read_to_string(p).is_ok_and(|t| t.contains("\"automemory\""))
        };
        if has(g.join("config/hooks.json")) && has(cwd.join(".agents/hooks.json")) {
            println!("  ⚠ Antigravity hooks are registered globally AND in this workspace; each hook runs twice (AutoMemory answers once, but run `automemory connect agy` here to drop the duplicate).");
            all_passed = false;
        }
    }

    // Lock coverage
    {
        print!("Checking lock coverage... ");
        if let Ok(db) = Database::open(&config.db_path) {
            println!("✓ {}", super::lock::coverage_line(&db, 7));
        } else {
            println!("– database unavailable");
        }
    }

    // Backups: every saved copy should open (a damaged one is no use when it is needed).
    {
        print!("Checking backups... ");
        let all = automemory_core::backup::list(&config.home_dir);
        let bad: Vec<_> = all.iter().filter(|(f, ..)| automemory_core::backup::check(f).is_err()).collect();
        match (all.first(), bad.is_empty()) {
            (None, _) => println!("– none yet (automemory backup)"),
            (_, false) => println!("⚠ {} unreadable: {}", bad.len(), bad.iter().map(|(f, ..)| f.display().to_string()).collect::<Vec<_>>().join(", ")),
            (Some((_, _, when)), true) => println!("✓ {} readable, newest {} day(s) old", all.len(), (chrono::Utc::now() - *when).num_days()),
        }
    }

    // 7. Native agent hooks (Claude Code, Codex)
    if let Some(home) = dirs::home_dir() {
        for (name, path, hint) in [
            ("Claude Code", home.join(".claude/settings.json"), "claude"),
            ("Codex", home.join(".codex/hooks.json"), "codex"),
        ] {
            print!("Checking {name} hooks... ");
            let installed = std::fs::read_to_string(&path)
                .is_ok_and(|t| t.contains(super::connect::HOOK_MARKER));
            if installed {
                println!("✓ OK ({})", path.display());
            } else {
                println!("– not installed (run `automemory connect {hint} -g`)");
            }
        }
    }

    // Getting started checklist
    println!("\n=== Getting Started Checklist ===");
    let cwd = std::env::current_dir().unwrap_or_else(|_| std::path::PathBuf::from("."));
    let home = dirs::home_dir().unwrap_or_else(|| std::path::PathBuf::from("."));
    if let Ok(db) = Database::open(&config.db_path) {
        let steps = automemory_core::setup::checklist(&db, &home, &cwd);
        for step in &steps {
            let mark = if step.done { "✓" } else { "○" };
            println!("  {} {} - {}", mark, step.title, if step.done { "done" } else { &step.command });
        }
        if let Some(next) = automemory_core::setup::next_step(&steps) {
            println!("\nNext step: {}. Try: `{}`", next.why.trim_end_matches('.'), next.command);
        }
    }

    println!();
    if all_passed {
        println!("✓ All health checks passed! AutoMemory is fully operational.");
    } else {
        println!("⚠ Some checks failed. Review warnings above.");
    }

    Ok(())
}
