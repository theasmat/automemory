//! The nightly tidy: look through the notes for what a person would want to clear out, and put each finding in the Inbox. It never changes a note
//! by itself: archiving is a person's call, and everything is reversible. Deterministic, no model.
//!   * the same note twice (near-identical words in one project): keep the better-trusted, older one, offer to archive the other;
//!   * a note that names files that no longer exist in the project: offer to archive it;
//!   * notes nobody has used for two months: the existing offers (`retrieval::fade`).

use crate::continuity::conflict::terms;
use crate::db::Database;
use crate::models::{Memory, MemoryStatus, MemoryType, ReviewProposal};
use anyhow::Result;
use chrono::Utc;
use std::path::Path;

/// New Inbox items per run, so a first run on a big database does not bury the Inbox.
const MAX_NEW: usize = 20;
/// Share of the smaller note's words that the other note must also use.
const SAME: f64 = 0.8;

#[derive(Debug, Default, PartialEq, Eq)]
pub struct TidyReport {
    pub duplicates: Vec<(String, String)>,
    pub missing_files: Vec<(String, Vec<String>)>,
    pub unused_offered: usize,
    pub filed: usize,
}

fn rank(m: &Memory) -> (u8, chrono::DateTime<Utc>) {
    // better trusted first (tier 1 best), then older first
    (m.trust_tier().as_u8(), m.created_at)
}

fn similar(a: &[String], b: &[String]) -> bool {
    let (small, big) = if a.len() <= b.len() { (a, b) } else { (b, a) };
    if small.len() < 4 {
        return false;
    }
    small.iter().filter(|t| big.contains(t)).count() as f64 / small.len() as f64 >= SAME
}

fn exists_once(db: &Database, id: &str) -> bool {
    db.get_proposal_by_id(id).ok().flatten().is_some()
}

#[allow(clippy::too_many_arguments)]
fn file(db: &Database, id: String, project: Option<String>, ptype: &str, target: &str, description: String, action: &str, rep: &mut TidyReport, dry: bool) -> Result<()> {
    if rep.filed >= MAX_NEW || exists_once(db, &id) {
        return Ok(());
    }
    if !dry {
        db.save_proposal(&ReviewProposal {
            id,
            project_id: project,
            proposal_type: ptype.into(),
            target_id: target.into(),
            description,
            suggested_action: action.into(),
            status: "pending".into(),
            created_at: Utc::now(),
            decided_at: None,
        })?;
    }
    rep.filed += 1;
    Ok(())
}

