use anyhow::Result;
use automemory_core::config::Config;
use automemory_core::db::Database;

/// Pins a memory record as an immutable Tier 1 invariant.
pub fn pin_memory(id: &str) -> Result<()> {
    let config = Config::default_config();
    let db = Database::open(&config.db_path)?;

    if db.pin_memory(id)? {
        println!("✓ Memory '{}' pinned as immutable Tier 1 invariant.", id);
        println!("  It cannot be overridden by unverified agent claims.");
    } else {
        println!("❌ Memory with ID '{}' not found.", id);
    }

    Ok(())
}

/// Vetoes and rejects a candidate memory or marks an active memory invalid.
pub fn veto_memory(id: &str, reason: Option<&str>) -> Result<()> {
    let config = Config::default_config();
    let db = Database::open(&config.db_path)?;

    if db.veto_memory(id, reason)? {
        println!("✓ Memory '{}' vetoed and marked invalid.", id);
        if let Some(r) = reason {
            println!("  Reason: {}", r);
        }
    } else {
        println!("❌ Memory with ID '{}' not found.", id);
    }

    Ok(())
}

/// Revokes memories produced by an agent or session.
pub fn revoke_memories(
    id: Option<&str>,
    session_id: Option<&str>,
    agent: Option<&str>,
) -> Result<()> {
    let config = Config::default_config();
    let db = Database::open(&config.db_path)?;

    let count = db.revoke_memories(id, session_id, agent)?;

    if count > 0 {
        println!(
            "✓ Successfully revoked and archived {} memory record(s).",
            count
        );
        if let Some(sess) = session_id {
            println!("  Session filter: {}", sess);
        }
        if let Some(ag) = agent {
            println!("  Agent filter:   {}", ag);
        }
        if let Some(mid) = id {
            println!("  Memory ID:      {}", mid);
        }
    } else {
        println!("ℹ️  No active memories found matching revocation criteria.");
    }

    Ok(())
}
