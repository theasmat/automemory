//! Notes nobody uses fade. An agent-written note that has not been recalled for 60 days ranks lower (half after 60 days, a quarter after 120), and is offered to the
//! person for archiving in the Inbox. A person's own notes, pinned notes and decisions never fade; archiving is reversible and always the person's decision.
use crate::db::Database;
use crate::models::{Memory, MemorySourceKind, MemoryStatus, MemoryType, ReviewProposal};
use anyhow::Result;
use chrono::{DateTime, Utc};

pub const IDLE_DAYS: i64 = 60;
/// New archive offers per session start, and the most that may wait in the Inbox at once.
const OFFERS_PER_PASS: usize = 5;
const OFFERS_WAITING_MAX: usize = 15;

fn fadeable(m: &Memory) -> bool {
    m.status == MemoryStatus::Active && m.source_kind != MemorySourceKind::User && m.memory_type != MemoryType::Decision
}

/// Days since the note was last recalled (or written, if never).
pub fn idle_days(m: &Memory, now: DateTime<Utc>) -> i64 {
    now.signed_duration_since(m.last_recalled_at.unwrap_or(m.created_at)).num_days()
}

/// Multiplier for the rank of a note: 1 normally, 0.5 after 60 idle days, 0.25 after 120.
pub fn factor(m: &Memory, now: DateTime<Utc>) -> f64 {
    if !fadeable(m) {
        return 1.0;
    }
    match idle_days(m, now) {
        d if d >= 2 * IDLE_DAYS => 0.25,
        d if d >= IDLE_DAYS => 0.5,
        _ => 1.0,
    }
}

/// Puts up to a few unused notes in the Inbox as "archive this?" items (each note is offered once). Returns how many were offered.
pub fn offer_stale_notes(db: &Database, project_id: Option<&str>) -> Result<usize> {
    let waiting = db.list_pending_proposals(project_id)?.iter().filter(|p| p.proposal_type == "stale_note").count();
    if waiting >= OFFERS_WAITING_MAX {
        return Ok(0);
    }
    let now = Utc::now();
    let mut offered = 0;
    for m in db.list_memories(project_id, Some(MemoryStatus::Active), None, 1000)? {
        if offered >= OFFERS_PER_PASS || waiting + offered >= OFFERS_WAITING_MAX {
            break;
        }
        let days = idle_days(&m, now);
        let id = format!("stale-{}", m.id);
        if !fadeable(&m) || days < IDLE_DAYS || db.get_proposal_by_id(&id)?.is_some() {
            continue;
        }
        db.save_proposal(&ReviewProposal {
            id,
            project_id: m.project_id.clone(),
            proposal_type: "stale_note".into(),
            target_id: m.id.clone(),
            description: format!("Nobody has used this note in {days} days: \"{}\". Archive it?", m.summary.chars().take(160).collect::<String>()),
            suggested_action: "Approve to archive it (restorable), reject to keep it".into(),
            status: "pending".into(),
            created_at: now,
            decided_at: None,
        })?;
        offered += 1;
    }
    Ok(offered)
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Duration;

    fn note(db: &Database, id: &str, source: MemorySourceKind, kind: MemoryType, age_days: i64, recalled_days_ago: Option<i64>) -> Memory {
        let m = Memory {
            id: id.into(), scope: crate::models::MemoryScope::Global, project_id: None, memory_type: kind, content: format!("{id} content"), summary: format!("{id} summary"),
            confidence: 0.8, importance: 0.5, freshness: 1.0, status: MemoryStatus::Active, source_kind: source, version: Some(1),
            created_at: Utc::now() - Duration::days(age_days), updated_at: Utc::now() - Duration::days(age_days),
            last_recalled_at: recalled_days_ago.map(|d| Utc::now() - Duration::days(d)), tags: vec![], entities: vec![], metadata: serde_json::json!({}),
        };
        db.save_memory_direct(&m).unwrap();
        m
    }

    #[test]
    fn only_old_unused_agent_notes_fade() {
        let db = Database::open_in_memory().unwrap();
        let now = Utc::now();
        let fresh = note(&db, "fresh", MemorySourceKind::Agent, MemoryType::Fact, 10, None);
        let old = note(&db, "old", MemorySourceKind::Agent, MemoryType::Fact, 70, None);
        let older = note(&db, "older", MemorySourceKind::Agent, MemoryType::Fact, 200, None);
        let used = note(&db, "used", MemorySourceKind::Agent, MemoryType::Fact, 200, Some(5));
        let mine = note(&db, "mine", MemorySourceKind::User, MemoryType::Fact, 200, None);
        let decision = note(&db, "decision", MemorySourceKind::Agent, MemoryType::Decision, 200, None);
        assert_eq!([&fresh, &old, &older, &used, &mine, &decision].map(|m| factor(m, now)), [1.0, 0.5, 0.25, 1.0, 1.0, 1.0]);
    }

    #[test]
    fn unused_notes_are_offered_once_few_at_a_time_and_archiving_is_a_decision() {
        let db = Database::open_in_memory().unwrap();
        for i in 0..8 {
            note(&db, &format!("n{i}"), MemorySourceKind::Agent, MemoryType::Fact, 90, None);
        }
        note(&db, "mine", MemorySourceKind::User, MemoryType::Fact, 90, None);
        note(&db, "fresh", MemorySourceKind::Agent, MemoryType::Fact, 3, None);
        assert_eq!(offer_stale_notes(&db, None).unwrap(), 5, "a few per session start");
        assert_eq!(offer_stale_notes(&db, None).unwrap(), 3, "the rest next time");
        assert_eq!(offer_stale_notes(&db, None).unwrap(), 0, "each note is offered once");
        let pending = db.list_pending_proposals(None).unwrap();
        assert_eq!(pending.len(), 8);
        assert!(pending.iter().all(|p| p.proposal_type == "stale_note" && p.description.contains("Nobody has used this note in 90 days")));
        let archive = &pending[0];
        let keep = &pending[1];
        db.decide_proposal(&archive.id, true).unwrap();
        db.decide_proposal(&keep.id, false).unwrap();
        assert_eq!(db.get_memory_by_id(&archive.target_id).unwrap().unwrap().status, MemoryStatus::Archived);
        let kept = db.get_memory_by_id(&keep.target_id).unwrap().unwrap();
        assert_eq!(kept.status, MemoryStatus::Active);
        assert!(factor(&kept, Utc::now()) == 1.0, "keeping a note counts as using it");
    }
}
