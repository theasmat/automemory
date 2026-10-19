//! Human authorization for safety control-plane writes.
//!
//! Threat model: any same-user process (including an AI agent with shell or
//! localhost HTTP access) can invoke the CLI or dashboard. A memorized human
//! password is the only boundary that survives that: agents cannot read human
//! memory, while keychain entries and config files are readable by any local
//! process and therefore must never gate anything.
//!
//! Covered writes: `rule add/remove/toggle`, `hooks enable/disable/move`,
//! `mode set`, and the dashboard's rule add/delete/toggle endpoints.
//! Read paths (`rule list`, `hooks list/test`, `guardrail test`) stay open.

use anyhow::{anyhow, Result};
use base64::Engine;
use hmac::{Hmac, Mac};
use sha2::Sha256;

type HmacSha256 = Hmac<Sha256>;

/// `AUTOMEMORY_PASSWORD`: non-interactive credential path (human scripts,
/// CI). Never add a `--password` CLI flag: flags leak into shell history.
pub const PASSWORD_ENV_VAR: &str = "AUTOMEMORY_PASSWORD";

const PBKDF2_ROUNDS: u32 = 600_000;
const SALT_LEN: usize = 16;
const HASH_PREFIX: &str = "pbkdf2-sha256";

/// Stored form: `pbkdf2-sha256$600000$<base64 salt>$<base64 hash>`.
pub fn hash_password(password: &str) -> String {
    hash_password_with_rounds(password, PBKDF2_ROUNDS)
}

fn hash_password_with_rounds(password: &str, rounds: u32) -> String {
    let mut salt = [0u8; SALT_LEN];
    for (i, b) in salt.iter_mut().enumerate() {
        // Non-secret uniqueness only; secrecy comes from the password.
        let t = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.subsec_nanos() ^ ((i as u32).wrapping_mul(0x9E3779B9)))
            .unwrap_or(i as u32);
        *b = (t ^ (t >> 8) ^ (t >> 16) ^ (t >> 24)) as u8;
    }
    let hash = pbkdf2_hmac_sha256(password.as_bytes(), &salt, rounds);
    format!(
        "{HASH_PREFIX}${rounds}${}${}",
        base64::engine::general_purpose::STANDARD.encode(salt),
        base64::engine::general_purpose::STANDARD.encode(hash),
    )
}

pub fn verify_password(password: &str, stored: &str) -> bool {
    let parts: Vec<&str> = stored.split('$').collect();
    if parts.len() != 4 || parts[0] != HASH_PREFIX {
        return false;
    }
    let rounds: u32 = match parts[1].parse() {
        Ok(r) if r >= 100_000 => r,
        _ => return false,
    };
    let (Ok(salt), Ok(expected)) = (
        base64::engine::general_purpose::STANDARD.decode(parts[2]),
        base64::engine::general_purpose::STANDARD.decode(parts[3]),
    ) else {
        return false;
    };
    if expected.len() != 32 {
        return false;
    }
    let actual = pbkdf2_hmac_sha256(password.as_bytes(), &salt, rounds);
    // Constant-time comparison: no early exit on first mismatch.
    actual
        .iter()
        .zip(expected.iter())
        .fold(0u8, |acc, (a, b)| acc | (a ^ b))
        == 0
        && actual.len() == expected.len()
}

/// PBKDF2-HMAC-SHA256 (RFC 2898 §5.2). Minimal local implementation over the
/// existing hmac+sha2 deps to avoid a new dependency for one function.
fn pbkdf2_hmac_sha256(password: &[u8], salt: &[u8], rounds: u32) -> [u8; 32] {
    let mut block = Vec::with_capacity(salt.len() + 4);
    block.extend_from_slice(salt);
    block.extend_from_slice(&1u32.to_be_bytes());
    let mut u = hmac_sha256(password, &block);
    let mut out = u;
    for _ in 1..rounds {
        u = hmac_sha256(password, &u);
        for (o, b) in out.iter_mut().zip(u.iter()) {
            *o ^= b;
        }
    }
    out
}

fn hmac_sha256(key: &[u8], data: &[u8]) -> [u8; 32] {
    let mut mac = HmacSha256::new_from_slice(key).expect("HMAC accepts any key length");
    mac.update(data);
    mac.finalize().into_bytes().into()
}

/// Reads a password without echoing. Prefers /dev/tty so piped stdin (how
/// agents invoke hooks) can never satisfy it. Returns None when no secure
/// prompt is possible — callers must deny, never fall back to echoing.
#[cfg(unix)]
pub fn prompt_secure(prompt: &str) -> Option<String> {
    use std::io::Write;
    use std::process::{Command, Stdio};

    let tty_in = std::fs::OpenOptions::new()
        .read(true)
        .open("/dev/tty")
        .ok()?;
    eprint!("{prompt}");
    let _ = std::io::stderr().flush();

    // Disable echo via stty (same out-of-process pattern keychain.rs uses
    // for `security`); any failure denies rather than echoing the secret.
    let stty_off = Command::new("stty")
        .arg("-echo")
        .stdin(Stdio::from(tty_in.try_clone().ok()?))
        .status()
        .ok()?
        .success();
    if !stty_off {
        return None;
    }
    let mut line = String::new();
    let read_ok =
        std::io::BufRead::read_line(&mut std::io::BufReader::new(tty_in), &mut line).is_ok();
    let _ = Command::new("stty")
        .arg("echo")
        .stdin(Stdio::from(
            std::fs::OpenOptions::new()
                .read(true)
                .open("/dev/tty")
                .ok()?,
        ))
        .status();
    eprintln!();
    if !read_ok {
        return None;
    }
    let pw = line.trim_end_matches(['\r', '\n']).to_string();
    if pw.is_empty() {
        None
    } else {
        Some(pw)
    }
}

