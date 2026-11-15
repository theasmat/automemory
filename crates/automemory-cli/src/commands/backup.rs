//! `automemory backup`: save, list and restore copies of the database.
use anyhow::{bail, Result};
use automemory_core::backup;
use automemory_core::config::Config;
use automemory_core::db::Database;
use std::io::{IsTerminal, Write};
use std::path::PathBuf;

pub fn create() -> Result<()> {
    let config = Config::default_config();
    let db = Database::open(&config.db_path)?;
    let file = backup::create(&db, &config.home_dir)?;
    println!("✓ Saved {} ({} active notes). The newest 10 are kept.", file.display(), backup::check(&file)?);
    Ok(())
}

pub fn list() -> Result<()> {
    let home = Config::default_config().home_dir;
    let all = backup::list(&home);
    if all.is_empty() {
        println!("No backups yet. Make one: automemory backup");
    }
    for (file, size, when) in all {
        let state = match backup::check(&file) {
            Ok(n) => format!("{n} active notes"),
            Err(_) => "UNREADABLE".to_string(),
        };
        println!("{}  {:>8} KB  {}  {}", when.format("%Y-%m-%d %H:%M"), size / 1024, state, file.display());
    }
    Ok(())
}

/// Shows what would change, asks, saves the current database as `before-restore-*`, then swaps.
pub fn restore(file: &str, yes: bool) -> Result<()> {
    let config = Config::default_config();
    let path = PathBuf::from(file);
    let theirs = backup::check(&path)?;
    let ours = backup::check(&config.db_path).unwrap_or(0);
    println!("Backup {}: {theirs} active notes. Current database: {ours}.", path.display());
    if !yes {
        if !std::io::stdin().is_terminal() {
            bail!("not a terminal: pass --yes to restore without asking");
        }
        print!("Replace the current database with this backup? The current one is kept as before-restore-*. [y/N] ");
        std::io::stdout().flush()?;
        let mut answer = String::new();
        std::io::stdin().read_line(&mut answer)?;
        if !answer.trim().eq_ignore_ascii_case("y") {
            println!("Nothing changed.");
            return Ok(());
        }
    }
    let saved = backup::restore(&config.db_path, &path, &config.home_dir)?;
    println!("✓ Restored. The previous database is at {}. Restart `automemory serve` and your agents.", saved.display());
    Ok(())
}

/// Restore drill: proves the newest backup opens and shows what it holds, without touching the live database.
pub fn drill() -> Result<()> {
    let r = backup::drill(&Config::default_config().home_dir)?;
    println!("✓ Restore drill passed on {}: {} notes, {} decisions, {} events (schema {}).", r.file.display(), r.notes, r.decisions, r.events, r.schema_version);
    Ok(())
}

/// `automemory export [--out file] [--markdown dir] [--project p]`: notes and decisions as versioned JSON, and optionally a Markdown folder.
pub fn export(out: Option<PathBuf>, markdown: Option<PathBuf>, project: Option<String>) -> Result<()> {
    let config = Config::default_config();
    let db = Database::open(&config.db_path)?;
    let all = automemory_core::portable::export(&db, project.as_deref())?;
    let json = serde_json::to_string_pretty(&all)?;
    match out {
        Some(file) => {
            std::fs::write(&file, json)?;
            println!("✓ {} notes and {} decisions written to {}", all.memories.len(), all.decisions.len(), file.display());
        }
        None if markdown.is_none() => println!("{json}"),
        None => {}
    }
    if let Some(dir) = markdown {
        let n = automemory_core::portable::write_markdown(&all, &dir)?;
        println!("✓ {n} Markdown files under {}", dir.display());
    }
    Ok(())
}

