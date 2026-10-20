//! `automemory stats`: what AutoMemory did for you lately, from rows it already keeps.
use anyhow::Result;
use automemory_core::config::Config;
use automemory_core::db::Database;

pub fn run(days: i64, json: bool) -> Result<()> {
    let report = automemory_core::report::value_report(&Database::open(&Config::default_config().db_path)?, days.clamp(1, 365));
    println!("{}", if json { serde_json::to_string_pretty(&report)? } else { automemory_core::report::summary(&report) });
    Ok(())
}
