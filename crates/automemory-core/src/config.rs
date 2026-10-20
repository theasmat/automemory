use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GlobalConfig {
    #[serde(default = "default_version")]
    pub version: u32,
    #[serde(default)]
    pub cloud: CloudConfig,
    #[serde(default)]
    pub vault: VaultConfig,
    #[serde(default)]
    pub defaults: DefaultSettings,
    #[serde(default)]
    pub security: SecurityConfig,
    #[serde(default)]
    pub locks: LockSettings,
    #[serde(default)]
    pub retrieval: RetrievalSettings,
    #[serde(default)]
    pub embeddings: EmbeddingSettings,
    #[serde(default)]
    pub llm: LlmSettings,
    #[serde(default)]
    pub projects: Vec<ProjectRegistryEntry>,
}

/// `[retrieval]`: how recall finds memories.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RetrievalSettings {
    /// `keyword` (default: FTS5, paths, trust, recency; no model, no network) or `hybrid`
    /// (the same plus vector similarity from `[embeddings]`, fused by rank).
    #[serde(default = "default_retrieval_mode")]
    pub mode: String,
}

fn default_retrieval_mode() -> String {
    "keyword".to_string()
}

impl Default for RetrievalSettings {
    fn default() -> Self {
        Self { mode: default_retrieval_mode() }
    }
}

impl RetrievalSettings {
    pub fn is_hybrid(&self) -> bool {
        self.mode.eq_ignore_ascii_case("hybrid")
    }
}

/// `[llm]`: an optional model that only *suggests* (conflicts, duplicates, candidate notes). Off while `model` is empty.
/// Any OpenAI-compatible chat endpoint, including Ollama's `/v1`. The flat fields are the `default` profile
/// (key in the keychain under `llm`); more setups live under `[llm.profiles.<name>]` (key under `llm.<name>`).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LlmSettings {
    #[serde(default)]
    pub model: String,
    /// Base URL, for example `https://api.openai.com/v1` or `http://127.0.0.1:11434/v1`.
    #[serde(default)]
    pub endpoint: String,
    #[serde(default)]
    pub api_key_env: String,
    /// Required before any text goes to a host that is not this machine.
    #[serde(default)]
    pub allow_network: bool,
    #[serde(default = "default_llm_timeout")]
    pub timeout_secs: u64,
    #[serde(default = "default_llm_tokens")]
    pub max_tokens_per_day: u64,
    /// Saved alternatives; `active` names the one in use (empty = the flat `default` fields).
    #[serde(default)]
    pub profiles: std::collections::BTreeMap<String, LlmProfile>,
    #[serde(default)]
    pub active: String,
}

/// One saved LLM setup. The API key is never part of it.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct LlmProfile {
    #[serde(default)]
    pub model: String,
    #[serde(default)]
    pub endpoint: String,
    #[serde(default)]
    pub api_key_env: String,
    #[serde(default)]
    pub allow_network: bool,
    #[serde(default = "default_llm_timeout")]
    pub timeout_secs: u64,
}

impl LlmSettings {
    /// The profile in use, as flat settings (no `profiles`, `active` set to its name).
    pub fn resolved(&self) -> LlmSettings {
        let base = LlmSettings { profiles: Default::default(), ..self.clone() };
        match self.profiles.get(&self.active).filter(|_| !self.active.is_empty()) {
            Some(p) => LlmSettings {
                model: p.model.clone(), endpoint: p.endpoint.clone(), api_key_env: p.api_key_env.clone(),
                allow_network: p.allow_network, timeout_secs: p.timeout_secs, ..base
            },
            None => LlmSettings { active: String::new(), ..base },
        }
    }

    /// The keychain name for a profile (`default` or empty = the original `llm`).
    pub fn secret_for(profile: &str) -> String {
        if profile.is_empty() || profile == "default" { "llm".to_string() } else { format!("llm.{profile}") }
    }

    /// Where the active profile's API key lives (`llm` for the default one, so existing keys keep working).
    pub fn secret_name(&self) -> String {
        Self::secret_for(&self.active)
    }
}

fn default_llm_timeout() -> u64 {
    60
}

fn default_llm_tokens() -> u64 {
    200_000
}

impl Default for LlmSettings {
    fn default() -> Self {
        Self {
            model: String::new(),
            endpoint: String::new(),
            api_key_env: String::new(),
            allow_network: false,
            timeout_secs: default_llm_timeout(),
            max_tokens_per_day: default_llm_tokens(),
            profiles: Default::default(),
            active: String::new(),
        }
    }
}

