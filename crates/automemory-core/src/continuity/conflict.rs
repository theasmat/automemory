//! A new decision that goes against something the brief says is ruled out. The agent that wrote the brief did so for a reason (the user did not
//! ask for it, it needs a broker, it was tried and failed); an agent that starts a session later can walk back into it without noticing.
//! This is a flag for a person, never a block: the check is by shared words, so it can be wrong, and a decision may be right to change the plan.

use super::{BriefItem, ItemKind};
use crate::db::Database;
use crate::models::{Decision, ReviewProposal};
use anyhow::Result;
use chrono::Utc;

const STOP: &[&str] = &[
    "the", "and", "are", "for", "not", "but", "was", "were", "with", "that", "this", "from", "into", "only", "also", "than", "then", "them",
    "they", "have", "has", "had", "its", "our", "your", "use", "used", "using", "will", "would", "should", "could", "can", "any", "all",
    "because", "since", "instead", "rather", "without", "small", "simple", "minimal", "tool", "project", "step", "steps", "when", "where",
    "which", "what", "who", "how", "why", "yes", "out", "off", "one", "two", "new", "add", "added", "adds", "make", "made", "need", "needs",
    "excluded", "exclude", "ruled", "rule", "rejected", "reject", "decision", "decided", "support", "supports", "provide", "provides",
];
/// A decision that says "no" to the same thing agrees with the brief, so it is not a conflict.
const NEGATION: &[&str] = &["not ", "no ", "never ", "without ", "instead of", "rather than", "avoid", "reject", "ruled out", "excluded", "exclude", "do not", "don't"];

fn stem(w: &str) -> String {
    let n = w.chars().count();
    let mut out = w.to_string();
    for suf in ["ies", "ing", "es", "ed", "s"] {
        if n > suf.len() + 2 {
            if let Some(b) = w.strip_suffix(suf) {
                out = if suf == "ies" { format!("{b}y") } else { b.to_string() };
                break;
            }
        }
    }
    // "invoice", "invoicing" and "invoiced" are one word: drop a final e after the suffix is gone
    if out.chars().count() > 4 && out.ends_with('e') {
        out.pop();
    }
    out
}

pub(crate) fn terms(text: &str) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    for w in text.to_lowercase().split(|c: char| !c.is_alphanumeric()).filter(|w| w.chars().count() >= 3 && !STOP.contains(w)) {
        let s = stem(w);
        if !out.contains(&s) {
            out.push(s);
        }
    }
    out
}

/// The ruled-out items whose words a decision uses, with the shared words. A short ruling matches on one shared word, a long one needs two,
/// and a shared word that is just the subject of the whole project (it is in the goal: "refund" in "Ship partial refunds") never counts alone.
pub fn conflicts(ruled_out: &[BriefItem], goal_text: &str, decision_text: &str) -> Vec<(BriefItem, Vec<String>)> {
    let lower = decision_text.to_lowercase();
    if NEGATION.iter().any(|n| lower.contains(n)) {
        return Vec::new();
    }
    let mine = terms(decision_text);
    let about = terms(goal_text);
    ruled_out
        .iter()
        .filter(|i| i.kind == ItemKind::Rejected)
        .filter_map(|i| {
            // the ruling itself is its first sentence; agents often append notes about the schema, a future idea and so on after it
            let head = i.text.split(['.', ';']).next().unwrap_or(&i.text);
            let theirs = terms(head);
            let shared: Vec<String> = theirs.iter().filter(|t| mine.contains(t)).cloned().collect();
            let distinctive = shared.iter().filter(|t| !about.contains(t)).count();
            // A short ruling ("Refund through a job queue") is matched by one distinctive word; a long one only when a real share of its words come
            // back, so two incidental words ("point" in "floating-point" and "basis points") do not make "integer cents" contradict "no floats".
            let hit = if theirs.len() <= 4 { !shared.is_empty() } else { shared.len() >= 3 || (shared.len() >= 2 && shared.len() * 4 >= theirs.len()) };
            (hit && distinctive >= 1).then(|| (i.clone(), shared))
        })
        .collect()
}

