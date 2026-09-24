use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionSummary {
    pub session_id: String,
    pub project_id: Option<String>,
    pub agent_adapter: String,
    pub event_count: usize,
    pub started_at: DateTime<Utc>,
    pub last_event_at: DateTime<Utc>,
    pub status: String,
}