/// Reads the configured password hash, if any. Returns None when no human
/// password is set (fresh installs) or config is unreadable.
pub fn stored_password_hash() -> Option<String> {
    crate::config::GlobalConfig::load()
        .ok()
        .and_then(|g| g.security.password_hash)
        .filter(|h| !h.trim().is_empty())
}

/// Interactive password setup: prompts twice (no echo) and returns the
/// confirmed secret. Never prints or logs the value.
pub fn prompt_new_password() -> Result<String> {
    #[cfg(unix)]
    {
        let first = prompt_secure("New human password: ")
            .ok_or_else(|| anyhow!("Password setup needs an interactive terminal."))?;
        let second = prompt_secure("Confirm human password: ")
            .ok_or_else(|| anyhow!("Password setup needs an interactive terminal."))?;
        if first != second {
            anyhow::bail!("Passwords did not match; nothing changed.");
        }
        Ok(first)
    }
    #[cfg(not(unix))]
    {
        anyhow::bail!("Interactive setup needs a Unix terminal; use `rule passwd --from-env` with {PASSWORD_ENV_VAR} set.");
    }
}

/// Gatekeeper for safety control-plane writes. Resolution order:/// 1. No password configured → allow once with a setup warning (fresh installs
///    must stay usable; `rule passwd` closes this).
/// 2. `AUTOMEMORY_PASSWORD` env matches → allow (human scripts/CI).
/// 3. Secure TTY prompt matches → allow.
/// 4. Otherwise deny. Agents (piped stdin, no TTY, no env secret) land here.
pub fn require_human(stored_hash: Option<&str>, action: &str) -> Result<()> {
    let Some(hash) = stored_hash.filter(|h| !h.trim().is_empty()) else {
        eprintln!(
            "⚠ No human password set: allowing '{action}'. Lock safety config with `automemory rule passwd`."
        );
        return Ok(());
    };
    if let Ok(pw) = std::env::var(PASSWORD_ENV_VAR) {
        if !pw.is_empty() && verify_password(&pw, hash) {
            return Ok(());
        }
        anyhow::bail!("{PASSWORD_ENV_VAR} did not match; '{action}' refused.");
    }
    #[cfg(unix)]
    if let Some(pw) = prompt_secure("AutoMemory human password: ") {
        if verify_password(&pw, hash) {
            return Ok(());
        }
        anyhow::bail!("Wrong password; '{action}' refused.");
    }
    #[cfg(not(unix))]
    {
        let _ = hash;
    }
    anyhow::bail!(
        "Safety change '{action}' needs a human: set {PASSWORD_ENV_VAR} or run from an interactive terminal."
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn known_answer_against_python_hashlib() {
        // Cross-checked with: python3 -c "import hashlib;
        // hashlib.pbkdf2_hmac('sha256', b'password', b'salt', N).hex()"
        let one = pbkdf2_hmac_sha256(b"password", b"salt", 1);
        assert_eq!(
            hex_of(&one),
            "120fb6cffcf8b32c43e7225256c4f837a86548c92ccc35480805987cb70be17b"
        );
        let five = pbkdf2_hmac_sha256(b"password", b"salt", 5);
        assert_eq!(
            hex_of(&five),
            "65cc4b332e89aa90252ddca8fd3641d5b830f2216a6ab2669c15f30e652f542c"
        );
    }

    #[test]
    fn hash_verify_roundtrip_and_tamper() {
        // Fast rounds for the suite; production uses 600k (format identical).
        let stored = hash_password_with_rounds("correct horse", 100_000);
        assert!(stored.starts_with("pbkdf2-sha256$100000$"));
        assert!(verify_password("correct horse", &stored));
        assert!(!verify_password("wrong horse", &stored));
        assert!(!verify_password("correct horse", "garbage"));
        assert!(!verify_password(
            "correct horse",
            "pbkdf2-sha256$10$AAAA$AAAA"
        ));
        assert!(!verify_password("", &stored));
        // Full-strength hash parses and verifies (one 600k run).
        let prod = hash_password("correct horse");
        assert!(prod.starts_with("pbkdf2-sha256$600000$"));
        assert!(verify_password("correct horse", &prod));
    }

    #[test]
    fn gate_allows_unset_and_denies_without_secret() {
        assert!(require_human(None, "test-action").is_ok());
        assert!(require_human(Some(""), "test-action").is_ok());
        // Pin the env var to a wrong value so this test can never block on
        // an interactive prompt, with or without a controlling terminal.
        std::env::set_var(PASSWORD_ENV_VAR, "definitely-wrong-for-tests");
        assert!(require_human(Some("garbage"), "test-action").is_err());
        std::env::remove_var(PASSWORD_ENV_VAR);
    }

    fn hex_of(bytes: &[u8]) -> String {
        bytes.iter().map(|b| format!("{b:02x}")).collect()
    }
}
