use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReviewProposal {
    pub id: String,
    pub project_id: Option<String>,
    pub proposal_type: String, // "conflict", "archive", "supersede", "deletion", "duplicate_merge", "tool_execution_review"
    pub target_id: String,
    pub description: String,
    pub suggested_action: String,
    pub status: String, // "pending", "approved", "rejected"
    pub created_at: DateTime<Utc>,
    pub decided_at: Option<DateTime<Utc>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProposalBatchResult {
    pub decided_count: usize,
    pub approved_count: usize,
    pub rejected_count: usize,
    pub archived_memories_count: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProposalSummaryItem {
    pub proposal_type: String,
    pub pending: usize,
    pub approved: usize,
    pub rejected: usize,
    pub total: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProposalsSummary {
    pub total_pending: usize,
    pub total_approved: usize,
    pub total_rejected: usize,
    pub total: usize,
    pub by_type: Vec<ProposalSummaryItem>,
}

