use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProceduralSkill {
    pub id: String,
    pub project_id: Option<String>,
    pub name: String,
    pub description: String,
    pub trigger_patterns: Vec<String>,
    pub preconditions: Vec<String>,
    pub steps: Vec<String>,
    pub caveats: Vec<String>,
    pub success_count: u32,
    pub failure_count: u32,
    pub source_trace_ids: Vec<String>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}
