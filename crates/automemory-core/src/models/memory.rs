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
    Superseded,
    Archived,
    Invalid,
    Review,
}

impl std::fmt::Display for MemoryStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Active => write!(f, "active"),
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
    pub memory_type: MemoryType,
    pub content: String,
    pub summary: String,
    pub confidence: f64,
    pub importance: f64,
    pub freshness: f64,
    pub status: MemoryStatus,
    pub source_kind: MemorySourceKind,
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

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MemoryLink {
    pub source_memory_id: String,
    pub target_memory_id: String,
    pub relation_type: String, // "supersedes", "relates_to", "contradicts"
    pub created_at: DateTime<Utc>,
}
