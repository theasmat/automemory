//! Cleaning up derived records (skills, traces, failures) that were written many times for the same thing, and the stable ids that keep new ones from piling up.
use crate::db::Database;
use anyhow::Result;
use serde::Serialize;
use sha2::{Digest, Sha256};

/// A stable id: the same input always gives the same id, so writing the same thing again updates one row instead of adding another.
pub fn stable_id(prefix: &str, parts: &[&str]) -> String {
    let mut h = Sha256::new();
    for p in parts {
        h.update(p.as_bytes());
        h.update([0u8]);
    }
    format!("{prefix}_{}", &format!("{:x}", h.finalize())[..20])
}

/// An error message reduced to what makes it the same error: lower case, numbers, hashes, paths and quoted names collapsed, first 160 characters.
pub fn error_signature(msg: &str) -> String {
    let mut out = String::new();
    let mut last_space = true;
    for word in msg.to_lowercase().split_whitespace() {
        let w = if word.chars().any(|c| c.is_ascii_digit()) || word.contains('/') || word.contains('\\') || word.len() > 40 { "#" } else { word };
        if w == "#" && out.ends_with('#') {
            continue;
        }
        if !last_space {
            out.push(' ');
        }
        out.push_str(w);
        last_space = false;
    }
    out.chars().take(160).collect()
}

#[derive(Debug, Default, Clone, Copy, Serialize, PartialEq, Eq)]
pub struct PruneCounts {
    pub skills: usize,
    pub traces: usize,
    pub failures: usize,
}

impl PruneCounts {
    pub fn total(&self) -> usize {
        self.skills + self.traces + self.failures
    }
}

// Rows to drop: every one but the newest (a resolved failure beats an unresolved one) in each group of identical ones.
const SKILLS_DUP: &str = "SELECT id FROM (SELECT id, ROW_NUMBER() OVER (PARTITION BY COALESCE(project_id, ''), name ORDER BY updated_at DESC, id) AS rn FROM procedural_skills) WHERE rn > 1";
const TRACES_DUP: &str = "SELECT id FROM (SELECT id, ROW_NUMBER() OVER (PARTITION BY COALESCE(project_id, ''), title, category, steps ORDER BY updated_at DESC, id) AS rn FROM success_traces) WHERE rn > 1";
const FAILURES_DUP: &str = "SELECT id FROM (SELECT id, ROW_NUMBER() OVER (PARTITION BY COALESCE(project_id, ''), COALESCE(failed_tool, ''), task_title, substr(error_message, 1, 120) ORDER BY (status = 'resolved') DESC, updated_at DESC, id) AS rn FROM failure_records) WHERE rn > 1";

impl Database {
    /// How many duplicate rows there are (nothing is changed).
    pub fn duplicates(&self) -> Result<PruneCounts> {
        let count = |sql: &str| -> Result<usize> { Ok(self.conn().query_row(&format!("SELECT COUNT(*) FROM ({sql})"), [], |r| r.get::<_, i64>(0))? as usize) };
        Ok(PruneCounts { skills: count(SKILLS_DUP)?, traces: count(TRACES_DUP)?, failures: count(FAILURES_DUP)? })
    }

    /// Deletes the duplicates, keeping the newest of each group. Returns what was removed.
    pub fn prune_duplicates(&self) -> Result<PruneCounts> {
        let before = self.duplicates()?;
        let conn = self.conn();
        conn.execute(&format!("DELETE FROM procedural_skills WHERE id IN ({SKILLS_DUP})"), [])?;
        conn.execute(&format!("DELETE FROM success_traces WHERE id IN ({TRACES_DUP})"), [])?;
        conn.execute(&format!("DELETE FROM failure_records WHERE id IN ({FAILURES_DUP})"), [])?;
        Ok(before)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::*;
    use chrono::Utc;

    #[test]
    fn stable_ids_and_error_signatures() {
        assert_eq!(stable_id("skill", &["p", "name"]), stable_id("skill", &["p", "name"]));
        assert_ne!(stable_id("skill", &["p", "ab"]), stable_id("skill", &["pa", "b"]), "parts are separated");
        assert_eq!(error_signature("error[E0432]: unresolved import `foo` at /Users/me/x.rs:12"), error_signature("error[E0599]: unresolved import `foo` at /home/you/y.rs:99"));
        assert_ne!(error_signature("connection refused"), error_signature("permission denied"));
    }

    fn skill(db: &Database, id: &str, name: &str, at: i64) {
        db.save_skill(&ProceduralSkill {
            id: id.into(), project_id: Some("p".into()), name: name.into(), description: "d".into(), trigger_patterns: vec![], preconditions: vec![], steps: vec!["1. run".into()],
            caveats: vec![], success_count: 1, failure_count: 0, source_trace_ids: vec![], created_at: Utc::now(), updated_at: Utc::now() + chrono::Duration::seconds(at),
        }).unwrap();
    }

    #[test]
    fn prune_keeps_the_newest_of_identical_rows_and_nothing_else() {
        let db = Database::open_in_memory().unwrap();
        for i in 0..5 {
            skill(&db, &format!("s{i}"), "Playbook: ship", i);
        }
        skill(&db, "other", "Playbook: test", 0);
        assert_eq!(db.duplicates().unwrap().skills, 4);
        let removed = db.prune_duplicates().unwrap();
        assert_eq!(removed.skills, 4);
        let left: Vec<String> = db.list_skills(None, 100).unwrap().into_iter().map(|s| s.id).collect();
        assert!(left.contains(&"s4".to_string()) && left.contains(&"other".to_string()) && left.len() == 2, "{left:?}");
        assert_eq!(db.prune_duplicates().unwrap().total(), 0, "running again removes nothing");
    }
}
