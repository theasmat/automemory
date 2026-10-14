use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

/// The lifecycle status of an architectural decision.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum DecisionStatus {
    #[default]
    Active,
    Superseded,
}

impl std::fmt::Display for DecisionStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Active => write!(f, "active"),
            Self::Superseded => write!(f, "superseded"),
        }
    }
}

impl std::str::FromStr for DecisionStatus {
    type Err = String;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "active" => Ok(Self::Active),
            "superseded" => Ok(Self::Superseded),
            other => Err(format!("Unknown decision status: {other}")),
        }
    }
}

/// An immutable, versioned project architectural decision.
/// Decisions are never deleted; when superseded, prior decisions and dissents are preserved.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Decision {
    pub id: String,
    pub project_id: Option<String>,
    pub topic: String,
    pub decision: String,
    pub reason: String,
    #[serde(default)]
    pub alternatives: Vec<String>,
    pub dissent: Option<String>,
    pub agent: String,
    #[serde(default)]
    pub symbols: Vec<String>,
    #[serde(default)]
    pub evidence: Vec<String>,
    pub status: DecisionStatus,
    pub superseded_by: Option<String>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

/// Arguments for recording a new binding architecture decision.
#[derive(Debug, Clone)]
pub struct DecisionRecordArgs<'a> {
    pub project_id: Option<&'a str>,
    pub topic: &'a str,
    pub decision: &'a str,
    pub reason: &'a str,
    pub alternatives: &'a [String],
    pub dissent: Option<&'a str>,
    pub agent: &'a str,
    pub symbols: &'a [String],
    pub evidence: &'a [String],
}

/// Result returned when querying decisions for a specific topic.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DecisionQueryResult {
    pub topic: String,
    pub active: Option<Decision>,
    pub history: Vec<Decision>,
}
