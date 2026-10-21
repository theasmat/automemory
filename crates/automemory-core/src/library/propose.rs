//! An agent may not change a file a person wrote, but it may suggest an edit. The suggestion waits in the Inbox with a short diff; approving it
//! (a person, with the password) applies it as the person's change, rejecting it throws it away. Never applied in bulk.
use super::model::*;
use super::store::clean_path;
use crate::db::Database;
use crate::models::ReviewProposal;
use crate::redaction::contains_secrets;
use anyhow::{bail, Result};
use chrono::Utc;
use serde::{Deserialize, Serialize};

const PENDING_MAX: usize = 5;
const DIFF_LINES: usize = 10;

#[derive(Serialize, Deserialize)]
struct Pending {
    project_id: Option<String>,
    path: String,
    content: String,
    agent: String,
}

/// Lines only in the new text (`+`) and only in the old (`-`), first few of each. Not a real diff algorithm: enough to see what changes.
fn summary(old: &str, new: &str) -> (usize, usize, String) {
    let o: std::collections::HashSet<&str> = old.lines().collect();
    let n: std::collections::HashSet<&str> = new.lines().collect();
    let added: Vec<&str> = new.lines().filter(|l| !l.trim().is_empty() && !o.contains(l)).collect();
    let removed: Vec<&str> = old.lines().filter(|l| !l.trim().is_empty() && !n.contains(l)).collect();
    let show = |tag: &str, v: &[&str]| v.iter().take(DIFF_LINES / 2).map(|l| format!("{tag} {}", l.chars().take(120).collect::<String>())).collect::<Vec<_>>().join("\n");
    let text = [show("+", &added), show("-", &removed)].into_iter().filter(|s| !s.is_empty()).collect::<Vec<_>>().join("\n");
    (added.len(), removed.len(), text)
}

/// Files an Inbox item for the suggested text. Returns the proposal id.
pub fn propose(db: &Database, project_id: Option<&str>, path: &str, content: &str, reason: Option<&str>, agent: &str) -> Result<String> {
    let path = clean_path(path)?;
    if content.contains('\0') || content.len() > FILE_MAX {
        bail!("only text up to {} KB is accepted", FILE_MAX / 1024);
    }
    if let Some(i) = content.lines().position(contains_secrets) {
        bail!("line {} looks like a secret: remove it first", i + 1);
    }
    let existing = db.library_get(project_id, &path)?;
    if existing.as_ref().is_some_and(|e| e.linked) {
        bail!("{path} is kept in place in the project folder: edit that file directly");
    }
    let old = existing.as_ref().map(|e| e.content.as_str()).unwrap_or("");
    if existing.as_ref().is_some_and(|e| e.content == content) {
        bail!("that is already the text of {path}");
    }
    let pending = db.list_pending_proposals(project_id)?.into_iter().filter(|p| p.proposal_type == "library_edit").count();
    if pending >= PENDING_MAX {
        bail!("{PENDING_MAX} library suggestions are already waiting for the person: wait for them to be decided");
    }
    let (add, del, diff) = summary(old, content);
    let id = format!("libprop-{}", uuid::Uuid::now_v7().simple());
    db.sync_kv_set(&format!("libprop:{id}"), &serde_json::to_string(&Pending { project_id: project_id.map(String::from), path: path.clone(), content: content.into(), agent: agent.into() })?);
    db.save_proposal(&ReviewProposal {
        id: id.clone(),
        project_id: project_id.map(String::from),
        proposal_type: "library_edit".into(),
        target_id: path.clone(),
        description: format!(
            "{agent} suggests {} library file {path} (+{add} -{del} lines){}.\n{diff}",
            if existing.is_some() { "changing" } else { "creating" },
            reason.map(|r| format!(": {}", r.chars().take(200).collect::<String>())).unwrap_or_default()
        ),
        suggested_action: "Approve to apply it as your change (it can be restored from History), reject to discard".into(),
        status: "pending".into(),
        created_at: Utc::now(),
        decided_at: None,
    })?;
    Ok(id)
}

