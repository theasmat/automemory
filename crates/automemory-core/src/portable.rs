//! Open export and import: your notes and decisions in a versioned JSON file you can read, keep, diff and move to another computer
//! or another tool, plus a Markdown folder (one file per note, YAML front matter) that opens in any editor.
//! The JSON carries what other tools drop: trust tier (in `metadata`), source, status and decision evidence.

use crate::db::Database;
use crate::models::{Decision, Memory};
use anyhow::{bail, Result};
use chrono::Utc;
use serde::{Deserialize, Serialize};
use std::path::Path;

pub const FORMAT: &str = "automemory.export";
pub const VERSION: u32 = 1;

#[derive(Debug, Serialize, Deserialize)]
pub struct Export {
    pub format: String,
    pub version: u32,
    pub exported_at: String,
    pub memories: Vec<Memory>,
    pub decisions: Vec<Decision>,
}

#[derive(Debug, Default, PartialEq, Eq)]
pub struct ImportReport {
    pub memories_added: usize,
    pub memories_skipped: usize,
    pub decisions_added: usize,
    pub decisions_skipped: usize,
}

/// Everything except archived notes' history and vectors (those are rebuilt), for one project or all.
pub fn export(db: &Database, project_id: Option<&str>) -> Result<Export> {
    Ok(Export {
        format: FORMAT.to_string(),
        version: VERSION,
        exported_at: Utc::now().to_rfc3339(),
        memories: db.list_memories(project_id, None, None, usize::MAX >> 1)?,
        decisions: db.list_decisions(project_id, None, usize::MAX >> 1)?,
    })
}

/// Adds what the database does not have yet. A record whose id exists is left alone, never overwritten, so importing the same file twice
/// changes nothing and a stale file cannot undo newer edits.
pub fn import(db: &Database, file: &Export) -> Result<ImportReport> {
    if file.format != FORMAT {
        bail!("not an AutoMemory export (format {:?})", file.format);
    }
    if file.version > VERSION {
        bail!("this export is version {}; this AutoMemory reads up to {VERSION}. Update AutoMemory", file.version);
    }
    let mut r = ImportReport::default();
    for m in &file.memories {
        if db.get_memory_by_id(&m.id)?.is_some() {
            r.memories_skipped += 1;
        } else {
            db.save_memory_direct(m)?;
            r.memories_added += 1;
        }
    }
    for d in &file.decisions {
        if db.get_decision_by_id(&d.id)?.is_some() {
            r.decisions_skipped += 1;
        } else {
            db.save_decision_direct(d)?;
            r.decisions_added += 1;
        }
    }
    Ok(r)
}

fn slug(text: &str) -> String {
    let s: String = text.chars().map(|c| if c.is_alphanumeric() { c.to_ascii_lowercase() } else { '-' }).collect();
    let s = s.split('-').filter(|p| !p.is_empty()).collect::<Vec<_>>().join("-");
    s.chars().take(48).collect()
}

