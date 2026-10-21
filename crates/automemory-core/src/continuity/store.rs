//! Brief items in SQLite. Same rules as the rest of AutoMemory: no duplicates (a repeated item updates the old one),
//! secrets never stored, agents cannot overwrite what a person wrote.
use super::model::*;
use crate::db::Database;
use crate::redaction::redact_text;
use anyhow::{anyhow, bail, Result};
use chrono::{DateTime, Utc};
use rusqlite::params;
use std::str::FromStr;
use uuid::Uuid;

const COLS: &str = "id, project_id, kind, text, reason, ref_id, origin, author, session, state, status, created_at, updated_at";

use crate::text::clip;

/// Lower-case, collapsed whitespace, no trailing punctuation: "Use SQLite." and "use  sqlite" are one item.
fn normal(s: &str) -> String {
    s.split_whitespace().collect::<Vec<_>>().join(" ").to_lowercase().trim_end_matches(['.', '!', ';', ',']).to_string()
}

fn when(s: String) -> DateTime<Utc> {
    DateTime::parse_from_rfc3339(&s).map(|d| d.with_timezone(&Utc)).unwrap_or_else(|_| Utc::now())
}

fn row(r: &rusqlite::Row) -> rusqlite::Result<BriefItem> {
    let parse = |i: usize, _name: &str| -> rusqlite::Result<String> { r.get::<_, String>(i) };
    Ok(BriefItem {
        id: r.get(0)?,
        project_id: r.get(1)?,
        kind: ItemKind::from_str(&parse(2, "kind")?).unwrap_or(ItemKind::Status),
        text: r.get(3)?,
        reason: r.get(4)?,
        ref_id: r.get(5)?,
        origin: Origin::from_str(&parse(6, "origin")?).unwrap_or(Origin::Agent),
        author: r.get(7)?,
        session: r.get(8)?,
        state: ItemState::from_str(&parse(9, "state")?).unwrap_or(ItemState::Draft),
        status: ItemStatus::from_str(&parse(10, "status")?).unwrap_or(ItemStatus::Active),
        created_at: when(r.get(11)?),
        updated_at: when(r.get(12)?),
    })
}

impl Database {
    pub fn brief_get(&self, id: &str) -> Result<Option<BriefItem>> {
        let conn = self.conn();
        let mut stmt = conn.prepare(&format!("SELECT {COLS} FROM brief_items WHERE id = ?1"))?;
        let mut rows = stmt.query_map([id], row)?;
        Ok(rows.next().transpose()?)
    }

    /// Items of a project (and project-less ones), newest first. `all` includes resolved and removed.
    pub fn brief_list(&self, project_id: Option<&str>, all: bool) -> Result<Vec<BriefItem>> {
        let conn = self.conn();
        let mut stmt = conn.prepare(&format!(
            "SELECT {COLS} FROM brief_items WHERE (?1 IS NULL OR project_id = ?1 OR project_id IS NULL) AND (?2 OR status = 'active') ORDER BY updated_at DESC"
        ))?;
        let rows = stmt.query_map(params![project_id, all], row)?.collect::<std::result::Result<Vec<_>, _>>()?;
        Ok(rows)
    }