/// Runs the pass over one project (or all). `dry` reports without filing anything.
pub fn run(db: &Database, project_id: Option<&str>, dry: bool) -> Result<TidyReport> {
    let mut rep = TidyReport::default();
    let notes: Vec<Memory> = db
        .list_memories(project_id, Some(MemoryStatus::Active), None, 5000)?
        .into_iter()
        .filter(|m| m.memory_type != MemoryType::Decision && !m.summary.starts_with("Session checkpoint"))
        .collect();

    // 1. duplicates: only within one project, only among notes in the same words
    let words: Vec<Vec<String>> = notes.iter().map(|m| terms(&format!("{} {}", m.summary, m.content))).collect();
    let mut done: Vec<&str> = Vec::new();
    for i in 0..notes.len() {
        for j in (i + 1)..notes.len() {
            let (a, b) = (&notes[i], &notes[j]);
            if a.project_id != b.project_id || done.contains(&a.id.as_str()) || done.contains(&b.id.as_str()) || !similar(&words[i], &words[j]) {
                continue;
            }
            let (keep, drop) = if rank(a) <= rank(b) { (a, b) } else { (b, a) };
            if drop.is_person_authored() {
                continue; // never offer to archive what a person wrote
            }
            done.push(drop.id.as_str());
            rep.duplicates.push((keep.id.clone(), drop.id.clone()));
            file(
                db,
                format!("dup-{}", drop.id),
                drop.project_id.clone(),
                "duplicate_merge",
                &drop.id,
                format!("This note repeats another one: \"{}\" says nearly the same as \"{}\". Archive the repeat?", drop.summary.chars().take(100).collect::<String>(), keep.summary.chars().take(100).collect::<String>()),
                "Approve to archive the repeat (restorable), reject to keep both",
                &mut rep,
                dry,
            )?;
        }
    }

    // 2. notes that name only files which are gone
    let mut roots: std::collections::HashMap<String, Vec<String>> = std::collections::HashMap::new();
    for m in &notes {
        let Some(pid) = m.project_id.as_deref() else { continue };
        let Some(files) = m.metadata.get("files_modified").and_then(|v| v.as_array()) else { continue };
        let files: Vec<String> = files.iter().filter_map(|f| f.as_str().map(String::from)).collect();
        if files.is_empty() {
            continue;
        }
        let paths = roots.entry(pid.to_string()).or_insert_with(|| db.get_project_paths(pid).unwrap_or_default().into_iter().map(|p| p.local_path).collect());
        // only judge on a machine where the project folder exists, or every file would look gone
        let Some(root) = paths.iter().find(|p| Path::new(p).is_dir()) else { continue };
        let gone: Vec<String> = files.iter().filter(|f| !Path::new(root).join(f).exists()).cloned().collect();
        if gone.len() == files.len() {
            rep.missing_files.push((m.id.clone(), gone.clone()));
            file(
                db,
                format!("gone-{}", m.id),
                m.project_id.clone(),
                "stale_note",
                &m.id,
                format!("\"{}\" is about files that no longer exist ({}). Archive it?", m.summary.chars().take(100).collect::<String>(), gone.iter().take(3).cloned().collect::<Vec<_>>().join(", ")),
                "Approve to archive it (restorable), reject to keep it",
                &mut rep,
                dry,
            )?;
        }
    }

    // 3. unused notes (the same offers a session start makes)
    if !dry {
        rep.unused_offered = crate::retrieval::fade::offer_stale_notes(db, project_id)?;
        rep.filed += rep.unused_offered;
    }
    Ok(rep)
}

/// For the daemon: runs the pass when the last one was over 20 hours ago. Returns a line for the log.
pub fn nightly(db: &Database) -> Option<String> {
    let now = Utc::now().timestamp();
    let last = db.sync_kv_get("tidy_at").and_then(|v| v.parse::<i64>().ok()).unwrap_or(0);
    if now - last < 20 * 3600 {
        return None;
    }
    db.sync_kv_set("tidy_at", &now.to_string());
    Some(match run(db, None, false) {
        Ok(r) => format!("tidy: {} item(s) filed in the Inbox ({} repeats, {} about missing files, {} unused)", r.filed, r.duplicates.len(), r.missing_files.len(), r.unused_offered),
        Err(e) => format!("tidy failed: {e}"),
    })
}

