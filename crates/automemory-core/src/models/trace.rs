use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SuccessTrace {
    pub id: String,
    pub project_id: Option<String>,
    pub title: String,
    pub category: String, // e.g. "deployment", "testing", "migration", "refactor"
    pub status: String,   // "active", "deprecated", "invalid"
    #[serde(default)]
    pub preconditions: Vec<String>,
    pub steps: Vec<TraceStep>,
    pub reproducibility_score: f64,
    #[serde(default)]
    pub caveats: Vec<String>,
    pub source_task_id: Option<String>,
    #[serde(default)]
    pub evidence_event_ids: Vec<String>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TraceStep {
    pub position: i32,
    pub action: String,
    pub tool: String,
    pub command: Option<String>,
    pub verification: Option<String>,
}
