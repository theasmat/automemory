use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TrackedDocument {
    pub id: String,
    pub project_id: String,
    pub relative_path: String,
    pub category: String,
    pub title: String,
    pub content_hash: String,
    pub remote_synced_hash: Option<String>,
    pub summary: Option<String>,
    pub l0_abstract: Option<String>,
    pub l1_overview: Option<String>,
    pub last_scanned_at: DateTime<Utc>,
    pub is_deleted: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DocumentRevision {
    pub id: String,
    pub document_id: String,
    pub revision_num: i32,
    pub content: String,
    pub content_hash: String,
    pub created_at: DateTime<Utc>,
}
