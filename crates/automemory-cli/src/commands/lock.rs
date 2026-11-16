use anyhow::Result;
use automemory_core::config::{Config, GlobalConfig};
use automemory_core::db::Database;

fn open() -> Result<Database> {
    Database::open(&Config::default_config().db_path)
}

fn project_id_of(db: &Database, project_arg: Option<&str>) -> Option<String> {
    project_arg.map(|p| db.get_project_by_slug(p).ok().flatten().map(|proj| proj.id).unwrap_or_else(|| p.to_string()))
}

/// Lists the files agents hold. The locks are Tracewise's (docs/specs/tracewise-companion.md); without it there are none.
pub fn list_locks(project_arg: Option<&str>) -> Result<()> {
    let db = open()?;
    let settings = GlobalConfig::load().unwrap_or_default().companion;
    let pid = project_id_of(&db, project_arg);
    match automemory_core::companion::held_locks(&settings, &db, pid.as_deref()) {
        Err(msg) => println!("ℹ️  {msg}"),
        Ok(l) if l.is_empty() => println!("ℹ️  No file is locked right now (locks come from Tracewise)."),
        Ok(l) => {
            println!("🔒 Files held right now ({}), from Tracewise:", l.len());
            println!("{:<44} {:<12} {:<12} {:<8} Why", "File", "Agent", "Session", "Left");
            for x in l {
                let sess: String = x.session_id.chars().take(10).collect();
                println!("{:<44} {:<12} {:<12} {:<8} {}", x.scope, x.agent, sess, format!("{}s", x.remaining_seconds), x.goal);
            }
            println!("💡 `automemory unlock <file>` asks Tracewise to release one.");
        }
    }
    Ok(())
}

/// Asks Tracewise to release one file (or every lock with `all`). Run by a person, so it may release another actor's lock.
fn release(args: &[&str], project_arg: Option<&str>) -> Result<()> {
    let db = open()?;
    let settings = GlobalConfig::load().unwrap_or_default().companion;
    let pid = project_id_of(&db, project_arg);
    for dir in automemory_core::companion::project_dirs(&db, pid.as_deref()) {
        match automemory_core::companion::json_in(&settings, &db, &dir, args) {
            Ok(v) => println!("✓ {}: {v}", dir.display()),
            Err(msg) => println!("ℹ️  {msg}"),
        }
    }
    Ok(())
}

pub fn unlock(scope: &str, project_arg: Option<&str>, _force: bool) -> Result<()> {
    release(&["lock", "release", scope, "--json"], project_arg)
}

/// Kill switch: releases every lock Tracewise holds for the project.
pub fn stop_all(project_arg: Option<&str>) -> Result<()> {
    release(&["lock", "release", "--all", "--json"], project_arg)
}

/// `automemory overlap`: exit code 1 when another branch edits the same files.
pub fn overlap(base: &str, branch: Option<&str>) -> Result<()> {
    let repo = std::env::current_dir()?;
    let current = match branch {
        Some(b) => b.to_string(),
        None => automemory_core::overlap::current_branch(&repo)?,
    };
    let hits = automemory_core::overlap::overlapping_branches(&repo, base, &current)?;
    if hits.is_empty() {
        println!("✓ No other branch changes the same files as '{current}' (vs {base}).");
        return Ok(());
    }
    println!("⚠ '{current}' shares changed files with other branches (vs {base}):");
    for h in &hits {
        println!("  {}", h.branch);
        for f in h.files.iter().take(10) {
            println!("    - {f}");
        }
        if h.files.len() > 10 {
            println!("    ... and {} more", h.files.len() - 10);
        }
    }
    println!("Merge one first and rebase the other, or coordinate with the agent on that branch.");
    std::process::exit(1);
}

/// `automemory locks --coverage`: how much of the file capture is in place, from `tracewise doctor --json`.
pub fn coverage(_days: i64) -> Result<()> {
    let db = open()?;
    let settings = GlobalConfig::load().unwrap_or_default().companion;
    let dir = std::env::current_dir()?;
    match automemory_core::companion::json_in(&settings, &db, &dir, &["doctor", "--json"]) {
        Ok(v) => {
            println!("Capture coverage (Tracewise): {}", v["coverage"]);
            println!("Who made the changes:          {}", v["attribution"]);
        }
        Err(msg) => println!("ℹ️  {msg}"),
    }
    Ok(())
}
