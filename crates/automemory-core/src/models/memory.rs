use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum MemoryScope {
    Global,
    Project,
}

impl std::fmt::Display for MemoryScope {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Global => write!(f, "global"),
            Self::Project => write!(f, "project"),
        }
    }
}

impl std::str::FromStr for MemoryScope {
    type Err = String;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "global" => Ok(Self::Global),
            "project" => Ok(Self::Project),
            other => Err(format!("Unknown memory scope: {other}")),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum MemoryType {
    Fact,
    Decision,
    Lesson,
    Pattern,
    Warning,
    Observation,
    Preference,
    Skill,
}

impl std::fmt::Display for MemoryType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Fact => write!(f, "fact"),
            Self::Decision => write!(f, "decision"),
            Self::Lesson => write!(f, "lesson"),
            Self::Pattern => write!(f, "pattern"),
            Self::Warning => write!(f, "warning"),
            Self::Observation => write!(f, "observation"),
            Self::Preference => write!(f, "preference"),
            Self::Skill => write!(f, "skill"),
        }
    }
}

impl std::str::FromStr for MemoryType {
    type Err = String;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "fact" => Ok(Self::Fact),
            "decision" => Ok(Self::Decision),
            "lesson" => Ok(Self::Lesson),
            "pattern" => Ok(Self::Pattern),
            "warning" => Ok(Self::Warning),
            "observation" => Ok(Self::Observation),
            "preference" => Ok(Self::Preference),
            "skill" => Ok(Self::Skill),
            other => Err(format!("Unknown memory type: {other}")),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum MemoryStatus {
    Active,
    Candidate,
    Pinned,
    Stale,
    Changed,
    Superseded,
    Archived,
    Invalid,
    Review,
}

impl std::fmt::Display for MemoryStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Active => write!(f, "active"),
            Self::Candidate => write!(f, "candidate"),
            Self::Pinned => write!(f, "pinned"),
            Self::Stale => write!(f, "stale"),
            Self::Changed => write!(f, "changed"),
            Self::Superseded => write!(f, "superseded"),
            Self::Archived => write!(f, "archived"),
            Self::Invalid => write!(f, "invalid"),
            Self::Review => write!(f, "review"),
        }
    }
}

impl std::str::FromStr for MemoryStatus {
    type Err = String;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "active" => Ok(Self::Active),
            "candidate" => Ok(Self::Candidate),
            "pinned" => Ok(Self::Pinned),
            "stale" => Ok(Self::Stale),
            "changed" => Ok(Self::Changed),
            "superseded" => Ok(Self::Superseded),
            "archived" => Ok(Self::Archived),
            "invalid" => Ok(Self::Invalid),
            "review" => Ok(Self::Review),
            other => Err(format!("Unknown memory status: {other}")),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MemorySourceKind {
    User,
    Agent,
    ToolEvent,
    TaskReflection,
    Document,
}

impl std::fmt::Display for MemorySourceKind {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::User => write!(f, "user"),
            Self::Agent => write!(f, "agent"),
            Self::ToolEvent => write!(f, "tool_event"),
            Self::TaskReflection => write!(f, "task_reflection"),
            Self::Document => write!(f, "document"),
        }
    }
}

impl std::str::FromStr for MemorySourceKind {
    type Err = String;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "user" => Ok(Self::User),
            "agent" => Ok(Self::Agent),
            "tool_event" => Ok(Self::ToolEvent),
            "task_reflection" => Ok(Self::TaskReflection),
            "document" => Ok(Self::Document),
            other => Err(format!("Unknown source kind: {other}")),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Memory {
    pub id: String,
    pub scope: MemoryScope,
    pub project_id: Option<String>,
    #[serde(rename = "type", alias = "memory_type")]
    pub memory_type: MemoryType,
    pub content: String,
    pub summary: String,
    pub confidence: f64,
    pub importance: f64,
    pub freshness: f64,
    pub status: MemoryStatus,
    pub source_kind: MemorySourceKind,
    #[serde(default)]
    pub version: Option<i32>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub last_recalled_at: Option<DateTime<Utc>>,
    #[serde(default)]
    pub tags: Vec<String>,
    #[serde(default)]
    pub entities: Vec<String>,
    #[serde(default)]
    pub metadata: serde_json::Value,
}

/// Verification and security Trust Tiers (1 to 5) for persistent memories.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Default, Serialize, Deserialize)]
#[repr(u8)]
pub enum TrustTier {
    /// Tier 1: Pinned by human developer / constitutional invariant. Fact.
    Pinned = 1,
    /// Tier 2: Verified fact from exit code / test run / symbol check. Fact with evidence.
    Verified = 2,
    /// Tier 3: Explicit user statement or preference in a session. Fact with source.
    UserStatement = 3,
    /// Tier 4: Autonomous agent claim (unverified candidate). Not returned in default fact recall.
    #[default]
    AgentClaim = 4,
    /// Tier 5: Untrusted external origin (web, fetched files, third-party agent posts). Quarantined.
    UntrustedOrigin = 5,
}

impl TrustTier {
    pub fn as_u8(&self) -> u8 {
        *self as u8
    }

    pub fn from_u8(n: u8) -> Self {
        match n {
            1 => Self::Pinned,
            2 => Self::Verified,
            3 => Self::UserStatement,
            5 => Self::UntrustedOrigin,
            _ => Self::AgentClaim,
        }
    }
}

impl Memory {
    /// Returns the resolved trust tier of this memory record.
    pub fn trust_tier(&self) -> TrustTier {
        if let Some(tier_num) = self.metadata.get("trust_tier").and_then(|v| v.as_u64()) {
            TrustTier::from_u8(tier_num as u8)
        } else if self.status == MemoryStatus::Pinned {
            TrustTier::Pinned
        } else if self.source_kind == MemorySourceKind::User {
            TrustTier::UserStatement
        } else if self.confidence >= 0.95 {
            TrustTier::Verified
        } else {
            TrustTier::AgentClaim
        }
    }

    /// Sets or updates the trust tier in memory metadata.
    pub fn set_trust_tier(&mut self, tier: TrustTier) {
        if let serde_json::Value::Object(ref mut map) = self.metadata {
            map.insert("trust_tier".to_string(), serde_json::json!(tier.as_u8()));
        } else {
            self.metadata = serde_json::json!({ "trust_tier": tier.as_u8() });
        }
    }

    /// Returns true if this memory is quarantined or pending human review.
    pub fn is_quarantined(&self) -> bool {
        self.trust_tier() == TrustTier::UntrustedOrigin
            || self.status == MemoryStatus::Review
            || self.status == MemoryStatus::Candidate
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MemoryVersion {
    pub id: String,
    pub memory_id: String,
    pub version_num: i32,
    pub content: String,
    pub summary: String,
    pub reason: Option<String>,
    #[serde(default)]
    pub evidence_event_ids: Vec<String>,
    pub created_at: DateTime<Utc>,
}
