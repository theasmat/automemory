use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ReviewStatus {
    #[default]
    Pending,
    Approved,
    Rejected,
}

impl std::fmt::Display for ReviewStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Pending => write!(f, "pending"),
            Self::Approved => write!(f, "approved"),
            Self::Rejected => write!(f, "rejected"),
        }
    }
}

impl std::str::FromStr for ReviewStatus {
    type Err = String;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "pending" => Ok(Self::Pending),
            "approved" => Ok(Self::Approved),
            "rejected" => Ok(Self::Rejected),
            other => Err(format!("Unknown review status: {other}")),
        }
    }
}

/// One diff awaiting a different agent's review. `tests_passed` is the
/// judge: a failing test run rejects regardless of the reviewer's vote.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Review {
    pub id: String,
    pub project_id: Option<String>,
    pub task_id: Option<String>,
    pub author_session: String,
    pub title: String,
    pub diff: String,
    pub test_command: Option<String>,
    pub status: ReviewStatus,
    pub reviewer_session: Option<String>,
    pub tests_passed: Option<bool>,
    pub verdict: Option<String>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}