impl Database {
    /// Called when a person approves a `library_edit` proposal: the suggested text becomes the file, under the person's name.
    pub fn library_apply_proposal(&self, proposal_id: &str) -> Result<()> {
        let key = format!("libprop:{proposal_id}");
        let Some(p) = self.sync_kv_get(&key).filter(|v| !v.is_empty()).and_then(|v| serde_json::from_str::<Pending>(&v).ok()) else { return Ok(()) };
        self.library_put(NewFile { project_id: p.project_id, path: p.path, content: p.content, origin: Origin::Human, author: format!("approved from {}", p.agent), personal: false, allow_secrets: false, linked: false })?;
        self.sync_kv_set(&key, "");
        Ok(())
    }

    pub fn library_drop_proposal(&self, proposal_id: &str) {
        self.sync_kv_set(&format!("libprop:{proposal_id}"), "");
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn put(db: &Database, path: &str, text: &str) {
        db.library_put(NewFile { project_id: Some("p".into()), path: path.into(), content: text.into(), origin: Origin::Human, author: "human".into(), personal: false, allow_secrets: false, linked: false }).unwrap();
    }

    #[test]
    fn a_suggestion_waits_for_a_person_and_is_applied_only_on_approval() {
        let db = Database::open_in_memory().unwrap();
        put(&db, "plans/prd.md", "# PRD\n\nCarts expire after 30 days.\n");
        let id = propose(&db, Some("p"), "plans/prd.md", "# PRD\n\nCarts expire after 90 days.\n", Some("support asked"), "claude").unwrap();
        assert!(db.library_get(Some("p"), "plans/prd.md").unwrap().unwrap().content.contains("30 days"), "nothing changed yet");
        let pend = db.list_pending_proposals(Some("p")).unwrap();
        let p = pend.iter().find(|p| p.id == id).unwrap();
        assert!(p.description.contains("claude suggests changing") && p.description.contains("+ Carts expire after 90 days") && p.description.contains("- Carts expire after 30 days") && p.description.contains("support asked"), "{}", p.description);
        assert!(db.decide_proposal(&id, true).unwrap());
        let f = db.library_get(Some("p"), "plans/prd.md").unwrap().unwrap();
        assert!(f.content.contains("90 days") && f.origin == Origin::Human && f.author == "approved from claude" && f.revisions == 2, "{f:?}");
        // Rejecting throws it away.
        let id2 = propose(&db, Some("p"), "plans/prd.md", "# PRD\n\nRejected text.\n", None, "claude").unwrap();
        db.decide_proposal(&id2, false).unwrap();
        assert!(db.library_get(Some("p"), "plans/prd.md").unwrap().unwrap().content.contains("90 days"));
    }

    #[test]
    fn suggestions_are_checked_and_limited_and_never_approved_in_bulk() {
        let db = Database::open_in_memory().unwrap();
        put(&db, "a.md", "# A\n\nText.\n");
        assert!(propose(&db, Some("p"), "a.md", "# A\n\nText.\n", None, "claude").is_err(), "no change");
        assert!(propose(&db, Some("p"), "a.md", "key sk-abcdefghijklmnopqrstuvwxyz1234567890\n", None, "claude").unwrap_err().to_string().contains("line 1"));
        assert!(propose(&db, Some("p"), "../a.md", "x", None, "claude").is_err());
        let mut ids = vec![];
        for i in 0..PENDING_MAX {
            ids.push(propose(&db, Some("p"), "a.md", &format!("# A\n\nVersion {i}.\n"), None, "claude").unwrap());
        }
        assert!(propose(&db, Some("p"), "a.md", "# A\n\nOne too many.\n", None, "claude").unwrap_err().to_string().contains("already waiting"));
        let r = db.decide_all_proposals(Some("p"), Some("all"), true).unwrap();
        assert_eq!(r.decided_count, 0, "approve-all leaves library suggestions for a person to read");
        assert!(db.library_get(Some("p"), "a.md").unwrap().unwrap().content.contains("Text."));
        assert!(db.decide_proposal(&ids[0], true).unwrap());
        assert!(db.library_get(Some("p"), "a.md").unwrap().unwrap().content.contains("Version 0"));
    }
}
