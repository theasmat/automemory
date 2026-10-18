use anyhow::Result;
use automemory_core::config::Config;
use automemory_core::db::Database;
use automemory_core::models::{DecisionRecordArgs, DecisionStatus};
use std::str::FromStr;

#[derive(Default)]
pub struct DecisionRecordCliArgs<'a> {
    pub topic: &'a str,
    pub decision: &'a str,
    pub reason: &'a str,
    pub alternatives: &'a [String],
    pub dissent: Option<&'a str>,
    pub agent: &'a str,
    pub symbols: &'a [String],
    pub evidence: &'a [String],
    pub project_arg: Option<&'a str>,
}

/// Records a new architectural decision, superseding any prior active decision on the same topic.
pub fn record_decision(args: DecisionRecordCliArgs<'_>) -> Result<()> {
    let config = Config::default_config();
    let db = Database::open(&config.db_path)?;

    let project_id = args.project_arg.and_then(|p| {
        db.get_project_by_slug(p)
            .ok()
            .flatten()
            .map(|proj| proj.id)
            .or_else(|| Some(p.to_string()))
    });

    // No --project: use the project of the current folder, so the decision shows up in its views.
    let project_id = project_id.or_else(|| {
        let root = super::rules_export::repo_root(&std::env::current_dir().unwrap_or_default());
        super::continuity::project_for(&db, &root)
    });

    let recorded = db.record_decision(DecisionRecordArgs {
        project_id: project_id.as_deref(),
        topic: args.topic,
        decision: args.decision,
        reason: args.reason,
        alternatives: args.alternatives,
        dissent: args.dissent,
        agent: args.agent,
        symbols: args.symbols,
        evidence: args.evidence,
    })?;

    super::rules_export::refresh_existing(&db, project_id.as_deref(), &std::env::current_dir().unwrap_or_default());

    println!(
        "✓ Recorded architectural decision '{}' for topic '{}'",
        recorded.id, recorded.topic
    );
    println!("  Decision:     {}", recorded.decision);
    println!("  Rationale:    {}", recorded.reason);
    if !recorded.alternatives.is_empty() {
        println!("  Alternatives: {}", recorded.alternatives.join(", "));
    }
    if let Some(ref d) = recorded.dissent {
        println!("  Dissent:      {}", d);
    }
    if !recorded.symbols.is_empty() {
        println!("  Symbols:      {}", recorded.symbols.join(", "));
    }
    println!("  Status:       Active (synced as Tier 2 Verified Fact)");

    Ok(())
}

/// Retrieves the active decision and preserved history/dissent for an architectural topic.
pub fn get_decision(topic: &str, project_arg: Option<&str>) -> Result<()> {
    let config = Config::default_config();
    let db = Database::open(&config.db_path)?;

    let project_id = project_arg.and_then(|p| {
        db.get_project_by_slug(p)
            .ok()
            .flatten()
            .map(|proj| proj.id)
            .or_else(|| Some(p.to_string()))
    });

    let res = db.get_decision(project_id.as_deref(), topic)?;

    match res.active {
        Some(act) => {
            println!("🏛️  Architectural Decision: {}", act.topic);
            println!("{:-<70}", "");
            println!("ID:           {}", act.id);
            println!("Decision:     {}", act.decision);
            println!("Rationale:    {}", act.reason);
            println!("Agent:        {}", act.agent);
            if !act.alternatives.is_empty() {
                println!("Alternatives: {}", act.alternatives.join(", "));
            }
            if let Some(ref d) = act.dissent {
                println!("Dissent:      {}", d);
            }
            if !act.symbols.is_empty() {
                println!("Symbols:      {}", act.symbols.join(", "));
            }
            if !act.evidence.is_empty() {
                println!("Evidence:     {}", act.evidence.join(", "));
            }
            println!("Updated:      {}", act.updated_at.to_rfc3339());
        }
        None => {
            println!("ℹ️  No active decision found for topic '{}'.", topic);
        }
    }

    if !res.history.is_empty() {
        println!(
            "\n📜 Superseded History ({} prior decisions):",
            res.history.len()
        );
        println!("{:-<70}", "");
        for (i, h) in res.history.iter().enumerate() {
            println!("[{}] {} - {} (by {})", i + 1, h.id, h.decision, h.agent);
            println!("    Rationale:     {}", h.reason);
            if let Some(ref d) = h.dissent {
                println!("    Dissent:       {}", d);
            }
            if let Some(ref s) = h.superseded_by {
                println!("    Superseded By: {}", s);
            }
        }
    }

    Ok(())
}

/// Lists recorded decisions.
pub fn list_decisions(
    project_arg: Option<&str>,
    status_str: Option<&str>,
    limit: usize,
) -> Result<()> {
    let config = Config::default_config();
    let db = Database::open(&config.db_path)?;

    let project_id = project_arg.and_then(|p| {
        db.get_project_by_slug(p)
            .ok()
            .flatten()
            .map(|proj| proj.id)
            .or_else(|| Some(p.to_string()))
    });

    let status = match status_str {
        Some(s) => Some(DecisionStatus::from_str(s).map_err(|e| anyhow::anyhow!(e))?),
        None => None,
    };

    let decisions = db.list_decisions(project_id.as_deref(), status, limit)?;

    if decisions.is_empty() {
        println!("ℹ️  No architectural decisions recorded.");
        return Ok(());
    }

    println!("🏛️  Architectural Decisions ({} total):", decisions.len());
    println!("{:-<95}", "");
    println!(
        "{:<15} {:<10} {:<15} {:<35} Agent",
        "Topic", "Status", "ID", "Decision"
    );
    println!("{:-<95}", "");

    for d in decisions {
        let dec_display = if d.decision.len() > 33 {
            format!("{}..", &d.decision[..31])
        } else {
            d.decision.clone()
        };
        let topic_display = if d.topic.len() > 14 {
            format!("{}..", &d.topic[..12])
        } else {
            d.topic.clone()
        };

        println!(
            "{:<15} {:<10} {:<15} {:<35} {}",
            topic_display,
            d.status.to_string(),
            d.id,
            dec_display,
            d.agent
        );
    }
    println!("{:-<95}", "");

    Ok(())
}
