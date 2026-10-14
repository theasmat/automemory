use anyhow::Result;
use automemory_core::baton::pass_baton;
use automemory_core::config::Config;
use automemory_core::db::Database;

/// Hands off work to another agent session by generating a ledger-grounded baton packet.
pub fn pass(
    session_id: &str,
    to_session: Option<&str>,
    agent: &str,
    note: Option<&str>,
    project_arg: Option<&str>,
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

    let baton = pass_baton(
        &db,
        project_id.as_deref(),
        session_id,
        to_session,
        agent,
        note,
    )?;

    println!("🏃 Baton passed successfully!");
    println!("   ID:         {}", baton.id);
    println!("   From:       {} ({})", baton.from_session, baton.agent);
    if let Some(ref target) = baton.to_session {
        println!("   To:         {}", target);
    } else {
        println!("   To:         [broadcast / any agent]");
    }
    println!(
        "\n📦 Packet ({} bytes):\n{:-<60}\n{}\n{:-<60}",
        baton.packet.len(),
        "",
        baton.packet,
        ""
    );

    Ok(())
}

/// Retrieves the latest handoff packet for this session (direct first, then broadcast).
pub fn pickup(session_id: Option<&str>, project_arg: Option<&str>) -> Result<()> {
    let config = Config::default_config();
    let db = Database::open(&config.db_path)?;

    let project_id = project_arg.and_then(|p| {
        db.get_project_by_slug(p)
            .ok()
            .flatten()
            .map(|proj| proj.id)
            .or_else(|| Some(p.to_string()))
    });

    match db.latest_baton(project_id.as_deref(), session_id)? {
        Some(baton) => {
            println!("📥 Baton received!");
            println!("   ID:         {}", baton.id);
            println!("   From:       {} ({})", baton.from_session, baton.agent);
            println!("   Created At: {}", baton.created_at.to_rfc3339());
            println!("\n📦 Packet:\n{:-<60}\n{}\n{:-<60}", "", baton.packet, "");
        }
        None => {
            println!("ℹ️  No handoff baton found for session: {:?}", session_id);
        }
    }

    Ok(())
}