/// Days since the last pass, if there ever was one.
pub fn days_since(db: &Database) -> Option<i64> {
    let last = db.sync_kv_get("tidy_at")?.parse::<i64>().ok()?;
    Some((Utc::now().timestamp() - last) / 86400)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::*;


    fn project(db: &Database, dir: &std::path::Path) {
        let now = Utc::now();
        db.save_project(&crate::models::Project {
            id: "p1".into(), slug: "p1".into(), display_name: "p1".into(), created_at: now, updated_at: now, capture_policy: Default::default(),
            context_budget: Default::default(), document_config: Default::default(), sync_config: Default::default(),
        })
        .unwrap();
        db.add_project_path("p1", &dir.to_string_lossy(), true).unwrap();
    }

    fn note(db: &Database, id: &str, summary: &str, content: &str, kind: MemorySourceKind, files: Option<Vec<&str>>) {
        let meta = files.map(|f| serde_json::json!({ "files_modified": f })).unwrap_or_else(|| serde_json::json!({}));
        db.save_memory_direct(&Memory {
            id: id.into(), scope: MemoryScope::Project, project_id: Some("p1".into()), memory_type: MemoryType::Fact, content: content.into(), summary: summary.into(),
            confidence: 0.9, importance: 0.5, freshness: 1.0, status: MemoryStatus::Active, source_kind: kind, version: Some(1),
            created_at: Utc::now() - chrono::Duration::minutes(if id == "a" { 10 } else { 0 }), updated_at: Utc::now(), last_recalled_at: Some(Utc::now()), tags: vec![], entities: vec![], metadata: meta,
        })
        .unwrap();
    }

    #[test]
    fn repeats_are_offered_never_changed_and_a_persons_note_is_never_the_one_dropped() {
        let db = Database::open_in_memory().unwrap();
        note(&db, "a", "Money is stored as integer cents", "Prices and totals are integer cents, never floats, in every table", MemorySourceKind::Agent, None);
        note(&db, "b", "Money stored as integer cents", "Prices and totals are integer cents never floats in every table", MemorySourceKind::Agent, None);
        note(&db, "c", "Staging deploys from release branches", "Cut a release branch and push; merging to main does not deploy", MemorySourceKind::Agent, None);
        let dry = run(&db, None, true).unwrap();
        assert_eq!(dry.duplicates, vec![("a".to_string(), "b".to_string())], "the older one stays");
        assert!(db.list_pending_proposals(None).unwrap().is_empty(), "a dry run files nothing");
        run(&db, None, false).unwrap();
        let pending = db.list_pending_proposals(None).unwrap();
        assert_eq!(pending.iter().filter(|p| p.proposal_type == "duplicate_merge" && p.target_id == "b").count(), 1);
        assert_eq!(db.get_memory_by_id("b").unwrap().unwrap().status, MemoryStatus::Active, "nothing was archived");
        run(&db, None, false).unwrap();
        assert_eq!(db.list_pending_proposals(None).unwrap().iter().filter(|p| p.proposal_type == "duplicate_merge").count(), 1, "a second run does not file it again");

        // a person's own note is never the one offered for archiving
        let db2 = Database::open_in_memory().unwrap();
        note(&db2, "a", "Money is stored as integer cents", "Prices and totals are integer cents, never floats, in every table", MemorySourceKind::Agent, None);
        note(&db2, "b", "Money stored as integer cents", "Prices and totals are integer cents never floats in every table", MemorySourceKind::User, None);
        let r = run(&db2, None, true).unwrap();
        assert_eq!(r.duplicates, vec![("b".to_string(), "a".to_string())], "the person's note is kept, the agent's repeat is the one offered");
    }

    #[test]
    fn a_note_about_files_that_are_gone_is_offered_but_only_where_the_folder_exists() {
        let dir = std::env::temp_dir().join(format!("am-tidy-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(dir.join("src")).unwrap();
        std::fs::write(dir.join("src/kept.rs"), "x").unwrap();
        let db = Database::open_in_memory().unwrap();
        project(&db, &dir);
        note(&db, "gone", "Refund handler lives in old module", "The refund handler sits in the legacy module", MemorySourceKind::Agent, Some(vec!["src/old.rs", "src/older.rs"]));
        note(&db, "live", "Cart totals", "Totals are computed in the kept module", MemorySourceKind::Agent, Some(vec!["src/kept.rs", "src/old.rs"]));
        let r = run(&db, None, false).unwrap();
        assert_eq!(r.missing_files.len(), 1);
        assert_eq!(r.missing_files[0].0, "gone");
        assert!(db.list_pending_proposals(None).unwrap().iter().any(|p| p.target_id == "gone" && p.proposal_type == "stale_note"));
        // on a machine where the folder does not exist nothing is judged
        std::fs::remove_dir_all(&dir).unwrap();
        let db2 = Database::open_in_memory().unwrap();
        project(&db2, &dir);
        note(&db2, "gone", "Refund handler lives in old module", "legacy", MemorySourceKind::Agent, Some(vec!["src/old.rs"]));
        assert!(run(&db2, None, true).unwrap().missing_files.is_empty());
    }

    #[test]
    fn the_nightly_pass_runs_about_once_a_day() {
        let db = Database::open_in_memory().unwrap();
        assert!(nightly(&db).unwrap().starts_with("tidy:"));
        assert!(nightly(&db).is_none());
        assert_eq!(days_since(&db), Some(0));
    }
}
