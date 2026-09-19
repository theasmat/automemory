use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReviewProposal {
    pub id: String,
    pub project_id: Option<String>,
    pub proposal_type: String, // "conflict", "archive", "supersede", "deletion"
    pub target_id: String,
    pub description: String,
    pub suggested_action: String,
    pub status: String, // "pending", "approved", "rejected"
    pub created_at: DateTime<Utc>,
    pub decided_at: Option<DateTime<Utc>>,
}
