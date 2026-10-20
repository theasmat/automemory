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