/// `automemory import <file>`: adds notes and decisions the database does not have. Never overwrites; asks for the human password.
pub fn import(file: PathBuf) -> Result<()> {
    automemory_core::auth::require_human(automemory_core::auth::stored_password_hash().as_deref(), "import notes and decisions from a file")?;
    let all: automemory_core::portable::Export = serde_json::from_str(&std::fs::read_to_string(&file)?)?;
    let db = Database::open(&Config::default_config().db_path)?;
    let r = automemory_core::portable::import(&db, &all)?;
    println!(
        "✓ Added {} notes and {} decisions; {} notes and {} decisions were already here. Imported notes keep the trust tier the file records, so import only files you trust.",
        r.memories_added, r.decisions_added, r.memories_skipped, r.decisions_skipped
    );
    Ok(())
}

/// `automemory reset`: deletes everything AutoMemory has stored (all projects). Needs the human password; with none set it refuses,
/// because a destructive command must not be open to an agent. Keeps config.toml (and so the password), the device id and the agent registrations.
pub fn reset(include_backups: bool, yes: bool) -> Result<()> {
    use automemory_core::reset;
    let Some(hash) = automemory_core::auth::stored_password_hash() else {
        bail!("`automemory reset` needs a human password, and none is set. Set one with `automemory rule passwd`, then run it again.");
    };
    automemory_core::auth::require_human(Some(hash.as_str()), "reset all AutoMemory data")?;
    let config = Config::default_config();
    let backups = backup::dir(&config.home_dir);
    let inv = reset::inventory(&config.db_path, &backups);
    println!("This deletes ALL AutoMemory data for every project:");
    println!("  {} notes, {} decisions, {} projects, the local sync vault", inv.notes, inv.decisions, inv.projects);
    println!("  {} saved backups: {}", inv.backups, if include_backups { "DELETED too (--include-backups)" } else { "kept" });
    println!("Kept: settings and your password, the device id, the agents `connect` registered. Code and your repositories are never touched.");
    if !yes {
        if !std::io::stdin().is_terminal() {
            bail!("not a terminal: pass --yes to reset without asking");
        }
        print!("Type 'reset' to continue: ");
        std::io::stdout().flush()?;
        let mut answer = String::new();
        std::io::stdin().read_line(&mut answer)?;
        if answer.trim() != "reset" {
            println!("Nothing changed.");
            return Ok(());
        }
    }
    let removed = reset::wipe(&config.db_path, &config.vault_dir, &backups, include_backups)?;
    println!("✓ Reset: removed {} item(s). Restart `automemory serve` and your agents; the next command starts a fresh database.", removed.len());
    Ok(())
}

/// `automemory tidy [--dry-run]`: files what a person would want to clear out (repeated notes, notes about files that are gone, unused notes) in the Inbox.
/// Nothing is changed by it; approving an item in the Inbox is what archives.
pub fn tidy(dry_run: bool) -> Result<()> {
    let db = Database::open(&Config::default_config().db_path)?;
    let cwd = std::env::current_dir().unwrap_or_default();
    let project = db.find_project_by_path(&cwd.to_string_lossy()).ok().flatten().map(|p| p.id);
    let r = automemory_core::tidy::run(&db, project.as_deref(), dry_run)?;
    if !dry_run {
        db.sync_kv_set("tidy_at", &chrono::Utc::now().timestamp().to_string());
    }
    let scope = if project.is_some() { "this project" } else { "all projects" };
    println!("Tidy ({scope}){}:", if dry_run { ", dry run: nothing filed" } else { "" });
    println!("  {} note(s) that repeat another", r.duplicates.len());
    for (keep, drop) in r.duplicates.iter().take(5) {
        println!("    {drop} repeats {keep}");
    }
    println!("  {} note(s) about files that no longer exist", r.missing_files.len());
    for (id, files) in r.missing_files.iter().take(5) {
        println!("    {id}: {}", files.iter().take(3).cloned().collect::<Vec<_>>().join(", "));
    }
    if !dry_run {
        println!("  {} item(s) filed in the Inbox ({} of them unused notes). Review them with `automemory proposal list` or in the dashboard.", r.filed, r.unused_offered);
    }
    Ok(())
}