/// One Markdown file per note and decision under `dir/memories` and `dir/decisions`: YAML front matter, then the text. Lossy by design
/// (the JSON is the full record); it is for reading and for editors such as Obsidian.
pub fn write_markdown(file: &Export, dir: &Path) -> Result<usize> {
    std::fs::create_dir_all(dir.join("memories"))?;
    std::fs::create_dir_all(dir.join("decisions"))?;
    let q = |s: &str| serde_json::to_string(s).unwrap_or_default();
    for m in &file.memories {
        let tier = m.trust_tier().as_u8();
        let text = format!(
            "---\nid: {}\ntype: {}\nstatus: {}\ntrust_tier: {}\nsource: {}\nconfidence: {}\ncreated: {}\nupdated: {}\ntags: {}\n---\n\n# {}\n\n{}\n",
            m.id, m.memory_type, m.status, tier, m.source_kind, m.confidence, m.created_at.to_rfc3339(), m.updated_at.to_rfc3339(),
            serde_json::to_string(&m.tags)?, m.summary, m.content
        );
        std::fs::write(dir.join("memories").join(format!("{}-{}.md", slug(&m.summary), &m.id[m.id.len().saturating_sub(8)..])), text)?;
    }
    for d in &file.decisions {
        let text = format!(
            "---\nid: {}\ntopic: {}\nstatus: {}\nagent: {}\ncreated: {}\n---\n\n# {}\n\n{}\n\n**Why:** {}\n\n{}",
            d.id, q(&d.topic), d.status, q(&d.agent), d.created_at.to_rfc3339(), d.topic, d.decision, d.reason,
            if d.alternatives.is_empty() { String::new() } else { format!("**Rejected:** {}\n", d.alternatives.join("; ")) }
        );
        std::fs::write(dir.join("decisions").join(format!("{}-{}.md", slug(&d.topic), &d.id[d.id.len().saturating_sub(8)..])), text)?;
    }
    Ok(file.memories.len() + file.decisions.len())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::*;

    fn note(id: &str, tier: u8) -> Memory {
        let mut m = Memory {
            id: id.into(), scope: MemoryScope::Global, project_id: None, memory_type: MemoryType::Decision, content: format!("content {id}"),
            summary: format!("summary {id}"), confidence: 0.9, importance: 0.5, freshness: 1.0, status: MemoryStatus::Active,
            source_kind: MemorySourceKind::User, version: Some(1), created_at: Utc::now(), updated_at: Utc::now(), last_recalled_at: None,
            tags: vec!["a".into()], entities: vec![], metadata: serde_json::json!({}),
        };
        m.set_trust_tier(TrustTier::from_u8(tier));
        m
    }

    #[test]
    fn export_then_import_is_lossless_and_repeatable() {
        let a = Database::open_in_memory().unwrap();
        a.save_memory_direct(&note("m1", 1)).unwrap();
        a.save_memory_direct(&note("m2", 4)).unwrap();
        a.record_decision(DecisionRecordArgs { project_id: None, topic: "db", decision: "sqlx", reason: "checked queries", alternatives: &["diesel".into()], dissent: None, agent: "claude", symbols: &[], evidence: &[] }).unwrap();
        let out = export(&a, None).unwrap();
        let json = serde_json::to_string(&out).unwrap();

        let b = Database::open_in_memory().unwrap();
        let back: Export = serde_json::from_str(&json).unwrap();
        let r = import(&b, &back).unwrap();
        assert_eq!((r.memories_added, r.decisions_added), (out.memories.len(), 1), "a decision also keeps its own note");
        assert_eq!(b.get_memory_by_id("m1").unwrap().unwrap().trust_tier().as_u8(), 1, "tier survives");
        assert_eq!(b.get_memory_by_id("m2").unwrap().unwrap().trust_tier().as_u8(), 4);
        assert_eq!(export(&b, None).unwrap().memories.len(), out.memories.len());
        assert_eq!(import(&b, &back).unwrap(), ImportReport { memories_skipped: out.memories.len(), decisions_skipped: 1, ..Default::default() }, "again changes nothing");
    }

    #[test]
    fn a_foreign_or_newer_file_is_refused() {
        let db = Database::open_in_memory().unwrap();
        let mut e = export(&db, None).unwrap();
        e.format = "other".into();
        assert!(import(&db, &e).is_err());
        e.format = FORMAT.into();
        e.version = VERSION + 1;
        assert!(import(&db, &e).unwrap_err().to_string().contains("Update AutoMemory"));
    }

    #[test]
    fn markdown_has_front_matter_and_one_file_per_record() {
        let db = Database::open_in_memory().unwrap();
        db.save_memory_direct(&note("m1", 1)).unwrap();
        let dir = std::env::temp_dir().join(format!("am-md-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        assert_eq!(write_markdown(&export(&db, None).unwrap(), &dir).unwrap(), 1);
        let f = std::fs::read_dir(dir.join("memories")).unwrap().next().unwrap().unwrap().path();
        let text = std::fs::read_to_string(f).unwrap();
        assert!(text.starts_with("---\nid: m1\n") && text.contains("trust_tier: 1") && text.contains("# summary m1"));
        let _ = std::fs::remove_dir_all(&dir);
    }
}
