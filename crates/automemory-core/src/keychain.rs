use anyhow::Result;
use base64::engine::general_purpose::STANDARD as BASE64;
use base64::Engine;
use std::path::PathBuf;

const KEYCHAIN_SERVICE: &str = "dev.automemory.master-key";
const KEYCHAIN_ACCOUNT: &str = "automemory";

/// Retrieves the 32-byte AES-256 master key from the OS Keychain, or falls back to disk.
pub fn get_master_key() -> Option<[u8; 32]> {
    // 1. Try OS Keychain on macOS
    #[cfg(target_os = "macos")]
    {
        if let Ok(output) = std::process::Command::new("/usr/bin/security")
            .args(&[
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

/// Stores the 32-byte master key into the OS Keychain and secures ~/.automemory/master.key with 0600 mode.
pub fn set_master_key(key: &[u8; 32]) -> Result<()> {
    let b64 = BASE64.encode(key);

    // 1. Save to OS Keychain on macOS
    #[cfg(target_os = "macos")]
    {
        let _ = std::process::Command::new("/usr/bin/security")
            .args(&[
                "add-generic-password",
                "-s",
                KEYCHAIN_SERVICE,
                "-a",
                KEYCHAIN_ACCOUNT,
                "-w",
                &b64,
                "-U",
            ])
            .output();
    }

    // 2. Save to ~/.automemory/master.key with strict permissions
    let key_file = get_master_key_path();
    if let Some(parent) = key_file.parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    std::fs::write(&key_file, &b64)?;

    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let _ = std::fs::set_permissions(&key_file, std::fs::Permissions::from_mode(0o600));
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
