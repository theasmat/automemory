use crate::models::{Memory, MemoryStatus};
use crate::retrieval::MemoryDecayScorer;

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

/// Default verifier: commit linkage + decay + trust-tier check.
/// `Changed` comes from the PostCommit marker (linkage changed); per-symbol
/// content hashes would refine it further (see CodeIntel::symbol_hash).
#[derive(Default)]
pub struct DecayVerifier {
    scorer: MemoryDecayScorer,
}

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
        let eval = self.scorer.evaluate(
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
        let v = DecayVerifier::default();
        assert_eq!(
            v.verify(&mem(MemoryStatus::Active, Utc::now())),
            VerifyStatus::Verified
        );
    }

    #[test]
    fn stale_memory_flags() {
        let v = DecayVerifier::default();
        let old = mem(MemoryStatus::Active, Utc::now() - Duration::days(120));
        assert_eq!(v.verify(&old), VerifyStatus::Stale);
    }

    #[test]
    fn quarantined_is_unverifiable_and_pinned_is_verified() {
        let v = DecayVerifier::default();
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
        let v = DecayVerifier::default();
        assert_eq!(
            v.verify(&mem(MemoryStatus::Changed, Utc::now())),
            VerifyStatus::Changed
        );
    }
}
