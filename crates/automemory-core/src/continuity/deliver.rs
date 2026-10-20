//! The brief as text for the next agent: layered (most useful first), every item labelled with where it came from,
//! trimmed to a token budget with a pointer to the rest. Used by the resume pack and by the MCP tool.
use super::model::*;
use crate::db::Database;
use chrono::{DateTime, Utc};

pub const DEFAULT_INJECT_TOKENS: usize = 3000;

fn ago(when: DateTime<Utc>) -> String {
    let m = (Utc::now() - when).num_minutes().max(0);
    match m {
        0 => "just now".into(),
        1..=59 => format!("{m} min ago"),
        60..=1439 => format!("{} h ago", m / 60),
        _ => format!("{} d ago", m / 1440),
    }
}

/// Who stands behind an item, in a few words: "(you)", "(claude, 2 h ago)", "(draft from antigravity chat)".
pub fn label(db: &Database, i: &BriefItem) -> String {
    let who = crate::sync::deltas::shared_by(db, &i.id).map(|n| format!("from {n}"));
    let core = match (i.origin, i.state) {
        (_, ItemState::Edited) => "edited by you".to_string(),
        (Origin::Human, _) => "you".to_string(),
        (Origin::Transcript, ItemState::Draft) => format!("draft from {} chat, unconfirmed", i.author),
        (Origin::Transcript, _) => format!("{} chat, confirmed", i.author),
        (Origin::Agent, _) => i.author.clone(),
    };
    let parts: Vec<String> = [Some(core), who, Some(ago(i.updated_at))].into_iter().flatten().collect();
    format!("({})", parts.join(", "))
}

fn line(db: &Database, i: &BriefItem) -> String {
    let why = i.reason.as_deref().map(|r| format!(" because {r}")).unwrap_or_default();
    format!("- {}{why} {}", i.text.trim_end_matches('.'), label(db, i))
}

/// Every active item grouped by kind, within about `max_tokens` (4 characters per token). `None` when there is nothing.
pub fn render_core(db: &Database, project_id: Option<&str>, max_tokens: usize) -> Option<String> {
    let items = db.brief_list(project_id, false).ok()?;
    if items.is_empty() {
        return None;
    }
    let budget = max_tokens.max(200) * 4;
    let (mut out, mut used, mut omitted) = (String::from("Brief (what the previous sessions worked out; confirm or correct with continuity.brief):\n"), 0usize, 0usize);
    used += out.len();
    for kind in ItemKind::ALL {
        let mut group: Vec<&BriefItem> = items.iter().filter(|i| i.kind == kind).collect();
        if group.is_empty() {
            continue;
        }
        // Confirmed before drafts, newest first (list order is newest first already).
        group.sort_by_key(|i| i.state == ItemState::Draft);
        let heading = format!("{}:\n", kind.heading());
        let mut section = String::new();
        for i in group {
            let l = format!("{}\n", line(db, i));
            if used + heading.len() + section.len() + l.len() > budget {
                omitted += 1;
            } else {
                section.push_str(&l);
            }
        }
        if !section.is_empty() {
            used += heading.len() + section.len();
            out.push_str(&heading);
            out.push_str(&section);
        }
    }
    if omitted > 0 {
        out.push_str(&format!("…{omitted} more item(s): call continuity.brief (action get) for all of them.\n"));
    }
    Some(out.trim_end().to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn add(db: &Database, kind: ItemKind, text: &str, origin: Origin, author: &str, reason: Option<&str>) {
        db.brief_add(NewItem { project_id: Some("p".into()), kind, text: text.into(), reason: reason.map(String::from), ref_id: None, origin, author: author.into(), session: None }).unwrap();
    }

    #[test]
    fn the_brief_reads_in_order_with_reasons_and_provenance() {
        let db = Database::open_in_memory().unwrap();
        assert!(render_core(&db, Some("p"), 3000).is_none());
        add(&db, ItemKind::Rejected, "Server-side sessions", Origin::Agent, "antigravity", Some("the load balancer has no sticky routing"));
        add(&db, ItemKind::Goal, "Ship the cart", Origin::Human, "human", None);
        add(&db, ItemKind::Next, "Write the cart table", Origin::Transcript, "antigravity", None);
        let text = render_core(&db, Some("p"), 3000).unwrap();
        let (g, n, r) = (text.find("Goal:").unwrap(), text.find("Next steps:").unwrap(), text.find("Ruled out").unwrap());
        assert!(g < n && n < r, "goal, then next, then ruled out:\n{text}");
        assert!(text.contains("Server-side sessions because the load balancer has no sticky routing (antigravity,"), "{text}");
        assert!(text.contains("Ship the cart (you,") && text.contains("draft from antigravity chat, unconfirmed"), "{text}");
    }

    #[test]
    fn a_small_budget_trims_and_points_at_the_rest() {
        let db = Database::open_in_memory().unwrap();
        for i in 0..60 {
            add(&db, ItemKind::Next, &format!("A reasonably long next step number {i} with some detail attached to it"), Origin::Agent, "claude", None);
        }
        let text = render_core(&db, Some("p"), 300).unwrap();
        assert!(text.len() < 1600 && text.contains("more item(s): call continuity.brief"), "{} chars", text.len());
    }
}