/// `[embeddings]`: read only in hybrid mode.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EmbeddingSettings {
    /// `ngram` (built in, offline), `ollama`, `openai` (any OpenAI-compatible endpoint),
    /// `exec` (a command: JSON texts in, JSON vectors out) or `onnx` (optional build feature).
    #[serde(default = "default_embedding_provider")]
    pub provider: String,
    #[serde(default)]
    pub model: String,
    /// Base URL (ollama, openai) or the command line (exec).
    #[serde(default)]
    pub endpoint: String,
    /// NAME of the environment variable holding the API key; the key itself is never stored.
    #[serde(default)]
    pub api_key_env: String,
    /// Hosted providers send memory text off the machine; this must be true to allow it.
    #[serde(default)]
    pub allow_network: bool,
    /// Request a vector size from providers that support it (OpenAI-style `dimensions`).
    #[serde(default)]
    pub dimensions: Option<usize>,
    #[serde(default = "default_embedding_timeout")]
    pub timeout_secs: u64,
    /// Saved alternatives (`[embeddings.profiles.<name>]`); `active` names the one in use.
    /// With no active profile the flat fields above are the "default" profile.
    #[serde(default)]
    pub profiles: std::collections::BTreeMap<String, EmbeddingProfile>,
    #[serde(default)]
    pub active: String,
    /// Stop indexing for the day after this many (estimated) tokens went to a non-local provider.
    #[serde(default = "default_token_cap")]
    pub max_tokens_per_day: u64,
}

/// One saved provider setup. The API key is never part of it: it lives in the keychain under
/// `embeddings.<profile name>` (or in the environment variable named by `api_key_env`).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EmbeddingProfile {
    #[serde(default = "default_embedding_provider")]
    pub provider: String,
    #[serde(default)]
    pub model: String,
    #[serde(default)]
    pub endpoint: String,
    #[serde(default)]
    pub api_key_env: String,
    #[serde(default)]
    pub allow_network: bool,
    #[serde(default)]
    pub dimensions: Option<usize>,
    #[serde(default = "default_embedding_timeout")]
    pub timeout_secs: u64,
}

fn default_token_cap() -> u64 {
    2_000_000
}

impl EmbeddingSettings {
    /// The profile in use, as flat settings (with `active` set to its name and no `profiles`).
    pub fn resolved(&self) -> EmbeddingSettings {
        let base = EmbeddingSettings { profiles: Default::default(), ..self.clone() };
        match self.profiles.get(&self.active).filter(|_| !self.active.is_empty()) {
            Some(p) => EmbeddingSettings {
                provider: p.provider.clone(),
                model: p.model.clone(),
                endpoint: p.endpoint.clone(),
                api_key_env: p.api_key_env.clone(),
                allow_network: p.allow_network,
                dimensions: p.dimensions,
                timeout_secs: p.timeout_secs,
                ..base
            },
            None => EmbeddingSettings { active: "default".to_string(), ..base },
        }
    }

    /// Where this profile's API key is stored in the keychain.
    pub fn secret_name(&self) -> String {
        format!("embeddings.{}", if self.active.is_empty() { "default" } else { &self.active })
    }
}

fn default_embedding_provider() -> String {
    "ngram".to_string()
}

fn default_embedding_timeout() -> u64 {
    8
}

impl Default for EmbeddingSettings {
    fn default() -> Self {
        Self {
            provider: default_embedding_provider(),
            model: String::new(),
            endpoint: String::new(),
            api_key_env: String::new(),
            allow_network: false,
            dimensions: None,
            timeout_secs: default_embedding_timeout(),
            profiles: Default::default(),
            active: String::new(),
            max_tokens_per_day: default_token_cap(),
        }
    }
}

/// Automatic file locking (`[locks]` in config.toml).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LockSettings {
    /// Paths that never take or conflict on a lease (build output, dependencies, lockfiles).
    /// Patterns: `dir/**` for a folder, `*.ext` for a suffix, otherwise an exact path or segment.
    #[serde(default = "default_lock_ignore")]
    pub ignore: Vec<String>,
    /// A lease expires this many seconds after its holder's last tool event.
    #[serde(default = "default_lock_idle_ttl")]
    pub idle_ttl_seconds: i64,
    /// After each shell command, compare `git status` before and after to find files it
    /// changed that no lease covered.
    #[serde(default = "default_true")]
    pub reconcile: bool,
}

fn default_lock_ignore() -> Vec<String> {
    [
        "target/**", "node_modules/**", "dist/**", "build/**", ".next/**", ".nuxt/**", ".git/**",
        "__pycache__/**", "*.lock", "*.pyc", "*.log", ".DS_Store",
    ]
    .iter()
    .map(|s| s.to_string())
    .collect()
}

fn default_lock_idle_ttl() -> i64 {
    600
}

impl Default for LockSettings {
    fn default() -> Self {
        Self {
            ignore: default_lock_ignore(),
            idle_ttl_seconds: default_lock_idle_ttl(),
            reconcile: true,
        }
    }
}

