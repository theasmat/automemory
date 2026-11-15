//! `automemory room`: planning rooms where agents propose, critique and revise, and you give the verdict.
use anyhow::Result;
use automemory_core::config::Config;
use automemory_core::db::Database;

fn db() -> Result<Database> {
    Database::open(&Config::default_config().db_path)
}

pub fn list() -> Result<()> {
    let db = db()?;
    let rooms = db.list_rooms(None, 50)?;
    if rooms.is_empty() {
        println!("No planning rooms yet. An agent opens one with the room.join tool.");
    }
    for (r, posts) in rooms {
        let note = match (db.room_outcome(&r.id), r.state) {
            (Some(o), _) => format!("{} by you", o.outcome),
            (None, automemory_core::models::RoomState::Decided) => "WAITING FOR YOUR VERDICT".to_string(),
            (None, s) => format!("{s}, turn: {}", r.turn_holder.as_deref().map(|t| db.agent_label(t)).unwrap_or_else(|| "open".into())),
        };
        println!("{}  {:<28} {} post(s)  {}", r.id, r.topic.chars().take(28).collect::<String>(), posts, note);
    }
    Ok(())
}

pub fn show(id: &str) -> Result<()> {
    let db = db()?;
    let room = db.get_room(id)?.ok_or_else(|| anyhow::anyhow!("room not found"))?;
    let members: Vec<String> = room.members.iter().map(|m| db.agent_label(m)).collect();
    println!("{}: {} [{}], members: {}", room.id, room.topic, room.state, members.join(", "));
    for p in db.room_posts(id)? {
        println!("\n--- {} ({}) ---\n{}", p.kind, db.agent_label(&p.author_session), p.body);
        if !p.evidence.is_empty() {
            println!("evidence: {}", p.evidence.join(", "));
        }
    }
    if let Ok(plan) = db.room_plan(id) {
        println!("\n=== If you approve ===\nDecision ({}): {}", plan.topic, plan.decision);
        for t in &plan.tasks {
            println!("  task: {t}");
        }
    }
    if let Some(o) = db.room_outcome(id) {
        println!("\nOutcome: {}", o.outcome);
    }
    Ok(())
}

fn human(what: &str) -> Result<Database> {
    automemory_core::auth::require_human(automemory_core::auth::stored_password_hash().as_deref(), what)?;
    db()
}

pub fn approve(id: &str) -> Result<()> {
    let o = human("approve a planning room's plan")?.approve_room(id)?;
    println!("✓ Approved. Recorded the decision and created {} task(s); agents pick them up with task.next.", o.task_ids.len());
    Ok(())
}

pub fn reject(id: &str) -> Result<()> {
    human("reject a planning room's plan")?.reject_room(id)?;
    println!("✓ Rejected. Nothing was stored.");
    Ok(())
}

pub fn again(id: &str) -> Result<()> {
    let r = human("send a planning room back for another round")?.another_round(id)?;
    println!("✓ Back for another round: {} critiques next.", r.turn_holder.unwrap_or_default());
    Ok(())
}
