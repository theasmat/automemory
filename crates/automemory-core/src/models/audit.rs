use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuditLogEntry {
    pub id: String,
    pub action: String,      // "create", "update", "archive", "delete", "sync"
    pub entity_type: String, // "memory", "trace", "failure", "preference", "doc"
    pub entity_id: String,
    pub details: serde_json::Value,
    pub timestamp: DateTime<Utc>,
}