impl Database {
    /// Checks a freshly recorded decision against the project's ruled-out items. For each one it touches, a proposal is filed in the Inbox for a
    /// person and a one-line warning is returned for the agent that recorded it.
    pub fn flag_ruled_out_conflicts(&self, d: &Decision) -> Result<Vec<String>> {
        let items = self.brief_list(d.project_id.as_deref(), false)?;
        let ruled_out: Vec<BriefItem> = items.iter().filter(|i| i.kind == ItemKind::Rejected).cloned().collect();
        let goal: String = items.iter().filter(|i| i.kind == ItemKind::Goal).map(|i| i.text.as_str()).collect::<Vec<_>>().join(" ");
        let mut warnings = Vec::new();
        for (item, shared) in conflicts(&ruled_out, &goal, &format!("{} {}", d.topic, d.decision)) {
            let by = if item.author.is_empty() { "an earlier session".to_string() } else { item.author.clone() };
            let why = item.reason.as_deref().map(|r| format!(" because {r}")).unwrap_or_default();
            warnings.push(format!("This decision touches something the brief rules out ({}): \"{}\", ruled out by {by}{why}. If the plan really changes, say so in the reason; a person is asked to confirm.", shared.join(", "), item.text));
            self.save_proposal(&ReviewProposal {
                id: format!("conflict-{}-{}", d.id, item.id),
                project_id: d.project_id.clone(),
                proposal_type: "rule_conflict".into(),
                target_id: d.id.clone(),
                description: format!("{} recorded the decision \"{}\" ({}), but the brief says: \"{}\" is ruled out by {by}{why}.", d.agent, d.decision.chars().take(140).collect::<String>(), d.topic, item.text),
                suggested_action: "Approve if the plan has changed, reject if the decision should not stand (then supersede or correct it)".into(),
                status: "pending".into(),
                created_at: Utc::now(),
                decided_at: None,
            })?;
        }
        Ok(warnings)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::continuity::{NewItem, Origin};
    use crate::models::DecisionRecordArgs;

    fn ruled_out(db: &Database, text: &str, reason: &str) {
        db.brief_add(NewItem { project_id: None, kind: ItemKind::Rejected, text: text.into(), reason: Some(reason.into()), ref_id: None, origin: Origin::Agent, author: "codex".into(), session: None }).unwrap();
    }

    fn record(db: &Database, topic: &str, decision: &str) -> Decision {
        db.record_decision(DecisionRecordArgs { project_id: None, topic, decision, reason: "because", alternatives: &[], dissent: None, agent: "agy", symbols: &[], evidence: &[] }).unwrap()
    }

    #[test]
    fn a_decision_that_brings_back_a_ruled_out_thing_is_flagged_for_a_person() {
        let db = Database::open_in_memory().unwrap();
        ruled_out(&db, "Tax, discounts, interactive prompts, and background processing are excluded", "the request leaves those policies unspecified");
        ruled_out(&db, "Refund through a job queue", "needs a broker");
        let d = record(&db, "invoice-extensions", "Expand the tool with status tracking, due dates, discounts and tax rates");
        let w = db.flag_ruled_out_conflicts(&d).unwrap();
        assert_eq!(w.len(), 1, "{w:?}");
        assert!(w[0].contains("discount") && w[0].contains("codex"));
        let pending = db.list_pending_proposals(None).unwrap();
        assert!(pending.iter().any(|p| p.proposal_type == "rule_conflict" && p.target_id == d.id));
        // flagging again does not file a second proposal
        db.flag_ruled_out_conflicts(&d).unwrap();
        assert_eq!(db.list_pending_proposals(None).unwrap().iter().filter(|p| p.proposal_type == "rule_conflict").count(), 1);
    }

    #[test]
    fn a_decision_that_agrees_or_is_about_something_else_is_not_flagged() {
        let db = Database::open_in_memory().unwrap();
        ruled_out(&db, "Refund through a job queue", "needs a broker");
        for (topic, text) in [("storage", "Use SQLite for storage"), ("queue", "Do not use a job queue; commands write directly"), ("cli", "Provide create and list subcommands")] {
            let d = record(&db, topic, text);
            assert!(db.flag_ruled_out_conflicts(&d).unwrap().is_empty(), "{text}");
        }
    }

    #[test]
    fn the_projects_own_subject_word_alone_is_not_a_conflict() {
        let db = Database::open_in_memory().unwrap();
        db.brief_add(NewItem { project_id: None, kind: ItemKind::Goal, text: "Ship partial refunds with an admin screen".into(), reason: None, ref_id: None, origin: Origin::Agent, author: "claude".into(), session: None }).unwrap();
        ruled_out(&db, "Refund through a job queue", "needs a broker");
        let fine = record(&db, "admin", "Admin screen lists refunds with a partial refund button");
        assert!(db.flag_ruled_out_conflicts(&fine).unwrap().is_empty());
        let bad = record(&db, "processing", "Send each refund to a job queue");
        assert_eq!(db.flag_ruled_out_conflicts(&bad).unwrap().len(), 1);
    }

    #[test]
    fn agreeing_decisions_that_share_a_stray_word_with_a_long_ruling_are_not_flagged() {
        // seen live: a pricing decision in integer cents ("basis points") against "floating-point money ... 10-million-invoice scaling idea"
        let db = Database::open_in_memory().unwrap();
        ruled_out(&db, "JSON files for persistence, floating-point money, in-memory numbering, and background queues. Keep the current schema unchanged for now; the 10-million-invoice scaling idea is saved as a future note.", "these conflict with exact cents");
        let d = record(&db, "invoice-pricing", "Compute line totals as quantity times unit price in cents, subtotal as sum of line totals, discount capped at subtotal, and tax once on the discounted amount using integer basis points rounded half up.");
        assert!(db.flag_ruled_out_conflicts(&d).unwrap().is_empty());
        let bad = record(&db, "storage", "Keep invoices in JSON files for persistence");
        assert_eq!(db.flag_ruled_out_conflicts(&bad).unwrap().len(), 1, "a real return of a ruled-out thing is still caught");
    }
}
