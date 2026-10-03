use anyhow::{Context, Result};
use automemory_core::config::Config;
use automemory_core::db::Database;

pub fn summary(project: Option<String>) -> Result<()> {
    let config = Config::default_config();
    let db = Database::open(&config.db_path)
        .with_context(|| format!("Failed to open database at {:?}", config.db_path))?;

    let summary = db.get_proposals_summary(project.as_deref())?;

    println!("========================================================");
    println!("             AutoMemory Review Proposals Summary        ");
    println!("========================================================");
    println!("Total Proposals:  {}", summary.total);
    println!("  • Pending:      {}", summary.total_pending);
    println!("  • Approved:     {}", summary.total_approved);
    println!("  • Rejected:     {}", summary.total_rejected);
    println!("--------------------------------------------------------");

    if summary.by_type.is_empty() {
        println!("(No proposals recorded in database)");
    } else {
        println!("{:<28} {:>10} {:>10} {:>10}", "Proposal Type", "Pending", "Approved", "Rejected");
        println!("{:-<28} {:-<10} {:-<10} {:-<10}", "", "", "", "");
        for item in summary.by_type {
            println!(
                "{:<28} {:>10} {:>10} {:>10}",
                item.proposal_type, item.pending, item.approved, item.rejected
            );
        }
    }
    println!("========================================================");

    Ok(())
}

pub fn list(
    status: String,
    proposal_type: Option<String>,
    limit: usize,
    offset: usize,
    project: Option<String>,
) -> Result<()> {
    let config = Config::default_config();
    let db = Database::open(&config.db_path)
        .with_context(|| format!("Failed to open database at {:?}", config.db_path))?;

    let proposals = db.list_proposals(
        project.as_deref(),
        Some(&status),
        proposal_type.as_deref(),
        Some(limit),
        Some(offset),
    )?;

    if proposals.is_empty() {
        println!("No proposals found matching status='{}'", status);
        return Ok(());
    }

    println!("=== AutoMemory Review Proposals (Status: {}) ===", status);
    for p in proposals {
        println!(
            "• [{}] [{}] ID: {}\n  Description: {}\n  Action: {}\n  Target ID: {}\n  Created: {}\n",
            p.status.to_uppercase(),
            p.proposal_type,
            p.id,
            p.description,
            p.suggested_action,
            p.target_id,
            p.created_at.to_rfc3339()
        );
    }

    Ok(())
}

pub fn decide(id: String, approve: bool) -> Result<()> {
    let config = Config::default_config();
    let db = Database::open(&config.db_path)
        .with_context(|| format!("Failed to open database at {:?}", config.db_path))?;

    let success = db.decide_proposal(&id, approve)?;
    if success {
        let action = if approve { "Approved" } else { "Rejected" };
        println!("✓ {} proposal ID: {}", action, id);
    } else {
        println!("✗ Proposal ID '{}' not found", id);
    }

    Ok(())
}

pub fn approve_all(proposal_type: Option<String>, project: Option<String>) -> Result<()> {
    let config = Config::default_config();
    let db = Database::open(&config.db_path)
        .with_context(|| format!("Failed to open database at {:?}", config.db_path))?;

    let result = db.decide_all_proposals(project.as_deref(), proposal_type.as_deref(), true)?;

    println!("========================================================");
    println!("         AutoMemory Bulk Proposal Approval Result       ");
    println!("========================================================");
    println!("  • Proposals Approved:        {}", result.approved_count);
    println!("  • Duplicate Memories Archived: {}", result.archived_memories_count);
    println!("========================================================");

    Ok(())
}

pub fn reject_all(proposal_type: Option<String>, project: Option<String>) -> Result<()> {
    let config = Config::default_config();
    let db = Database::open(&config.db_path)
        .with_context(|| format!("Failed to open database at {:?}", config.db_path))?;

    let result = db.decide_all_proposals(project.as_deref(), proposal_type.as_deref(), false)?;

    println!("========================================================");
    println!("         AutoMemory Bulk Proposal Rejection Result      ");
    println!("========================================================");
    println!("  • Proposals Rejected:        {}", result.rejected_count);
    println!("========================================================");

    Ok(())
}
