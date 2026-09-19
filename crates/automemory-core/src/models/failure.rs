use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FailureRecord {
    pub id: String,
    pub project_id: Option<String>,
    pub task_title: String,
    pub category: Option<String>,
    pub failed_tool: Option<String>,
    pub error_message: String,
    pub root_cause_hypothesis: Option<String>,
    pub fix_applied: Option<String>,
    pub verification_result: Option<String>,
    pub status: String, // "unresolved", "resolved", "obsolete"
    #[serde(default)]
    pub evidence_event_ids: Vec<String>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}
