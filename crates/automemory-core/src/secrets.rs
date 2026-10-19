//! Named secrets (cloud API keys) that must never appear in a file we write, the database, a log or a
//! command line. The OS keychain holds them (macOS Keychain, Windows Credential Manager, Linux Secret
//! Service through the `keyring` crate). Where no keychain exists, an encrypted file is used instead.
//! `AUTOMEMORY_SECRETS_BACKEND=file` forces the file (tests, headless machines).

use crate::crypto::{BlindCrypto, EncryptedBlob};
use anyhow::{Context, Result};
use std::collections::BTreeMap;
use std::path::PathBuf;

const SERVICE: &str = "dev.automemory.secret";

/// Where a secret was found (never the secret itself).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Source {
    Env(String),
    Keychain,
    EncryptedFile,
    None,
}

impl Source {
    pub fn label(&self) -> String {
        match self {
            Self::Env(v) => format!("environment variable {v}"),
            Self::Keychain => "OS keychain".to_string(),
            Self::EncryptedFile => "encrypted file (no keychain on this machine)".to_string(),
            Self::None => "not set".to_string(),
        }
    }
}

fn entry(name: &str) -> Option<keyring::Entry> {
    if std::env::var("AUTOMEMORY_SECRETS_BACKEND").is_ok_and(|v| v == "file") {
        return None;
    }
    keyring::Entry::new(SERVICE, name).ok()
}

fn file_path() -> PathBuf {
    dirs::home_dir().unwrap_or_else(|| PathBuf::from(".")).join(".automemory").join("secrets.enc")
}

fn file_read() -> BTreeMap<String, String> {
    let Ok(bytes) = std::fs::read(file_path()) else { return BTreeMap::new() };
    let Ok(blob) = serde_json::from_slice::<EncryptedBlob>(&bytes) else { return BTreeMap::new() };
    let Ok(key) = BlindCrypto::get_or_create_master_key(None) else { return BTreeMap::new() };
    BlindCrypto::decrypt(&key, &blob)
        .ok()
        .and_then(|p| serde_json::from_slice(&p).ok())
        .unwrap_or_default()
}

fn file_write(map: &BTreeMap<String, String>) -> Result<()> {
    let path = file_path();
    if map.is_empty() {
        let _ = std::fs::remove_file(&path);
        return Ok(());
    }
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let key = BlindCrypto::get_or_create_master_key(None)?;
    let blob = BlindCrypto::encrypt(&key, &serde_json::to_vec(map)?)?;
    std::fs::write(&path, serde_json::to_vec(&blob)?).context("cannot write the secrets file")?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let _ = std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o600));
    }
    Ok(())
}

/// Stores a secret; the keychain when it works (verified by reading it back), else the encrypted file.
pub fn set(name: &str, value: &str) -> Result<Source> {
    if let Some(e) = entry(name) {
        if e.set_password(value).is_ok() && e.get_password().ok().as_deref() == Some(value) {
            let mut map = file_read();
            if map.remove(name).is_some() {
                let _ = file_write(&map); // a copy left from an earlier fallback is removed
            }
            return Ok(Source::Keychain);
        }
    }
    let mut map = file_read();
    map.insert(name.to_string(), value.to_string());
    file_write(&map)?;
    Ok(Source::EncryptedFile)
}

pub fn get(name: &str) -> Option<String> {
    if let Some(v) = entry(name).and_then(|e| e.get_password().ok()) {
        return Some(v);
    }
    file_read().get(name).cloned()
}

/// Removes the secret everywhere it may live. True when something was removed.
pub fn delete(name: &str) -> Result<bool> {
    let mut removed = entry(name).is_some_and(|e| e.delete_credential().is_ok());
    let mut map = file_read();
    if map.remove(name).is_some() {
        file_write(&map)?;
        removed = true;
    }
    Ok(removed)
}

/// Where a secret would come from, with an optional environment variable taking priority.
pub fn status(name: &str, env_var: Option<&str>) -> Source {
    if let Some(v) = env_var.filter(|v| !v.is_empty()) {
        if std::env::var(v).is_ok_and(|s| !s.is_empty()) {
            return Source::Env(v.to_string());
        }
    }
    if entry(name).is_some_and(|e| e.get_password().is_ok()) {
        return Source::Keychain;
    }
    if file_read().contains_key(name) {
        return Source::EncryptedFile;
    }
    Source::None
}

/// The value: the environment variable first, then the keychain or file.
pub fn resolve(name: &str, env_var: Option<&str>) -> Option<String> {
    env_var
        .filter(|v| !v.is_empty())
        .and_then(|v| std::env::var(v).ok())
        .filter(|s| !s.is_empty())
        .or_else(|| get(name))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// One test mutates process-wide environment, so everything runs in a single test.
    #[test]
    fn secrets_round_trip_in_the_encrypted_file_and_never_leak() {
        let home = std::env::temp_dir().join(format!("am-secrets-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&home);
        std::fs::create_dir_all(&home).unwrap();
        std::env::set_var("HOME", &home);
        std::env::set_var("AUTOMEMORY_SECRETS_BACKEND", "file");
        std::env::set_var("AUTOMEMORY_KEY", "test-master");

        assert_eq!(status("embeddings.cloud", None), Source::None);
        assert_eq!(set("embeddings.cloud", "sk-PLANTED-123456").unwrap(), Source::EncryptedFile);
        assert_eq!(get("embeddings.cloud").as_deref(), Some("sk-PLANTED-123456"));
        assert_eq!(status("embeddings.cloud", None), Source::EncryptedFile);

        // The file is ciphertext: the key is not in it.
        let raw = std::fs::read_to_string(file_path()).unwrap();
        assert!(!raw.contains("PLANTED"), "secret must not be readable in the file");

        // The environment variable wins over the stored value.
        std::env::set_var("AM_TEST_EMB_KEY", "from-env");
        assert_eq!(resolve("embeddings.cloud", Some("AM_TEST_EMB_KEY")).as_deref(), Some("from-env"));
        assert_eq!(status("embeddings.cloud", Some("AM_TEST_EMB_KEY")), Source::Env("AM_TEST_EMB_KEY".into()));
        std::env::remove_var("AM_TEST_EMB_KEY");

        assert!(delete("embeddings.cloud").unwrap());
        assert_eq!(get("embeddings.cloud"), None);
        assert!(!delete("embeddings.cloud").unwrap());
        assert!(!file_path().exists(), "an empty store leaves no file");
        let _ = std::fs::remove_dir_all(&home);
    }
}
