use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

/// The scope granularity of a resource lease.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum LeaseScopeType {
    #[default]
    File,
    Glob,
    Symbol,
}

/// Arguments for acquiring or renewing a coordinated multi-agent resource lease.
#[derive(Debug, Clone)]
pub struct LeaseAcquireArgs<'a> {
    pub project_id: Option<&'a str>,
    pub agent: &'a str,
    pub session_id: &'a str,
    pub scope: &'a str,
    pub scope_type: LeaseScopeType,
    pub goal: &'a str,
    pub ttl_seconds: i64,
}

impl std::fmt::Display for LeaseScopeType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::File => write!(f, "file"),
            Self::Glob => write!(f, "glob"),
            Self::Symbol => write!(f, "symbol"),
        }
    }
}

impl From<&str> for LeaseScopeType {
    fn from(s: &str) -> Self {
        match s.to_lowercase().as_str() {
            "glob" => Self::Glob,
            "symbol" => Self::Symbol,
            _ => Self::File,
        }
    }
}

/// A coordinated multi-agent resource lease.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Lease {
    pub id: String,
    pub project_id: Option<String>,
    pub agent: String,
    pub session_id: String,
    pub scope: String,
    pub scope_type: LeaseScopeType,
    pub goal: String,
    pub ttl_seconds: i64,
    pub acquired_at: DateTime<Utc>,
    pub heartbeat_at: DateTime<Utc>,
    pub expires_at: DateTime<Utc>,
}

impl Lease {
    pub fn remaining_seconds(&self, now: DateTime<Utc>) -> i64 {
        (self.expires_at - now).num_seconds().max(0)
    }
}

/// Details of a collision when an agent attempts to edit or claim a locked resource.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LeaseConflict {
    pub scope: String,
    pub holder_agent: String,
    pub holder_session_id: String,
    pub goal: String,
    pub remaining_seconds: i64,
    pub expires_at: DateTime<Utc>,
}

/// Result of an acquire or renewal attempt.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "status", rename_all = "lowercase")]
pub enum LeaseAcquireResult {
    Acquired { lease: Lease },
    Renewed { lease: Lease },
    Conflict { conflict: LeaseConflict },
}
