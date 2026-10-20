//! Deterministic drafter: reads the project's archived chats and proposes brief items as drafts (origin transcript,
//! state draft). No model is called. Drafts are shown with their provenance and a next agent or you confirms,
//! corrects or removes them; nothing here is trusted knowledge. To add a heuristic, add a file with an `Extractor`
//! and one line in `extractors()`.
mod rules;

use super::archive::{ChatMessage, ChatSession};
use super::{ItemKind, ItemStatus, NewItem, Origin};
use crate::db::Database;
use anyhow::Result;
use serde::Serialize;

/// Newest sessions first; at most this many are read, so a long history stays cheap.
const SESSIONS: usize = 5;
const MESSAGES: usize = 3000;

pub struct DraftContext {
    /// (session, messages in order), newest session first.
    pub chats: Vec<(ChatSession, Vec<ChatMessage>)>,
}

#[derive(Debug, Clone)]
pub struct Draft {
    pub kind: ItemKind,
    pub text: String,
    pub reason: Option<String>,
    pub session: String,
}

pub trait Extractor {
    fn id(&self) -> &'static str;
    /// Most relevant first; the runner applies `cap`.
    fn extract(&self, ctx: &DraftContext) -> Vec<Draft>;
    fn cap(&self) -> usize;
}

pub fn extractors() -> Vec<Box<dyn Extractor>> {
    vec![Box::new(rules::Goal), Box::new(rules::Constraints), Box::new(rules::Rejections), Box::new(rules::Decisions), Box::new(rules::Next), Box::new(rules::Open)]
}

#[derive(Debug, Default, Serialize)]
pub struct DraftReport {
    pub added: usize,
    pub already_there: usize,
    pub skipped_removed: usize,
}

/// Drafts items from this project's archived chats. Safe to repeat: identical items are not added twice, and an item a
/// person removed or finished is not drafted again.
pub fn draft_project(db: &Database, project_id: Option<&str>) -> Result<DraftReport> {
    let mut sessions = db.chat_sessions(project_id, None)?;
    sessions.sort_by(|a, b| b.last_at.cmp(&a.last_at));
    let mut chats = Vec::new();
    for s in sessions.into_iter().take(SESSIONS) {
        let msgs = db.chat_get(&s.id, 0, MESSAGES)?;
        chats.push((s, msgs));
    }
    let ctx = DraftContext { chats };
    let all = db.brief_list(project_id, true)?;
    let norm = |s: &str| s.split_whitespace().collect::<Vec<_>>().join(" ").to_lowercase();
    let mut report = DraftReport::default();
    for ex in extractors() {
        for d in ex.extract(&ctx).into_iter().take(ex.cap()) {
            let gone = all.iter().any(|i| i.kind == d.kind && i.status != ItemStatus::Active && norm(&i.text) == norm(&d.text));
            if gone {
                report.skipped_removed += 1;
                continue;
            }
            // A goal or status written by an agent or a person is never replaced by a guess.
            if d.kind.is_singleton() && all.iter().any(|i| i.kind == d.kind && i.status == ItemStatus::Active && i.origin != Origin::Transcript) {
                continue;
            }
            let agent = ctx.chats.iter().find(|(s, _)| s.id == d.session).map(|(s, _)| s.agent.clone()).unwrap_or_else(|| "chat".into());
            let (_, new) = db.brief_add(NewItem { project_id: project_id.map(String::from), kind: d.kind, text: d.text, reason: d.reason, ref_id: None, origin: Origin::Transcript, author: agent, session: Some(d.session) })?;
            if new { report.added += 1 } else { report.already_there += 1 }
        }
    }
    Ok(report)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::continuity::import::import_project;
    use crate::continuity::readers::claude_code::ClaudeCode;
    use crate::continuity::ItemState;
    use std::path::Path;

    fn db_with_fixture() -> Database {
        let fixture = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tests/continuity/fixtures/claude/home");
        let db = Database::open_in_memory().unwrap();
        import_project(&db, &ClaudeCode, &fixture, Some("p"), Path::new("/tmp/demo-shop")).unwrap();
        db
    }

    #[test]
    fn a_fixture_chat_yields_the_expected_drafts_once() {
        let db = db_with_fixture();
        let r = draft_project(&db, Some("p")).unwrap();
        assert!(r.added >= 3, "{r:?}");
        let items = db.brief_list(Some("p"), false).unwrap();
        let has = |k: ItemKind, w: &str| items.iter().any(|i| i.kind == k && i.text.contains(w) && i.state == ItemState::Draft && i.origin == Origin::Transcript);
        assert!(has(ItemKind::Goal, "shopping cart"), "{items:#?}");
        assert!(has(ItemKind::Rejected, "sticky routing"), "{items:#?}");
        assert!(has(ItemKind::Next, "cart_items table"), "{items:#?}");
        assert!(items.iter().all(|i| !i.text.contains("sk-FIXTURE") && !i.text.contains("do not show this")));
        let again = draft_project(&db, Some("p")).unwrap();
        assert_eq!(again.added, 0, "{again:?}");
    }

    #[test]
    fn a_removed_draft_stays_removed_and_a_written_goal_is_not_replaced() {
        let db = db_with_fixture();
        draft_project(&db, Some("p")).unwrap();
        let rej = db.brief_list(Some("p"), false).unwrap().into_iter().find(|i| i.kind == ItemKind::Rejected).unwrap();
        db.brief_set_status(&rej.id, ItemStatus::Removed).unwrap();
        let goal = db.brief_list(Some("p"), false).unwrap().into_iter().find(|i| i.kind == ItemKind::Goal).unwrap();
        db.brief_set_status(&goal.id, ItemStatus::Resolved).unwrap();
        db.brief_add(NewItem { project_id: Some("p".into()), kind: ItemKind::Goal, text: "Ship the cart by Friday".into(), reason: None, ref_id: None, origin: Origin::Human, author: "human".into(), session: None }).unwrap();
        let r = draft_project(&db, Some("p")).unwrap();
        assert!(r.skipped_removed >= 1, "{r:?}");
        let items = db.brief_list(Some("p"), false).unwrap();
        assert!(!items.iter().any(|i| i.id == rej.id || (i.kind == ItemKind::Rejected && i.text == rej.text)));
        assert_eq!(items.iter().filter(|i| i.kind == ItemKind::Goal).count(), 1);
        assert!(items.iter().any(|i| i.kind == ItemKind::Goal && i.text.contains("Friday")));
    }
}
