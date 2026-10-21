use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

pub const FILE_MAX: usize = 1_000_000;
pub const PROJECT_MAX: usize = 50_000_000;
pub const REVISIONS_KEPT: usize = 20;
pub const EXTENSIONS: [&str; 9] = ["md", "markdown", "txt", "json", "yaml", "yml", "toml", "rst", "csv"];

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Origin {
    Human,
    Agent,
}

impl Origin {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Human => "human",
            Self::Agent => "agent",
        }
    }
    pub fn parse(s: &str) -> Self {
        if s == "agent" { Self::Agent } else { Self::Human }
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct LibraryFile {
    pub id: String,
    pub project_id: Option<String>,
    pub path: String,
    pub origin: Origin,
    /// `human`, or the agent's name.
    pub author: String,
    /// `project` (syncs to your computers once L3 exists) or `personal` (this machine only).
    pub scope: String,
    pub size: usize,
    pub sha256: String,
    pub content: String,
    pub l0: String,
    pub l1: String,
    pub revisions: usize,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone)]
pub struct NewFile {
    pub project_id: Option<String>,
    pub path: String,
    pub content: String,
    pub origin: Origin,
    pub author: String,
    pub personal: bool,
    /// A person accepts a file that looks like it holds a secret (never an agent).
    pub allow_secrets: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub enum PutOutcome {
    Created,
    Updated,
    /// Same content as before: nothing changed.
    Unchanged,
}
