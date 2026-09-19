use anyhow::Result;
use automemory_core::config::Config;
use automemory_core::db::Database;
use automemory_core::sync::{GitSync, VaultManager};

pub fn export_vault(path: Option<String>) -> Result<()> {
    let config = Config::default_config();
    let vault_path = path.map(std::path::PathBuf::from).unwrap_or(config.vault_dir);
    let db = Database::open(&config.db_path)?;

    let manager = VaultManager::new(&db, &vault_path);
    manager.export_to_vault()?;

    println!("✓ Exported database to vault at: {:?}", vault_path);
    Ok(())
}

pub fn import_vault(path: Option<String>) -> Result<()> {
    let config = Config::default_config();
    let vault_path = path.map(std::path::PathBuf::from).unwrap_or(config.vault_dir);
    let db = Database::open(&config.db_path)?;

    let manager = VaultManager::new(&db, &vault_path);
    manager.import_from_vault()?;

    println!("✓ Imported records from vault at: {:?}", vault_path);
    Ok(())
}

pub fn status() -> Result<()> {
    let config = Config::default_config();
    let is_git = GitSync::is_git_repo(&config.vault_dir);
    println!("Vault path: {:?}", config.vault_dir);
    println!("Git repository: {}", if is_git { "Initialized" } else { "Not initialized (run 'git init' inside vault or set git_remote in project)" });
    Ok(())
}
