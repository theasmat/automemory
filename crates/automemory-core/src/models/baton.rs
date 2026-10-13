use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

/// A <400-token handoff packet from one agent session to another,
/// built from the ledger (exit codes, files, leases) not agent prose.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Baton {
    pub id: String,
    pub project_id: Option<String>,
    pub from_session: String,
    pub to_session: Option<String>,
    pub agent: String,
    pub packet: String,
    pub created_at: DateTime<Utc>,
}
