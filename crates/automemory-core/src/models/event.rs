use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NormalizedEvent {
    pub event_id: String,
    pub event_type: String, // "session.start", "tool.requested", "tool.completed", "tool.failed", "task.completed", "session.stop"
    pub timestamp: DateTime<Utc>,
    pub agent: AgentInfo,
    pub session_id: String,
    pub task_id: Option<String>,
    pub project: ProjectRef,
    pub tool: Option<ToolInfo>,
    #[serde(default)]
    pub artifacts: Vec<ArtifactRef>,
    pub sensitivity: String,
    pub schema_version: i32,
    #[serde(default)]
    pub raw_payload: Option<serde_json::Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentInfo {
    pub adapter_id: String, // "antigravity", "claude", "cursor", etc.
    pub agent_instance_id: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProjectRef {
    pub id: Option<String>,
    pub slug: Option<String>,
    pub root_path: String,
    pub source_git_remote: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolInfo {
    pub name: String,
    pub category: String, // "command", "file_read", "file_write", "search", etc.
    pub input: Option<serde_json::Value>,
    pub input_hash: Option<String>,
    pub output: Option<serde_json::Value>,
    pub output_hash: Option<String>,
    pub status: String, // "success", "failed"
    pub exit_code: Option<i32>,
    pub duration_ms: Option<u64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ArtifactRef {
    pub path: String,
    pub kind: String, // "file", "directory", "url"
    pub hash: Option<String>,
}
