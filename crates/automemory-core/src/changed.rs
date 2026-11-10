//! "What changed": decisions recorded or superseded, and memories added or no longer active, since a point in time.
//!
//! No extra columns: a decision is valid from `created_at`; it stops being valid when it is superseded, and that is the
//! last time its row changed (`updated_at`). A memory that is not `active` any more stopped being used at its last change.
use crate::db::Database;
use crate::models::{Decision, DecisionStatus, Memory, MemoryStatus};
use anyhow::Result;
use chrono::{DateTime, Duration, Utc};

/// `30m`, `2h`, `7d` or `2w` as a length of time.
pub fn parse_since(text: &str) -> Option<Duration> {
    let t = text.trim();
    let (num, unit) = t.split_at(t.len().checked_sub(1)?);
    let n: i64 = num.parse().ok()?;
    match unit {
        "m" => Some(Duration::minutes(n)),
        "h" => Some(Duration::hours(n)),
        "d" => Some(Duration::days(n)),
        "w" => Some(Duration::weeks(n)),
        _ => None,
    }
}

#[derive(Debug, Default)]
pub struct Changed {
    pub decisions_recorded: Vec<Decision>,
    pub decisions_superseded: Vec<Decision>,
    pub memories_added: Vec<Memory>,
    pub memories_inactive: Vec<Memory>,
}

impl Decision {
    /// When the decision took effect.
    pub fn valid_from(&self) -> DateTime<Utc> {
        self.created_at
    }

    /// When it stopped being the decision in force; `None` while it is.
    pub fn invalid_at(&self) -> Option<DateTime<Utc>> {
        (self.status == DecisionStatus::Superseded).then_some(self.updated_at)
    }

    /// The decision as JSON with its validity dates, for tools and the CLI.
    pub fn dated_json(&self) -> serde_json::Value {
        let mut v = serde_json::to_value(self).unwrap_or_default();
        if let Some(o) = v.as_object_mut() {
            o.insert("valid_from".into(), self.valid_from().to_rfc3339().into());
            o.insert("invalid_at".into(), self.invalid_at().map(|d| d.to_rfc3339()).into());
        }
        v
    }
}

pub fn changed_since(db: &Database, project_id: Option<&str>, since: DateTime<Utc>) -> Result<Changed> {
    let mut out = Changed::default();
    for d in db.list_decisions(project_id, None, 1000)? {
        if d.created_at >= since {
            out.decisions_recorded.push(d.clone());
        }
        if d.invalid_at().is_some_and(|t| t >= since) {
            out.decisions_superseded.push(d);
        }
    }
    for m in db.list_memories(project_id, None, None, 5000)? {
        if m.created_at >= since {
            out.memories_added.push(m.clone());
        }
        if !matches!(m.status, MemoryStatus::Active | MemoryStatus::Pinned | MemoryStatus::Candidate | MemoryStatus::Review) && m.updated_at >= since {
            out.memories_inactive.push(m);
        }
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::DecisionRecordArgs;

    fn record(db: &Database, topic: &str, decision: &str) -> Decision {
        db.record_decision(DecisionRecordArgs {
            project_id: None,
            topic,
            decision,
            reason: "because",
            alternatives: &[],
            dissent: None,
            agent: "t",
            symbols: &[],
            evidence: &[],
        })
        .unwrap()
    }

    #[test]
    fn search_never_returns_a_memory_that_is_no_longer_active() {
        use crate::models::{Memory, MemoryScope, MemorySourceKind, MemoryType};
        let db = Database::open_in_memory().unwrap();
        let mk = |id: &str, summary: &str| Memory {
            id: id.into(),
            scope: MemoryScope::Global,
            project_id: None,
            memory_type: MemoryType::Fact,
            content: summary.into(),
            summary: summary.into(),
            confidence: 0.9,
            importance: 0.5,
            freshness: 1.0,
            status: MemoryStatus::Active,
            source_kind: MemorySourceKind::User,
            version: Some(1),
            created_at: Utc::now(),
            updated_at: Utc::now(),
            last_recalled_at: None,
            tags: vec![],
            entities: vec![],
            metadata: serde_json::json!({}),
        };
        db.save_memory(&mk("m-keep", "the zebra pattern is used for retries")).unwrap();
        db.save_memory(&mk("m-gone", "the zebra pattern is used for caching")).unwrap();
        assert_eq!(db.search_memories_fts("zebra", None, 10).unwrap().len(), 2);
        db.archive_memory("m-gone").unwrap();
        let hits = db.search_memories_fts("zebra", None, 10).unwrap();
        assert_eq!(hits.len(), 1);
        assert_eq!(hits[0].0.id, "m-keep");
        let c = changed_since(&db, None, Utc::now() - Duration::hours(1)).unwrap();
        assert_eq!(c.memories_inactive.len(), 1);
        assert_eq!(c.memories_inactive[0].id, "m-gone");
    }

    #[test]
    fn parses_short_durations() {
        assert_eq!(parse_since("90m"), Some(Duration::minutes(90)));
        assert_eq!(parse_since("7d"), Some(Duration::days(7)));
        assert_eq!(parse_since("2w"), Some(Duration::weeks(2)));
        assert!(parse_since("soon").is_none() && parse_since("").is_none());
    }

    #[test]
    fn a_superseded_decision_stays_readable_with_both_dates() {
        let db = Database::open_in_memory().unwrap();
        let first = record(&db, "cache", "use redis");
        let second = record(&db, "cache", "use sqlite");
        let got = db.get_decision(None, "cache").unwrap();
        assert_eq!(got.active.as_ref().unwrap().id, second.id);
        let old = got.history.iter().find(|d| d.id == first.id).expect("the old decision is still there");
        let j = old.dated_json();
        assert_eq!(j["decision"], "use redis");
        assert!(j["valid_from"].is_string() && j["invalid_at"].is_string(), "{j}");
        assert!(got.active.unwrap().dated_json()["invalid_at"].is_null(), "the one in force has no end date");

        let c = changed_since(&db, None, Utc::now() - Duration::hours(1)).unwrap();
        assert_eq!(c.decisions_recorded.len(), 2);
        assert_eq!(c.decisions_superseded.len(), 1);
        assert_eq!(c.decisions_superseded[0].id, first.id);
        assert!(changed_since(&db, None, Utc::now() + Duration::hours(1)).unwrap().decisions_recorded.is_empty());
    }
}
