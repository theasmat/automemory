//! An agent keeps its MCP server running for days. After `automemory update` (or `cargo install`) that server is still the old code: it kept writing duplicate skills and ran
//! old room rules. So between two requests the server checks whether the installed binary changed and, if it did, starts the new one in its place on the same pipes
//! (`exec`: same process id, same stdin and stdout, the agent notices nothing). Unix only; elsewhere `doctor` still names servers to restart by hand.
//!
//! shortcut: it only restarts when no further input is waiting in the read buffer, and agents ask one question at a time; a client that pipelines requests is simply served
//! by the old code until a quiet moment. Upgrade if a client is seen to pipeline for long.

use std::path::PathBuf;
use std::time::{Duration, SystemTime};

const MAX_RESTARTS: u32 = 5;
pub const RESTARTS_ENV: &str = "AUTOMEMORY_MCP_RESTARTS";
pub const CLIENT_ENV: &str = "AUTOMEMORY_MCP_CLIENT";
/// The request that was being read when the server restarted: the new process answers it first, so the call that noticed the update is already served by the new code.
pub const PENDING_ENV: &str = "AUTOMEMORY_MCP_PENDING";
const PENDING_MAX: usize = 64_000;

/// What identifies a version of the installed file: replacing it (new inode or time or size) changes this.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BinaryStamp {
    modified: Option<SystemTime>,
    len: u64,
    #[cfg(unix)]
    inode: u64,
}

fn exe_path() -> Option<PathBuf> {
    let p = std::env::current_exe().ok()?;
    // Linux appends " (deleted)" to the path of a binary that was replaced while running.
    let s = p.to_string_lossy();
    Some(PathBuf::from(s.strip_suffix(" (deleted)").unwrap_or(&s).to_string()))
}

impl BinaryStamp {
    pub fn of(path: &std::path::Path) -> Option<Self> {
        let m = std::fs::metadata(path).ok()?;
        #[cfg(unix)]
        let inode = std::os::unix::fs::MetadataExt::ino(&m);
        Some(Self { modified: m.modified().ok(), len: m.len(), #[cfg(unix)] inode })
    }
}

pub struct Restarter {
    path: Option<PathBuf>,
    started_as: Option<BinaryStamp>,
}

impl Restarter {
    pub fn new() -> Self {
        let path = exe_path();
        let started_as = path.as_deref().and_then(BinaryStamp::of);
        Self { path, started_as }
    }

    /// True when the file at our path is no longer the one we started from.
    pub fn is_stale(&self) -> bool {
        match (&self.path, &self.started_as) {
            (Some(p), Some(then)) => BinaryStamp::of(p).is_some_and(|now| now != *then),
            _ => false,
        }
    }

    /// Replaces this process with the installed binary (same arguments, same pipes). Returns only if it did not restart: on a platform without `exec`, after too many
    /// restarts, or when the new binary does not even answer `--version` (a half-written file must not cut the agent off).
    pub fn restart(&self, client: Option<&str>, pending: Option<&str>) {
        #[cfg(unix)]
        {
            use std::os::unix::process::CommandExt;
            let Some(path) = &self.path else { return };
            let count: u32 = std::env::var(RESTARTS_ENV).ok().and_then(|v| v.parse().ok()).unwrap_or(0);
            if count >= MAX_RESTARTS || !answers_version(path) {
                return;
            }
            eprintln!("[automemory] a newer automemory is installed: the MCP server is restarting (restart {} of {MAX_RESTARTS} at most)", count + 1);
            let mut cmd = std::process::Command::new(path);
            cmd.args(std::env::args_os().skip(1)).env(RESTARTS_ENV, (count + 1).to_string());
            if let Some(c) = client {
                cmd.env(CLIENT_ENV, c);
            }
            match pending {
                Some(p) if p.len() <= PENDING_MAX => {
                    cmd.env(PENDING_ENV, p);
                }
                Some(_) => return, // too big to hand over in the environment: restart at the next quiet moment
                None => {}
            }
            let err = cmd.exec(); // only returns on failure
            eprintln!("[automemory] restart failed ({err}); continuing with the running version");
        }
        #[cfg(not(unix))]
        let _ = (client, pending);
    }
}

impl Default for Restarter {
    fn default() -> Self {
        Self::new()
    }
}

/// The new file runs and says its version within three seconds.
#[cfg(unix)]
fn answers_version(path: &std::path::Path) -> bool {
    let Ok(mut child) = std::process::Command::new(path).arg("--version").stdin(std::process::Stdio::null()).stdout(std::process::Stdio::null()).stderr(std::process::Stdio::null()).spawn() else { return false };
    let deadline = std::time::Instant::now() + Duration::from_secs(3);
    loop {
        match child.try_wait() {
            Ok(Some(status)) => return status.success(),
            Ok(None) if std::time::Instant::now() < deadline => std::thread::sleep(Duration::from_millis(20)),
            _ => {
                let _ = child.kill();
                return false;
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_replaced_file_has_a_different_stamp() {
        let dir = std::env::temp_dir().join(format!("am-restart-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let f = dir.join("bin");
        std::fs::write(&f, "one").unwrap();
        let a = BinaryStamp::of(&f).unwrap();
        assert_eq!(a, BinaryStamp::of(&f).unwrap(), "untouched, the same");
        let tmp = dir.join("bin.new");
        std::fs::write(&tmp, "one").unwrap();
        std::fs::rename(&tmp, &f).unwrap();
        assert_ne!(a, BinaryStamp::of(&f).unwrap(), "same bytes but a replaced file still counts as new");
        assert!(BinaryStamp::of(&dir.join("missing")).is_none());
        let _ = std::fs::remove_dir_all(dir);
    }
}