impl LockSettings {
    /// True when `path` (relative or absolute) is covered by an ignore pattern.
    pub fn is_ignored(&self, path: &str) -> bool {
        let p = path.trim_start_matches("./");
        self.ignore.iter().any(|pat| {
            if let Some(dir) = pat.strip_suffix("/**") {
                p == dir
                    || p.starts_with(&format!("{dir}/"))
                    || p.contains(&format!("/{dir}/"))
            } else if let Some(ext) = pat.strip_prefix('*') {
                p.ends_with(ext)
            } else {
                p == pat || p.ends_with(&format!("/{pat}"))
            }
        })
    }
}

fn default_version() -> u32 {
    1
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CloudConfig {
    pub endpoint: Option<String>,
    pub auth_token: Option<String>,
    pub encryption_key: Option<String>,
    #[serde(default = "default_true")]
    pub auto_sync: bool,
}

impl Default for CloudConfig {
    fn default() -> Self {
        Self {
            endpoint: None,
            auth_token: None,
            encryption_key: None,
            auto_sync: true,
        }
    }
}

impl Default for GlobalConfig {
    fn default() -> Self {
        Self {
            version: 1,
            cloud: CloudConfig::default(),
            vault: VaultConfig::default(),
            defaults: DefaultSettings::default(),
            security: SecurityConfig::default(),
            locks: LockSettings::default(),
            retrieval: RetrievalSettings::default(),
            embeddings: EmbeddingSettings::default(),
            llm: LlmSettings::default(),
            projects: Vec::new(),
        }
    }
}

/// Human authorization for safety control-plane writes. The hash (never the
/// password) lives in config; see `auth.rs` for why keychain/config-file
/// contents must not gate anything by themselves.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct SecurityConfig {
    #[serde(default)]
    pub password_hash: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VaultConfig {
    pub remote_url: Option<String>,
    pub local_vault_dir: Option<PathBuf>,
    pub encryption_key: Option<String>,
    #[serde(default = "default_true")]
    pub auto_sync: bool,
    #[serde(default = "default_true")]
    pub auto_push: bool,
    #[serde(default = "default_sync_interval")]
    pub sync_interval_seconds: u64,
}

fn default_true() -> bool {
    true
}

fn default_sync_interval() -> u64 {
    300
}

impl Default for VaultConfig {
    fn default() -> Self {
        Self {
            remote_url: None,
            local_vault_dir: None,
            encryption_key: None,
            auto_sync: true,
            auto_push: true,
            sync_interval_seconds: 300,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DefaultSettings {
    #[serde(default = "default_mode")]
    pub autopilot_mode: String,
    #[serde(default = "default_boot_budget")]
    pub token_budget_boot: usize,
    #[serde(default = "default_task_budget")]
    pub token_budget_task: usize,
    /// PRD §5 is pull-only and §16.2 cuts boot-time injection, so session
    /// start stays silent unless the user explicitly opts back in.
    #[serde(default)]
    pub boot_injection: bool,
    /// Trust policy selecting recall tiers: "builtin" or "strict" (agent
    /// claims quarantined). Unknown values are rejected, never defaulted.
    #[serde(default = "default_trust_policy")]
    pub trust_policy: String,
    /// Human-turn ping channel: "stderr" or "silent".
    #[serde(default = "default_notifier")]
    pub notifier: String,
    /// Take a file lease automatically when an agent is about to edit a file, so
    /// agents never need to call `lock.acquire`. Released when the session stops.
    #[serde(default = "default_true")]
    pub auto_lease: bool,
}

fn default_mode() -> String {
    "review".to_string()
}
fn default_boot_budget() -> usize {
    2000
}
fn default_trust_policy() -> String {
    "builtin".to_string()
}
fn default_notifier() -> String {
    "stderr".to_string()
}
fn default_task_budget() -> usize {
    4000
}

impl Default for DefaultSettings {
    fn default() -> Self {
        Self {
            autopilot_mode: default_mode(),
            token_budget_boot: default_boot_budget(),
            token_budget_task: default_task_budget(),
            boot_injection: false,
            trust_policy: default_trust_policy(),
            notifier: default_notifier(),
            auto_lease: true,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProjectRegistryEntry {
    pub name: String,
    pub local_dir: String,
    #[serde(default = "default_trackable_files")]
    pub trackable_files: Vec<String>,
    #[serde(default)]
    pub exclude_patterns: Vec<String>,
    pub vault_repo: Option<String>,
    #[serde(default = "default_true")]
    pub restore_to_workspace: bool,
}

pub fn default_trackable_files() -> Vec<String> {
    vec![
        "PRD.md".to_string(),
        "docs/**/*.md".to_string(),
        "specs/**/*.md".to_string(),
        "plans/**/*.md".to_string(),
        "plan.md".to_string(),
        "PLAN.md".to_string(),
        "ROADMAP.md".to_string(),
        "*.plan.md".to_string(),
        "ARCHITECTURE.md".to_string(),
        "TODO.md".to_string(),
        ".cursorrules".to_string(),
        "AGENTS.md".to_string(),
        "CLAUDE.md".to_string(),
    ]
}

impl GlobalConfig {
    pub fn config_path() -> PathBuf {
        let home = dirs::home_dir().unwrap_or_else(|| PathBuf::from("."));
        let primary = home.join(".automemory").join("config.toml");
        let legacy = home.join(".automemory_pro").join("config.toml");
        if !primary.exists() && legacy.exists() {
            legacy
        } else {
            primary
        }
    }

    pub fn load() -> Result<Self> {
        let path = Self::config_path();
        if !path.exists() {
            return Ok(Self::default());
        }
        let content = fs::read_to_string(&path)
            .with_context(|| format!("Failed to read global config at {:?}", path))?;
        let cfg: GlobalConfig = toml::from_str(&content)
            .with_context(|| format!("Failed to parse global config at {:?}", path))?;
        Ok(cfg)
    }

    pub fn save(&self) -> Result<()> {
        let path = Self::config_path();
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)?;
        }
        let content =
            toml::to_string_pretty(self).context("Failed to serialize global config to TOML")?;
        fs::write(&path, content)
            .with_context(|| format!("Failed to write global config to {:?}", path))?;
        Ok(())
    }

    pub fn find_project_by_path(&self, cwd: &Path) -> Option<&ProjectRegistryEntry> {
        let cwd_canon = cwd.canonicalize().ok().unwrap_or_else(|| cwd.to_path_buf());
        for p in &self.projects {
            let p_dir = Path::new(&p.local_dir);
            let p_canon = p_dir
                .canonicalize()
                .ok()
                .unwrap_or_else(|| p_dir.to_path_buf());
            if cwd_canon == p_canon || cwd_canon.starts_with(&p_canon) {
                return Some(p);
            }
        }
        // Worktree fallback: if cwd is inside a linked git worktree, resolve its canonical git root
        if let Some(git_root) = crate::sync::GitSync::resolve_git_canonical_root(cwd) {
            for p in &self.projects {
                let p_dir = Path::new(&p.local_dir);
                let p_canon = p_dir
                    .canonicalize()
                    .ok()
                    .unwrap_or_else(|| p_dir.to_path_buf());
                if git_root == p_canon || git_root.starts_with(&p_canon) {
                    return Some(p);
                }
            }
        }
        None
    }

    pub fn register_project(&mut self, mut entry: ProjectRegistryEntry) -> Result<()> {
        let p = Path::new(&entry.local_dir);
        let normalized = p.canonicalize().ok().unwrap_or_else(|| p.to_path_buf());
        entry.local_dir = normalized.to_string_lossy().to_string();

        if let Some(pos) = self
            .projects
            .iter()
            .position(|p| p.name == entry.name || p.local_dir == entry.local_dir)
        {
            self.projects[pos] = entry;
        } else {
            self.projects.push(entry);
        }
        self.save()
    }

    pub fn remove_project(&mut self, name_or_path: &str) -> Result<bool> {
        let orig_len = self.projects.len();
        self.projects
            .retain(|p| p.name != name_or_path && p.local_dir != name_or_path);
        if self.projects.len() != orig_len {
            self.save()?;
            Ok(true)
        } else {
            Ok(false)
        }
    }
}

#[derive(Debug, Clone)]
pub struct Config {
    pub home_dir: PathBuf,
    pub db_path: PathBuf,
    pub hooks_dir: PathBuf,
    pub vault_dir: PathBuf,
}

impl Config {
    pub fn default_config() -> Self {
        let home = dirs::home_dir().unwrap_or_else(|| PathBuf::from("."));
        let primary = home.join(".automemory");
        let legacy = home.join(".automemory_pro");
        let base = if !primary.exists() && legacy.exists() {
            legacy
        } else {
            primary
        };
        let vault_dir = GlobalConfig::load()
            .ok()
            .and_then(|g| g.vault.local_vault_dir)
            .unwrap_or_else(|| base.join("vault"));

        Self {
            db_path: base.join("automemory.db"),
            hooks_dir: base.join("hooks"),
            vault_dir,
            home_dir: base,
        }
    }

    pub fn ensure_dirs(&self) -> std::io::Result<()> {
        std::fs::create_dir_all(&self.home_dir)?;
        std::fs::create_dir_all(&self.hooks_dir)?;
        std::fs::create_dir_all(&self.vault_dir)?;
        Ok(())
    }
}
