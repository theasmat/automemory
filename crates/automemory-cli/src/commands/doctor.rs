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

    // A config.toml that does not parse is not "no password": every gate refuses until it is fixed.
    {
        print!("Checking config.toml... ");
        match automemory_core::config::GlobalConfig::load() {
            Ok(_) => println!("✓ readable"),
            Err(e) => {
                println!("✗ {e:#}\n    The human password cannot be checked, so actions that need you are refused until this is fixed.");
                all_passed = false;
            }
        }
    }

    // Agents keep their MCP server running for days. After an upgrade it is still the old code (found in trial T11b:
    // Antigravity's server from the day before ignored new turn rules). Say which ones to restart.
    {
        print!("Checking running MCP servers... ");
        let stale = stale_mcp_servers();
        if stale.is_empty() {
            println!("✓ none older than this binary");
        } else {
            println!("⚠ {} started before this binary was installed; restart the agent that owns each (servers from this version on replace themselves after an update):", stale.len());
            for (pid, age_hours) in stale {
                println!("    pid {pid} (running {age_hours} h), started by {}", parent_name(pid));
            }
        }
    }

    // Who each MCP server says it is: a server is configured for one agent and signs its records with that name. Flag a different program on an entry,
    // and entries from before the name existed (their records are signed by whatever the caller types).
    {
        print!("Checking agent identities... ");
        let rows: Vec<(String, String)> = automemory_core::db::Database::open(&config.db_path)
            .ok()
            .and_then(|db| {
                let conn = db.conn();
                let mut st = conn.prepare("SELECT key, value FROM sync_kv WHERE key LIKE 'mcp_client:%'").ok()?;
                let rows = st.query_map([], |r| Ok((r.get(0)?, r.get(1)?))).ok()?.filter_map(|r| r.ok()).collect();
                Some(rows)
            })
            .unwrap_or_default();
        let week_ago = chrono::Utc::now() - chrono::Duration::days(7);
        let mut problems = Vec::new();
        for (key, value) in &rows {
            let who = key.trim_start_matches("mcp_client:");
            let v: serde_json::Value = serde_json::from_str(value).unwrap_or_default();
            let recent = v["at"].as_str().and_then(|t| chrono::DateTime::parse_from_rfc3339(t).ok()).is_some_and(|t| t > week_ago);
            let client = v["client"].as_str().unwrap_or("");
            if !recent {
                continue;
            }
            if v["configured"].as_bool() != Some(true) {
                problems.push(format!("\"{client}\" connected without an agent name: run `automemory connect {who}` again so its records are signed {who}"));
            } else if automemory_mcp::canonical_agent(client) != who {
                problems.push(format!("the entry for {who} was started by \"{client}\": another program uses it, or the entry was copied; run `automemory connect <agent>` for each"));
            }
        }
        if problems.is_empty() {
            println!("✓ {}", if rows.is_empty() { "no agent connected yet".to_string() } else { format!("{} agent(s), each signed with its own name", rows.len()) });
        } else {
            println!("⚠ {} thing(s) to look at:", problems.len());
            for p in problems {
                println!("    {p}");
            }
        }
    }

    // Derived records written many times for the same thing (older versions did this on every session end).
    {
        print!("Checking duplicate records... ");
        match automemory_core::db::Database::open(&config.db_path).and_then(|db| db.duplicates()) {
            Ok(d) if d.total() == 0 => println!("✓ none"),
            Ok(d) => println!("⚠ {} duplicate skill(s), {} trace(s), {} failure(s): run `automemory memory prune --yes` (nothing else is touched)", d.skills, d.traces, d.failures),
            Err(_) => println!("– database unavailable"),
        }
    }

    // Project library: copies up to date, linked files present, the folder out of git status.
    {
        print!("Checking project library... ");
        let db = automemory_core::db::Database::open(&config.db_path).ok();
        let here = std::env::current_dir().ok();
        match (db, here) {
            (Some(db), Some(cwd)) => match db.find_project_by_path(&cwd.to_string_lossy()).ok().flatten() {
                Some(p) => {
                    let cfg = automemory_core::config::GlobalConfig::load().unwrap_or_default();
                    let problems = automemory_core::library::fs::check(&db, &p.id, &p.slug, &cwd, &cfg.library);
                    if problems.is_empty() {
                        println!("✓ {} file(s)", db.library_list(Some(&p.id), None).map(|f| f.len()).unwrap_or(0));
                    } else {
                        println!("⚠ {} thing(s) to look at:", problems.len());
                        for pr in problems {
                            println!("    {pr}");
                        }
                    }
                }
                None => println!("– this folder is not a registered project"),
            },
            _ => println!("– database unavailable"),
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

/// `automemory mcp` processes of this very binary that started before the binary file was last replaced.
/// Returns (pid, hours running). Unix only (needs `ps`); elsewhere it reports nothing.
fn stale_mcp_servers() -> Vec<(u32, u64)> {
    let Ok(exe) = std::env::current_exe() else { return vec![] };
    let Some(modified) = std::fs::metadata(&exe).ok().and_then(|m| m.modified().ok()) else { return vec![] };
    let installed_ago = std::time::SystemTime::now().duration_since(modified).map(|d| d.as_secs()).unwrap_or(0);
    let Ok(out) = std::process::Command::new("ps").args(["-axo", "pid=,etime=,command="]).output() else { return vec![] };
    String::from_utf8_lossy(&out.stdout)
        .lines()
        .filter_map(|l| {
            let mut it = l.split_whitespace();
            let (pid, etime, bin, sub) = (it.next()?, it.next()?, it.next()?, it.next()?);
            let same = std::path::Path::new(bin).canonicalize().ok().as_deref() == Some(exe.as_path()) || std::path::Path::new(bin) == exe.as_path();
            if !same || sub != "mcp" {
                return None;
            }
            let running = parse_etime(etime)?;
            // Started before the file was replaced (a few seconds of slack for clock rounding).
            if running > installed_ago + 5 { Some((pid.parse().ok()?, running / 3600)) } else { None }
        })
        .collect()
}

/// What launched a process, for the message ("Antigravity", "claude"): the parent's program name.
fn parent_name(pid: u32) -> String {
    let ps = |args: &[&str]| std::process::Command::new("ps").args(args).output().ok().map(|o| String::from_utf8_lossy(&o.stdout).trim().to_string());
    ps(&["-o", "ppid=", "-p", &pid.to_string()])
        .and_then(|ppid| ps(&["-o", "comm=", "-p", &ppid]))
        .map(|c| c.rsplit('/').next().unwrap_or(&c).to_string())
        .filter(|c| !c.is_empty())
        .unwrap_or_else(|| "an unknown program".to_string())
}

/// `ps` elapsed time: `[[dd-]hh:]mm:ss` in seconds.
fn parse_etime(s: &str) -> Option<u64> {
    let (days, rest) = s.split_once('-').map_or((0, s), |(d, r)| (d.parse().unwrap_or(0), r));
    let parts: Vec<u64> = rest.split(':').map(|p| p.parse().ok()).collect::<Option<_>>()?;
    let secs = match parts.as_slice() {
        [m, s] => m * 60 + s,
        [h, m, s] => h * 3600 + m * 60 + s,
        _ => return None,
    };
    Some(days * 86400 + secs)
}

#[cfg(test)]
mod etime_tests {
    #[test]
    fn ps_elapsed_time_parses() {
        assert_eq!(super::parse_etime("05:07"), Some(307));
        assert_eq!(super::parse_etime("03:05:07"), Some(11107));
        assert_eq!(super::parse_etime("2-03:05:07"), Some(2 * 86400 + 11107));
        assert_eq!(super::parse_etime("junk"), None);
    }
}
