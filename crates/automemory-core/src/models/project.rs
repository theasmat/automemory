use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Project {
    pub id: String,
    pub slug: String,
    pub display_name: String,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub capture_policy: CapturePolicy,
    pub context_budget: ContextBudget,
    pub document_config: DocumentConfig,
    pub sync_config: SyncConfig,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProjectPath {
    pub project_id: String,
    pub local_path: String,
    pub is_primary: bool,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AutopilotMode {
    #[default]
    Vibe,
    Strict,
    Review,
}

impl std::fmt::Display for AutopilotMode {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Vibe => write!(f, "vibe"),
            Self::Strict => write!(f, "strict"),
            Self::Review => write!(f, "review"),
        }
    }
}

impl std::str::FromStr for AutopilotMode {
    type Err = anyhow::Error;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().trim() {
            "vibe" => Ok(Self::Vibe),
            "strict" => Ok(Self::Strict),
            "review" => Ok(Self::Review),
            other => anyhow::bail!(
                "Unknown autopilot mode '{}'. Supported: vibe, strict, review",
                other
            ),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CapturePolicy {
    pub enabled: bool,
    pub auto_approve_aware: bool,
    pub raw_event_policy: String, // "hash_only", "redacted", "full_local", "full_synced"
    pub allow_agent_preference_writes: bool,
    #[serde(default)]
    pub mode: AutopilotMode,
}

impl Default for CapturePolicy {
    fn default() -> Self {
        Self {
            enabled: true,
            auto_approve_aware: true,
            raw_event_policy: "redacted".to_string(),
            allow_agent_preference_writes: false,
            mode: AutopilotMode::Vibe,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ContextBudget {
    pub boot_token_budget: usize,
    pub task_token_budget: usize,
}

impl Default for ContextBudget {
    fn default() -> Self {
        Self {
            boot_token_budget: 2000,
            task_token_budget: 4000,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DocumentConfig {
    pub include_patterns: Vec<String>,
    pub exclude_patterns: Vec<String>,
}

impl Default for DocumentConfig {
    fn default() -> Self {
        Self {
            include_patterns: vec![
                "PRD.md".to_string(),
                "docs/**/*.md".to_string(),
                ".cursor/rules/**".to_string(),
                "AGENTS.md".to_string(),
                "CLAUDE.md".to_string(),
            ],
            exclude_patterns: vec![
                "**/.env*".to_string(),
                "**/*.pem".to_string(),
                "**/node_modules/**".to_string(),
                "**/target/**".to_string(),
            ],
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SyncConfig {
    pub mode: String, // "local", "github", "managed"
    pub enabled: bool,
    pub auto_push_on_task: bool,
    pub auto_push_on_stop: bool,
    pub vault_project_path: Option<String>,
    pub git_remote: Option<String>,
}

impl Default for SyncConfig {
    fn default() -> Self {
        Self {
            mode: "local".to_string(),
            enabled: false,
            auto_push_on_task: true,
            auto_push_on_stop: true,
            vault_project_path: None,
            git_remote: None,
        }
    }
}
