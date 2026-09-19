use std::path::PathBuf;

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
        let base = home.join(".automemory_pro");
        Self {
            db_path: base.join("automemory.db"),
            hooks_dir: base.join("hooks"),
            vault_dir: base.join("vault"),
            home_dir: base,
        }
    }

    pub fn with_db_path(mut self, path: PathBuf) -> Self {
        self.db_path = path;
        self
    }

    pub fn ensure_dirs(&self) -> std::io::Result<()> {
        std::fs::create_dir_all(&self.home_dir)?;
        std::fs::create_dir_all(&self.hooks_dir)?;
        std::fs::create_dir_all(&self.vault_dir)?;
        Ok(())
    }
}
