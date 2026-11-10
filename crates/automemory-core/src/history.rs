//! Who changed a file, and why, when Tracewise (`tw`) is installed.
//!
//! Optional, like the CodeIntel bridge: AutoMemory reads `tw file <path> --json` and never opens
//! Tracewise's database. With `tw` absent or failing the answer is empty and nothing else changes.
use serde_json::Value;
use std::io::Read;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::time::Duration;

const CALL_TIMEOUT: Duration = Duration::from_secs(3);

/// One recorded change to a file.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FileChange {
    /// `claude`, `codex`, a person's name, ...
    pub actor: String,
    /// `agent`, `human`, `script`, ...
    pub kind: String,
    pub session: Option<String>,
    /// RFC 3339 end time of the change.
    pub when: String,
    /// The recorded reason, if one was given.
    pub why: Option<String>,
}

impl FileChange {
    /// `claude/f8c93710, 2026-11-10T13:10, "reason"` for one line of output.
    pub fn describe(&self) -> String {
        let who = match &self.session {
            Some(s) => format!("{}/{}", self.actor, s.chars().take(8).collect::<String>()),
            None => self.actor.clone(),
        };
        let when = self.when.chars().take(16).collect::<String>();
        match &self.why {
            Some(w) => format!("{who}, {when}, \"{w}\""),
            None => format!("{who}, {when}"),
        }
    }
}

/// Answers "who changed this file". Both implementations are total: they never fail.
pub trait ChangeHistory {
    /// Newest first, at most `limit`. Empty when unknown.
    fn recent_changes(&self, path: &str, limit: usize) -> Vec<FileChange>;
}

/// Used when `tw` is not installed (or `[history] provider = "off"`).
pub struct NoHistory;

impl ChangeHistory for NoHistory {
    fn recent_changes(&self, _path: &str, _limit: usize) -> Vec<FileChange> {
        Vec::new()
    }
}

pub struct TwHistory {
    bin: PathBuf,
    root: PathBuf,
}

/// Locates `tw`: `AUTOMEMORY_TW_BIN`, then `PATH` (`tw`, `tracewise`), then `~/.tracewise/bin`.
pub fn find_tw() -> Option<PathBuf> {
    if let Some(p) = std::env::var_os("AUTOMEMORY_TW_BIN").map(PathBuf::from) {
        return p.is_file().then_some(p);
    }
    let on_path = std::env::var_os("PATH").and_then(|paths| {
        std::env::split_paths(&paths).flat_map(|d| [d.join("tw"), d.join("tracewise")]).find(|p| p.is_file())
    });
    on_path.or_else(|| {
        dirs::home_dir()
            .map(|h| h.join(".tracewise/bin"))
            .and_then(|d| [d.join("tw"), d.join("tracewise")].into_iter().find(|p| p.is_file()))
    })
}

/// `tracewise 0.3.0`, if `tw` runs.
pub fn tw_version() -> Option<String> {
    let out = Command::new(find_tw()?).arg("--version").output().ok()?;
    out.status.success().then(|| String::from_utf8_lossy(&out.stdout).trim().to_string())
}

/// The history provider for a project: `tw` when installed, otherwise nothing.
pub fn detect(project_root: &Path) -> Box<dyn ChangeHistory> {
    match TwHistory::detect(project_root) {
        Some(tw) => Box::new(tw),
        None => Box::new(NoHistory),
    }
}

impl TwHistory {
    pub fn detect(project_root: &Path) -> Option<Self> {
        Some(Self { bin: find_tw()?, root: project_root.to_path_buf() })
    }
}

impl ChangeHistory for TwHistory {
    fn recent_changes(&self, path: &str, limit: usize) -> Vec<FileChange> {
        let Ok(mut child) = Command::new(&self.bin)
            .args(["file", path, "--json"])
            .current_dir(&self.root)
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .spawn()
        else {
            return Vec::new();
        };
        let Some(mut stdout) = child.stdout.take() else { return Vec::new() };
        // Drain on a thread so a large reply cannot block the child and look like a timeout.
        let (tx, rx) = std::sync::mpsc::channel();
        std::thread::spawn(move || {
            let mut buf = String::new();
            let _ = stdout.read_to_string(&mut buf);
            let _ = tx.send(buf);
        });
        let Ok(raw) = rx.recv_timeout(CALL_TIMEOUT) else {
            let _ = child.kill();
            let _ = child.wait();
            return Vec::new();
        };
        let _ = child.wait();
        parse_file_json(&raw, limit)
    }
}

/// Parses `tw file --json` output. Baseline rows (the starting state) are not changes by anyone.
pub fn parse_file_json(raw: &str, limit: usize) -> Vec<FileChange> {
    let Ok(v) = serde_json::from_str::<Value>(raw) else { return Vec::new() };
    let Some(changes) = v["changes"].as_array() else { return Vec::new() };
    changes
        .iter()
        .filter_map(|c| {
            let kind = c["actor"]["kind"].as_str()?;
            if kind == "baseline" {
                return None;
            }
            Some(FileChange {
                actor: c["actor"]["name"].as_str().unwrap_or(kind).to_string(),
                kind: kind.to_string(),
                session: c["actor"]["session"].as_str().map(str::to_string),
                when: c["time"]["end"].as_str().unwrap_or_default().to_string(),
                why: c["why"].as_str().filter(|w| !w.is_empty()).map(str::to_string),
            })
        })
        .take(limit)
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    const SAMPLE: &str = r#"{"path":"a.rs","lock":null,"changes":[
      {"actor":{"kind":"agent","name":"codex","session":"abcdef0123456789"},"time":{"end":"2026-11-10T13:10:16.948Z"},"why":"handle empty input"},
      {"actor":{"kind":"human","name":"asmat"},"time":{"end":"2026-11-10T09:00:00.000Z"},"why":null},
      {"actor":{"kind":"baseline"},"time":{"end":"2026-11-09T08:00:00.000Z"},"why":null}]}"#;

    #[test]
    fn parses_actor_session_and_reason_and_skips_the_baseline() {
        let c = parse_file_json(SAMPLE, 10);
        assert_eq!(c.len(), 2);
        assert_eq!(c[0].actor, "codex");
        assert_eq!(c[0].session.as_deref(), Some("abcdef0123456789"));
        assert_eq!(c[0].why.as_deref(), Some("handle empty input"));
        assert_eq!(c[0].describe(), "codex/abcdef01, 2026-11-10T13:10, \"handle empty input\"");
        assert_eq!(c[1].describe(), "asmat, 2026-11-10T09:00");
    }

    #[test]
    fn respects_the_limit_and_survives_garbage() {
        assert_eq!(parse_file_json(SAMPLE, 1).len(), 1);
        assert!(parse_file_json("not json", 5).is_empty());
        assert!(parse_file_json("{}", 5).is_empty());
    }

    #[test]
    fn no_history_is_empty() {
        assert!(NoHistory.recent_changes("x", 3).is_empty());
    }
}
