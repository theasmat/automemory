use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RiskCategory {
    CatastrophicFilesystem,
    ProductionGitDestruction,
    DatabaseMigrationOrDrop,
    CloudInfrastructureMutation,
    SecretExposure,
    DestructiveGitReset,
    CriticalConfigTamper,
    KnownFailureSignature,
    StandardAction,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GuardrailRule {
    pub id: String,
    pub name: String,
    pub category: RiskCategory,
    pub pattern: String,
    pub is_critical: bool,
    pub description: String,
    pub mitigation_hint: String,
}

impl GuardrailRule {
    pub fn default_rules() -> Vec<Self> {
        vec![
            // ── Critical Catastrophic Rules (Block in all modes including Vibe) ──
            Self {
                id: "rule_rm_root".to_string(),
                name: "Destructive Root/Home Directory Deletion".to_string(),
                category: RiskCategory::CatastrophicFilesystem,
                pattern: r"(?i)\brm\s+(-[a-z]*r[a-z]*f[a-z]*|-[a-z]*f[a-z]*r[a-z]*)\s+(/|/\*|~|~/\*|\$HOME|\$HOME/\*)".to_string(),
                is_critical: true,
                description: "Attempting to recursively delete root, home, or entire system drive.".to_string(),
                mitigation_hint: "Target specific project subdirectories instead of root or home paths.".to_string(),
            },
            Self {
                id: "rule_raw_disk_format".to_string(),
                name: "Raw Disk Write or Format".to_string(),
                category: RiskCategory::CatastrophicFilesystem,
                pattern: r"(?i)\b(mkfs(\.[a-z0-9]+)?|dd\s+if=.*of=/dev/)".to_string(),
                is_critical: true,
                description: "Attempting raw disk formatting or direct block device overwrites.".to_string(),
                mitigation_hint: "Direct disk formatting commands are permanently prohibited.".to_string(),
            },
            Self {
                id: "rule_git_force_push_prod".to_string(),
                name: "Force Push to Protected Production Branch".to_string(),
                category: RiskCategory::ProductionGitDestruction,
                pattern: r"(?i)\bgit\s+push\s+.*(--force|-f)\s+.*(main|master|prod|production|release)".to_string(),
                is_critical: true,
                description: "Attempting destructive force push to main/master/production branch.".to_string(),
                mitigation_hint: "Use standard git push or rebase safely without overwriting remote history.".to_string(),
            },
            Self {
                id: "rule_raw_db_drop".to_string(),
                name: "Raw Database Drop or Truncate".to_string(),
                category: RiskCategory::DatabaseMigrationOrDrop,
                pattern: r"(?i)\b(DROP\s+DATABASE|TRUNCATE\s+TABLE|DROP\s+SCHEMA)\b".to_string(),
                is_critical: true,
                description: "Attempting raw unescaped database drop or table truncation.".to_string(),
                mitigation_hint: "Use versioned migrations with reversible rollbacks instead of raw drops.".to_string(),
            },
            Self {
                id: "rule_plain_secret_cli".to_string(),
                name: "Plain-text Secret in Command Line".to_string(),
                category: RiskCategory::SecretExposure,
                pattern: r"(?i)(sk-[a-zA-Z0-9]{20,}|ghp_[a-zA-Z0-9]{20,}|xoxb-[0-9]{10,}|AWS_SECRET_ACCESS_KEY=[a-zA-Z0-9/+=]{20,})".to_string(),
                is_critical: true,
                description: "Plaintext API secret or credential detected directly in command arguments.".to_string(),
                mitigation_hint: "Load credentials via environment variables instead of hardcoding in commands.".to_string(),
            },

            // ── High Risk Operations (Block in Strict mode, Warn in Vibe mode) ──
            Self {
                id: "rule_db_migration".to_string(),
                name: "Database Schema Migration".to_string(),
                category: RiskCategory::DatabaseMigrationOrDrop,
                pattern: r"(?i)\b(alembic\s+upgrade|prisma\s+migrate|diesel\s+migration\s+run|flyway\s+migrate|rake\s+db:migrate)\b".to_string(),
                is_critical: false,
                description: "Database schema migration detected. High impact on stateful database tables.".to_string(),
                mitigation_hint: "Verify backup and ensure schema changes are backwards compatible.".to_string(),
            },
            Self {
                id: "rule_cloud_infra_delete".to_string(),
                name: "Cloud Infrastructure Teardown".to_string(),
                category: RiskCategory::CloudInfrastructureMutation,
                pattern: r"(?i)\b(kubectl\s+delete|helm\s+uninstall|terraform\s+destroy|docker\s+rm\s+-f)\b".to_string(),
                is_critical: false,
                description: "Destructive cloud or container infrastructure modification.".to_string(),
                mitigation_hint: "Verify target cluster and namespace before applying destructive changes.".to_string(),
            },
            Self {
                id: "rule_git_hard_reset".to_string(),
                name: "Destructive Git Reset or Clean".to_string(),
                category: RiskCategory::DestructiveGitReset,
                pattern: r"(?i)\bgit\s+(reset\s+--hard|clean\s+-[a-z]*f[a-z]*d|clean\s+-[a-z]*d[a-z]*f)".to_string(),
                is_critical: false,
                description: "Destructive git reset --hard or clean will permanently discard uncommitted changes.".to_string(),
                mitigation_hint: "Use git stash before resetting to prevent accidental code loss.".to_string(),
            },
            Self {
                id: "rule_env_file_tamper".to_string(),
                name: "Environment Secret File Modification".to_string(),
                category: RiskCategory::CriticalConfigTamper,
                pattern: r"(?i)(\.env|\.env\.production|\.env\.local|id_rsa|\.pem)$".to_string(),
                is_critical: false,
                description: "Direct modification or deletion of secret environment configuration.".to_string(),
                mitigation_hint: "Ensure environment secrets are handled securely without exposing values.".to_string(),
            },
            Self {
                id: "rule_raw_sqlite_internal_db".to_string(),
                name: "Raw SQLite Access to Internal AutoMemory Database".to_string(),
                category: RiskCategory::DatabaseMigrationOrDrop,
                pattern: r"(?i)\bsqlite3\s+.*automemory\.db\b".to_string(),
                is_critical: false,
                description: "Direct raw SQLite manipulation of AutoMemory internal database.".to_string(),
                mitigation_hint: "Use native CLI 'automemory proposal ...' or MCP tools 'memory.proposals.*' instead of raw SQLite commands.".to_string(),
            },
        ]
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CustomGuardrailRule {
    pub id: String,
    pub project_id: Option<String>,
    pub name: String,
    pub category: String,
    pub pattern: String,
    pub is_critical: bool,
    pub action: String, // "block" or "warn"
    pub description: String,
    pub mitigation_hint: String,
    pub enabled: bool,
    pub created_at: chrono::DateTime<chrono::Utc>,
}

impl From<CustomGuardrailRule> for GuardrailRule {
    fn from(c: CustomGuardrailRule) -> Self {
        let category = match c.category.to_lowercase().as_str() {
            "catastrophic_filesystem" => RiskCategory::CatastrophicFilesystem,
            "production_git_destruction" => RiskCategory::ProductionGitDestruction,
            "database_migration_or_drop" => RiskCategory::DatabaseMigrationOrDrop,
            "cloud_infrastructure_mutation" => RiskCategory::CloudInfrastructureMutation,
            "secret_exposure" => RiskCategory::SecretExposure,
            "destructive_git_reset" => RiskCategory::DestructiveGitReset,
            "critical_config_tamper" => RiskCategory::CriticalConfigTamper,
            "known_failure_signature" => RiskCategory::KnownFailureSignature,
            _ => RiskCategory::StandardAction,
        };

        Self {
            id: c.id,
            name: c.name,
            category,
            pattern: c.pattern,
            is_critical: c.is_critical,
            description: c.description,
            mitigation_hint: c.mitigation_hint,
        }
    }
}
