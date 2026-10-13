use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

/// Lifecycle of a queue work item: open → claimed → submitted (for review).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum TaskState {
    #[default]
    Open,
    Claimed,
    Submitted,
}

impl std::fmt::Display for TaskState {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Open => write!(f, "open"),
            Self::Claimed => write!(f, "claimed"),
            Self::Submitted => write!(f, "submitted"),
        }
    }
}

impl std::str::FromStr for TaskState {
    type Err = String;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "open" => Ok(Self::Open),
            "claimed" => Ok(Self::Claimed),
            "submitted" => Ok(Self::Submitted),
            other => Err(format!("Unknown task state: {other}")),
        }
    }
}

/// One claimable unit of work. `session_id` is the creator; `owner_session`
/// is the claiming agent (none while open).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkTask {
    pub id: String,
    pub session_id: String,
    pub project_id: Option<String>,
    pub title: String,
    pub spec: String,
    #[serde(default)]
    pub symbols: Vec<String>,
    pub owner_session: Option<String>,
    pub result: Option<String>,
    pub status: TaskState,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}
