//! Guided learning, onboarding next step, interactive tour, and demo data.

use anyhow::Result;
use automemory_core::config::Config;
use automemory_core::db::Database;
use automemory_core::setup;
use std::path::PathBuf;

/// Prints the single best next onboarding step and command.
pub fn next() -> Result<()> {
    let cwd = std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."));
    let home = dirs::home_dir().unwrap_or_else(|| PathBuf::from("."));
    let config = Config::default_config();
    let db = Database::open(&config.db_path)?;
    let steps = setup::checklist(&db, &home, &cwd);
    if let Some(step) = setup::next_step(&steps) {
        let reason = step.why.trim_end_matches('.');
        println!("{}. Try: `{}`", reason, step.command);
    } else {
        println!("All onboarding checklist steps are complete!");
    }
    Ok(())
}
