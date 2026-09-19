use super::memory::MemoryScope;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Preference {
    pub id: String,
    pub key: String,
    pub value: serde_json::Value,
    pub scope: MemoryScope,
    pub project_id: Option<String>,
    pub locked: bool,
    pub source: String, // "user", "agent", "system"
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}