    /// Adds an item, or refreshes the identical active one (and promotes a draft to confirmed when a writer repeats it).
    /// Returns the item and whether it is new. A new goal or status retires the previous one.
    pub fn brief_add(&self, new: NewItem) -> Result<(BriefItem, bool)> {
        let text = redact_text(&clip(&new.text, TEXT_MAX));
        if text.is_empty() {
            bail!("the item is empty");
        }
        let reason = new.reason.as_deref().map(|r| redact_text(&clip(r, REASON_MAX))).filter(|r| !r.is_empty());
        let active = self.brief_list(new.project_id.as_deref(), false)?;
        let same_project = |i: &BriefItem| i.project_id == new.project_id;
        if let Some(existing) = active.iter().find(|i| same_project(i) && i.kind == new.kind && normal(&i.text) == normal(&text)) {
            let mut item = existing.clone();
            item.updated_at = Utc::now();
            if item.reason.is_none() {
                item.reason = reason;
            }
            if item.state == ItemState::Draft && new.origin != Origin::Transcript {
                item.state = ItemState::Confirmed;
            }
            self.brief_save(&item)?;
            return Ok((item, false));
        }
        if new.kind.is_singleton() {
            for old in active.iter().filter(|i| same_project(i) && i.kind == new.kind) {
                self.brief_set_status(&old.id, ItemStatus::Resolved)?;
            }
        } else if active.iter().filter(|i| same_project(i)).count() >= ACTIVE_MAX {
            bail!("the brief has {ACTIVE_MAX} active items: resolve or remove some first");
        }
        let now = Utc::now();
        let item = BriefItem {
            id: format!("brief-{}", Uuid::now_v7().simple()),
            project_id: new.project_id,
            kind: new.kind,
            text,
            reason,
            ref_id: new.ref_id,
            origin: new.origin,
            author: new.author,
            session: new.session,
            state: if new.origin == Origin::Transcript { ItemState::Draft } else { ItemState::Confirmed },
            status: ItemStatus::Active,
            created_at: now,
            updated_at: now,
        };
        self.brief_save(&item)?;
        Ok((item, true))
    }

    /// Insert or replace one item exactly as given (sync and edits).
    pub fn brief_save(&self, i: &BriefItem) -> Result<()> {
        self.conn().execute(
            &format!("INSERT OR REPLACE INTO brief_items ({COLS}) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13)"),
            params![
                i.id, i.project_id, i.kind.as_str(), i.text, i.reason, i.ref_id, i.origin.as_str(), i.author, i.session,
                i.state.as_str(), i.status.as_str(), i.created_at.to_rfc3339(), i.updated_at.to_rfc3339()
            ],
        )?;
        Ok(())
    }

    /// Same as `brief_save` (the sync macro wants a method taking the record by reference).
    pub fn brief_save_ref(&self, i: &BriefItem) -> Result<()> {
        self.brief_save(i)
    }

    pub fn brief_set_status(&self, id: &str, status: ItemStatus) -> Result<bool> {
        let n = self.conn().execute("UPDATE brief_items SET status = ?1, updated_at = ?2 WHERE id = ?3", params![status.as_str(), Utc::now().to_rfc3339(), id])?;
        Ok(n > 0)
    }

    /// Someone confirms a draft or an item they did not write. A person's confirmation is final.
    pub fn brief_confirm(&self, id: &str) -> Result<BriefItem> {
        let mut item = self.brief_get(id)?.ok_or_else(|| anyhow!("no such item"))?;
        if item.state == ItemState::Draft {
            item.state = ItemState::Confirmed;
            item.updated_at = Utc::now();
            self.brief_save(&item)?;
        }
        Ok(item)
    }

    /// Changes the wording. A person's change marks it `edited` (agents may not touch those afterwards).
    pub fn brief_correct(&self, id: &str, text: &str, reason: Option<&str>, by_human: bool, author: &str) -> Result<BriefItem> {
        let mut item = self.brief_get(id)?.ok_or_else(|| anyhow!("no such item"))?;
        let text = redact_text(&clip(text, TEXT_MAX));
        if text.is_empty() {
            bail!("the new text is empty");
        }
        item.text = text;
        if let Some(r) = reason {
            item.reason = Some(redact_text(&clip(r, REASON_MAX))).filter(|r| !r.is_empty());
        }
        item.state = if by_human { ItemState::Edited } else { ItemState::Confirmed };
        if !by_human {
            item.author = author.to_string();
        }
        item.updated_at = Utc::now();
        self.brief_save(&item)?;
        Ok(item)
    }

    /// Deletes every item of a project (or all of them). Returns how many.
    pub fn brief_forget(&self, project_id: Option<&str>) -> Result<usize> {
        Ok(self.conn().execute("DELETE FROM brief_items WHERE ?1 IS NULL OR project_id = ?1", params![project_id])?)
    }
}

