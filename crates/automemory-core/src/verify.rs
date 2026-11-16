use crate::models::{Memory, MemoryStatus, MemoryType};
use crate::retrieval::{MemoryDecayConfig, MemoryDecayScorer};

/// Verification status returned on recall (PRD §8 Verify).
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum VerifyStatus {
    Verified,
    Changed,
    Stale,
    Unverifiable,
}

impl std::fmt::Display for VerifyStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Verified => write!(f, "verified"),
            Self::Changed => write!(f, "changed"),
            Self::Stale => write!(f, "stale"),
            Self::Unverifiable => write!(f, "unverifiable"),
        }
    }
}

/// Versioned Verifier port (PRD §7.1). Default impl below; replacements must
/// keep `api_version` in sync so the host can refuse mismatches.
pub trait Verifier {
    fn api_version(&self) -> &'static str {
        "1"
    }
    fn verify(&self, memory: &Memory) -> VerifyStatus;
}

/// First-guess half-lives by kind, replacing one 14-day half-life for everything (a 0.72 note went `stale` after about a month
/// whatever it said). To be confirmed on real notes: the research found decay as a score multiplier hurt, and staleness should come from
/// verification against the code first, age second.
const LONG_HALF_LIFE_DAYS: f64 = 90.0;
const FACT_HALF_LIFE_DAYS: f64 = 30.0;

/// Default verifier: commit linkage + decay + trust-tier check.
/// `Changed` comes from the PostCommit marker (linkage changed); per-symbol
/// content hashes refine it at PostCommit (`flag_memories_changed_by_files_with`): a memory that recorded its symbols' hashes is only
/// marked `changed` when one of them changed.
#[derive(Default)]
pub struct DecayVerifier;

impl Verifier for DecayVerifier {
    fn verify(&self, memory: &Memory) -> VerifyStatus {
        if memory.status == MemoryStatus::Pinned {
            return VerifyStatus::Verified;
        }
        if memory.is_quarantined() {
            return VerifyStatus::Unverifiable;
        }
        if memory.status == MemoryStatus::Changed {
            return VerifyStatus::Changed;
        }
        // Age alone does not make a decision or a person's own note stale; only a commit that changed its code does (status Changed).
        let half_life = match memory.memory_type {
            _ if memory.is_person_authored() => return VerifyStatus::Verified,
            MemoryType::Decision | MemoryType::Preference => return VerifyStatus::Verified,
            MemoryType::Lesson | MemoryType::Warning | MemoryType::Pattern | MemoryType::Skill => LONG_HALF_LIFE_DAYS,
            MemoryType::Fact | MemoryType::Observation => FACT_HALF_LIFE_DAYS,
        };
        let eval = MemoryDecayScorer::new(MemoryDecayConfig { half_life_days: half_life, ..MemoryDecayConfig::default() }).evaluate(
            memory.importance,
            memory.confidence,
            memory.updated_at,
            memory.version.unwrap_or(1).max(1) as u32,
            false,
            None,
        );
        if eval.is_stale {
            VerifyStatus::Stale
        } else {
            VerifyStatus::Verified
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::{MemoryScope, MemorySourceKind, MemoryType};
    use chrono::{Duration, Utc};
    use uuid::Uuid;

    fn mem(status: MemoryStatus, updated_at: chrono::DateTime<Utc>) -> Memory {
        Memory {
            id: Uuid::now_v7().to_string(),
            scope: MemoryScope::Project,
            project_id: Some("p".to_string()),
            memory_type: MemoryType::Fact,
            content: "c".to_string(),
            summary: "s".to_string(),
            confidence: 0.9,
            importance: 0.8,
            freshness: 1.0,
            status,
            source_kind: MemorySourceKind::Agent,
            version: Some(1),
            created_at: updated_at,
            updated_at,
            last_recalled_at: None,
            tags: vec![],
            entities: vec![],
            metadata: serde_json::json!({}),
        }
    }

    #[test]
    fn fresh_memory_verifies() {
        let v = DecayVerifier;
        assert_eq!(
            v.verify(&mem(MemoryStatus::Active, Utc::now())),
            VerifyStatus::Verified
        );
    }

    #[test]
    fn stale_memory_flags() {
        let v = DecayVerifier;
        let old = mem(MemoryStatus::Active, Utc::now() - Duration::days(120));
        assert_eq!(v.verify(&old), VerifyStatus::Stale);
    }

    #[test]
    fn quarantined_is_unverifiable_and_pinned_is_verified() {
        let v = DecayVerifier;
        assert_eq!(
            v.verify(&mem(MemoryStatus::Candidate, Utc::now())),
            VerifyStatus::Unverifiable
        );
        assert_eq!(
            v.verify(&mem(MemoryStatus::Pinned, Utc::now())),
            VerifyStatus::Verified
        );
    }

    #[test]
    fn commit_marked_memory_reports_changed() {
        let v = DecayVerifier;
        assert_eq!(
            v.verify(&mem(MemoryStatus::Changed, Utc::now())),
            VerifyStatus::Changed
        );
    }

    fn days_until_stale(kind: MemoryType) -> Option<i64> {
        let v = DecayVerifier;
        (0..2000).find(|d| {
            let mut m = mem(MemoryStatus::Active, Utc::now() - Duration::days(*d));
            m.memory_type = kind;
            v.verify(&m) == VerifyStatus::Stale
        })
    }

    /// Before: every unverified note was flagged `stale` after about 31 days, whatever it said. Now it depends on the kind.
    #[test]
    fn age_flags_depend_on_the_kind_of_note() {
        assert_eq!(days_until_stale(MemoryType::Decision), None, "a decision is never stale by age");
        assert_eq!(days_until_stale(MemoryType::Preference), None);
        let fact = days_until_stale(MemoryType::Fact).unwrap();
        let lesson = days_until_stale(MemoryType::Lesson).unwrap();
        assert!((60..=80).contains(&fact), "fact {fact}");
        assert!((190..=230).contains(&lesson), "lesson {lesson}");
    }
}
