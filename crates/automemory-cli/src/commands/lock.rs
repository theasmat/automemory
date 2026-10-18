use anyhow::Result;
use automemory_core::config::Config;
use automemory_core::db::Database;
use chrono::Utc;

/// Lists all active multi-agent leases across files and symbols.
pub fn list_locks(project_arg: Option<&str>) -> Result<()> {
    let config = Config::default_config();
    let db = Database::open(&config.db_path)?;

    let project_id = project_arg.and_then(|p| {
        db.get_project_by_slug(p)
            .ok()
            .flatten()
            .map(|proj| proj.id)
            .or_else(|| Some(p.to_string()))
    });

    let leases = db.list_active_leases(project_id.as_deref())?;

    if leases.is_empty() {
        println!("ℹ️  No active multi-agent locks or leases found.");
        println!("   All codebase files and symbols are free for concurrent editing.");
        return Ok(());
    }

    let now = Utc::now();
    println!("🔒 Active Multi-Agent Leases ({} active):", leases.len());
    println!("{:-<105}", "");
    println!(
        "{:<32} {:<8} {:<15} {:<12} {:<10} Goal",
        "Scope", "Type", "Agent", "Session", "TTL Left"
    );
    println!("{:-<105}", "");

    for l in leases {
        let remaining = format!("{}s", l.remaining_seconds(now));
        let session_display = if l.session_id.len() > 10 {
            format!("{}..", &l.session_id[..8])
        } else {
            l.session_id.clone()
        };
        let scope_display = if l.scope.len() > 30 {
            format!("{}..", &l.scope[..28])
        } else {
            l.scope.clone()
        };

        println!(
            "{:<32} {:<8} {:<15} {:<12} {:<10} {}",
            scope_display,
            l.scope_type.to_string(),
            l.agent,
            session_display,
            remaining,
            l.goal
        );
    }
    println!("{:-<105}", "");
    println!("💡 Use 'automemory unlock <scope>' to release a lease manually.");

    Ok(())
}

/// Kill switch: releases every active lease regardless of holder.
pub fn stop_all(project_arg: Option<&str>) -> Result<()> {
    let config = Config::default_config();
    let db = Database::open(&config.db_path)?;

    let project_id = project_arg.and_then(|p| {
        db.get_project_by_slug(p)
            .ok()
            .flatten()
            .map(|proj| proj.id)
            .or_else(|| Some(p.to_string()))
    });

    let count = db.release_all_leases(project_id.as_deref())?;
    println!("■ Stopped: released {count} active lease(s). Multi-agent work is halted.");
    Ok(())
}

/// Releases or overrides an active resource lease.
pub fn unlock(scope: &str, project_arg: Option<&str>, _force: bool) -> Result<()> {
    let config = Config::default_config();
    let db = Database::open(&config.db_path)?;

    let project_id = project_arg.and_then(|p| {
        db.get_project_by_slug(p)
            .ok()
            .flatten()
            .map(|proj| proj.id)
            .or_else(|| Some(p.to_string()))
    });

    let count = db.unlock_scope(project_id.as_deref(), scope)?;

    if count > 0 {
        println!(
            "✓ Successfully released {} lease(s) for scope '{}'.",
            count, scope
        );
    } else {
        println!("ℹ️  No active lease found matching scope '{}'.", scope);
    }

    Ok(())
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
