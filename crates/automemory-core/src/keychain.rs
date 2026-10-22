use anyhow::Result;
use base64::engine::general_purpose::STANDARD as BASE64;
use base64::Engine;
use std::path::PathBuf;

const KEYCHAIN_SERVICE: &str = "dev.automemory.master-key";
const KEYCHAIN_ACCOUNT: &str = "automemory";

/// Retrieves the 32-byte AES-256 master key from the OS Keychain, or falls back to disk.
pub fn get_master_key() -> Option<[u8; 32]> {
    // 0. The keyring entry written by set_master_key
    if std::env::var("AUTOMEMORY_SECRETS_BACKEND").map_or(true, |v| v != "file") {
        if let Some(b64) = keyring::Entry::new(KEYCHAIN_SERVICE, KEYCHAIN_ACCOUNT).ok().and_then(|e| e.get_password().ok()) {
            if let Ok(bytes) = BASE64.decode(b64.trim()) {
                if bytes.len() == 32 {
                    let mut key = [0u8; 32];
                    key.copy_from_slice(&bytes);
                    return Some(key);
                }
            }
        }
    }
    // 1. Try OS Keychain on macOS
    #[cfg(target_os = "macos")]
    {
        if let Ok(output) = std::process::Command::new("/usr/bin/security")
            .args([
                "find-generic-password",
                "-s",
                KEYCHAIN_SERVICE,
                "-a",
                KEYCHAIN_ACCOUNT,
                "-w",
            ])
            .output()
        {
            if output.status.success() {
                let trimmed = String::from_utf8_lossy(&output.stdout).trim().to_string();
                if let Ok(bytes) = BASE64.decode(&trimmed) {
                    if bytes.len() == 32 {
                        let mut key = [0u8; 32];
                        key.copy_from_slice(&bytes);
                        return Some(key);
                    }
                }
            }
        }
    }

    // 2. Fallback to ~/.automemory/master.key on disk
    let key_file = get_master_key_path();
    if key_file.exists() {
        if let Ok(content) = std::fs::read_to_string(&key_file) {
            let trimmed = content.trim();
            if let Ok(bytes) = BASE64.decode(trimmed) {
                if bytes.len() == 32 {
                    let mut key = [0u8; 32];
                    key.copy_from_slice(&bytes);
                    return Some(key);
                }
            }
        }
    }

    None
}

/// Stores the 32-byte master key into the OS Keychain (or falls back to ~/.automemory/master.key with 0600 mode only if keychain is unavailable).
pub fn set_master_key(key: &[u8; 32]) -> Result<()> {
    let b64 = BASE64.encode(key);
    let mut stored_in_keychain = false;

    // 1. OS keychain through the keyring crate (the key never appears on a command line)
    if std::env::var("AUTOMEMORY_SECRETS_BACKEND").map_or(true, |v| v != "file") {
        if let Ok(e) = keyring::Entry::new(KEYCHAIN_SERVICE, KEYCHAIN_ACCOUNT) {
            // Read it back: only then is the plaintext file safe to remove.
            if e.set_password(&b64).is_ok() && e.get_password().ok().as_deref() == Some(b64.as_str()) {
                stored_in_keychain = true;
            }
        }
    }

    let key_file = get_master_key_path();
    if stored_in_keychain {
        // If successfully stored in the OS keychain, clean up any plaintext fallback file
        if key_file.exists() {
            let _ = std::fs::remove_file(&key_file);
        }
    } else {
        // 2. Fallback to ~/.automemory/master.key with strict permissions only when keychain is not available
        if let Some(parent) = key_file.parent() {
            let _ = std::fs::create_dir_all(parent);
        }
        #[cfg(unix)]
        {
            use std::io::Write;
            use std::os::unix::fs::OpenOptionsExt;
            let mut opts = std::fs::OpenOptions::new();
            opts.write(true).create(true).truncate(true).mode(0o600);
            if let Ok(mut f) = opts.open(&key_file) {
                let _ = f.write_all(b64.as_bytes());
            } else {
                std::fs::write(&key_file, &b64)?;
            }
        }
        #[cfg(not(unix))]
        {
            std::fs::write(&key_file, &b64)?;
        }
    }

    Ok(())
}

/// Returns whether native Apple Keychain integration is active.
pub fn is_native_keychain_active() -> bool {
    cfg!(target_os = "macos")
}

pub fn get_master_key_path() -> PathBuf {
    let home = dirs::home_dir().unwrap_or_else(|| PathBuf::from("."));
    home.join(".automemory").join("master.key")
}
