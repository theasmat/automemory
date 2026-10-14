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
    pub projects: Vec<ProjectRegistryEntry>,
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
            projects: Vec::new(),
        }
    }
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
}

fn default_mode() -> String {
    "review".to_string()
}
fn default_boot_budget() -> usize {
    2000
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