/// Whether `writer` (an agent name, or "human") may change `item`: nobody overwrites a person's words.
pub fn may_change(item: &BriefItem, writer_is_human: bool) -> bool {
    writer_is_human || (item.origin != Origin::Human && item.state != ItemState::Edited)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn new(kind: ItemKind, text: &str) -> NewItem {
        NewItem { project_id: Some("p".into()), kind, text: text.into(), reason: None, ref_id: None, origin: Origin::Agent, author: "claude".into(), session: Some("s1".into()) }
    }

    #[test]
    fn a_repeated_item_updates_the_old_one_instead_of_duplicating() {
        let db = Database::open_in_memory().unwrap();
        let (a, created) = db.brief_add(new(ItemKind::Open, "Which cache TTL?")).unwrap();
        assert!(created);
        let (b, created) = db.brief_add(new(ItemKind::Open, "which  cache ttl?  ")).unwrap();
        assert!(!created && a.id == b.id);
        db.brief_add(new(ItemKind::Next, "Which cache TTL?")).unwrap();
        assert_eq!(db.brief_list(Some("p"), false).unwrap().len(), 2, "same words under another kind is another item");
    }

    #[test]
    fn a_new_goal_retires_the_old_one_and_history_is_kept() {
        let db = Database::open_in_memory().unwrap();
        db.brief_add(new(ItemKind::Goal, "Build the cart")).unwrap();
        db.brief_add(new(ItemKind::Goal, "Build the cart and checkout")).unwrap();
        let active: Vec<_> = db.brief_list(Some("p"), false).unwrap();
        assert_eq!(active.len(), 1);
        assert_eq!(active[0].text, "Build the cart and checkout");
        assert_eq!(db.brief_list(Some("p"), true).unwrap().len(), 2);
    }

    #[test]
    fn secrets_are_removed_and_text_is_capped() {
        let db = Database::open_in_memory().unwrap();
        let (i, _) = db.brief_add(new(ItemKind::Next, "Use key sk-abcdefghijklmnopqrstuvwxyz1234 for the API")).unwrap();
        assert!(!i.text.contains("abcdefghijklmnopqrstuvwxyz"), "{}", i.text);
        let (long, _) = db.brief_add(new(ItemKind::Next, &"x".repeat(2000))).unwrap();
        assert!(long.text.chars().count() <= TEXT_MAX + 1);
        assert!(db.brief_add(new(ItemKind::Next, "   ")).is_err());
    }

    #[test]
    fn drafts_are_confirmed_by_a_repeat_or_a_confirm_and_a_person_edit_is_protected() {
        let db = Database::open_in_memory().unwrap();
        let mut d = new(ItemKind::Rejected, "Server-side sessions");
        d.origin = Origin::Transcript;
        d.reason = Some("needs sticky routing".into());
        let (draft, _) = db.brief_add(d).unwrap();
        assert_eq!(draft.state, ItemState::Draft);
        assert_eq!(db.brief_confirm(&draft.id).unwrap().state, ItemState::Confirmed);

        let (item, _) = db.brief_add(new(ItemKind::Next, "Write the migration")).unwrap();
        let edited = db.brief_correct(&item.id, "Write the migration with a rollback", None, true, "human").unwrap();
        assert_eq!(edited.state, ItemState::Edited);
        assert!(!may_change(&edited, false), "agents cannot touch an item a person edited");
        assert!(may_change(&edited, true));
        let human = { let mut h = new(ItemKind::Constraint, "No new services"); h.origin = Origin::Human; h.author = "human".into(); db.brief_add(h).unwrap().0 };
        assert!(!may_change(&human, false), "nor one a person wrote");
    }

    #[test]
    fn the_brief_is_bounded() {
        let db = Database::open_in_memory().unwrap();
        for i in 0..ACTIVE_MAX {
            db.brief_add(new(ItemKind::Next, &format!("step {i}"))).unwrap();
        }
        assert!(db.brief_add(new(ItemKind::Next, "one more")).is_err());
        assert_eq!(db.brief_forget(Some("p")).unwrap(), ACTIVE_MAX);
    }
}
