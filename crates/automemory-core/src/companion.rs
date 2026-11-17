//! Tracewise as AutoMemory's companion (docs/specs/tracewise-companion.md). AutoMemory's own hook calls `tracewise hook --neutral` inside itself,
//! with the agent's raw payload, and merges the answer with its own. Everything here fails open: no Tracewise, a slow one, an unknown API number or
//! unreadable output all mean "no companion for this call", never an error for the agent.

use crate::config::CompanionSettings;
use crate::db::Database;
use serde::Deserialize;
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

/// The companion API this build understands.
pub const API: u64 = 1;
const PROBE_TTL_SECS: i64 = 60;
const PROBE_KEY: &str = "companion_probe";

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Companion {
    pub path: PathBuf,
    pub version: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Decision {
    Allow,
    Ask,
    Deny,
}

/// One answer of `tracewise hook --neutral`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Answer {
    pub event: String,
    pub decision: Decision,
    pub reason: Option<String>,
    pub context: Vec<String>,
}

#[derive(Deserialize)]
struct RawAnswer {
    api: u64,
    #[serde(default)]
    event: String,
    #[serde(default)]
    decision: String,
    reason: Option<String>,
    #[serde(default)]
    context: Vec<String>,
}

fn candidates(settings: &CompanionSettings) -> Vec<PathBuf> {
    let mut out: Vec<PathBuf> = Vec::new();
    if let Ok(p) = std::env::var("AUTOMEMORY_TRACEWISE_BIN") {
        out.push(PathBuf::from(p));
    }
    if let Some(p) = &settings.path {
        out.push(PathBuf::from(p));
    }
    for dir in std::env::split_paths(&std::env::var_os("PATH").unwrap_or_default()) {
        out.push(dir.join("tracewise"));
    }
    if let Some(home) = dirs::home_dir() {
        out.push(home.join(".tracewise/bin/tracewise"));
    }
    out
}

/// Runs `program args` with `stdin`, giving up after `timeout`. None on any failure or timeout.
fn run(program: &Path, args: &[&str], stdin: &str, timeout: Duration) -> Option<String> {
    let mut child = Command::new(program).args(args).stdin(Stdio::piped()).stdout(Stdio::piped()).stderr(Stdio::null()).spawn().ok()?;
    if let Some(mut si) = child.stdin.take() {
        let _ = si.write_all(stdin.as_bytes());
    }
    let start = Instant::now();
    loop {
        match child.try_wait() {
            Ok(Some(status)) => {
                let mut out = String::new();
                child.stdout.take()?.read_to_string(&mut out).ok()?;
                return status.success().then_some(out);
            }
            Ok(None) if start.elapsed() < timeout => std::thread::sleep(Duration::from_millis(1)),
            _ => {
                let _ = child.kill();
                let _ = child.wait();
                return None;
            }
        }
    }
}

pub fn probe(path: &Path) -> Option<Companion> {
    let out = run(path, &["companion", "--json"], "", Duration::from_millis(1500))?;
    let v: serde_json::Value = serde_json::from_str(out.trim()).ok()?;
    (v["api"].as_u64() == Some(API)).then(|| Companion { path: path.to_path_buf(), version: v["version"].as_str().unwrap_or("").to_string() })
}

/// The companion to use, or None. Looked up at most once a minute (cached in the database), so a hook does not spawn a probe each call.
pub fn detect(settings: &CompanionSettings, db: &Database) -> Option<Companion> {
    if settings.tracewise.eq_ignore_ascii_case("off") {
        return None;
    }
    let now = chrono::Utc::now().timestamp();
    if let Some(v) = db.sync_kv_get(PROBE_KEY).and_then(|j| serde_json::from_str::<serde_json::Value>(&j).ok()) {
        if now - v["at"].as_i64().unwrap_or(0) < PROBE_TTL_SECS {
            return v["path"].as_str().filter(|p| !p.is_empty()).map(|p| Companion { path: PathBuf::from(p), version: v["version"].as_str().unwrap_or("").to_string() });
        }
    }
    let found = candidates(settings).into_iter().filter(|p| p.is_file()).find_map(|p| probe(&p));
    db.sync_kv_set(
        PROBE_KEY,
        &serde_json::json!({ "at": now, "path": found.as_ref().map(|c| c.path.display().to_string()).unwrap_or_default(), "version": found.as_ref().map(|c| c.version.clone()).unwrap_or_default() })
            .to_string(),
    );
    found
}

/// The Tracewise names of the agents AutoMemory knows by other names.
pub fn tracewise_agent(agent: &str) -> &str {
    match agent {
        "agy" => "antigravity",
        "gemini-cli" => "gemini",
        other => other,
    }
}

/// The event name a Tracewise hook expects, from AutoMemory's.
pub fn tracewise_event(event: &str) -> Option<&'static str> {
    match event {
        "pre-tool" => Some("PreToolUse"),
        "post-tool" | "post-tool-fail" => Some("PostToolUse"),
        "prompt" | "user-prompt-submit" => Some("UserPromptSubmit"),
        "stop" | "task-done" => Some("Stop"),
        "session-end" => Some("SessionEnd"),
        _ => None,
    }
}

/// Calls the companion with the agent's raw payload. None when it did not answer in time or in the API this build knows.
pub fn call(c: &Companion, agent: &str, event: &str, payload: &str, timeout: Duration) -> Option<Answer> {
    let ev = tracewise_event(event)?;
    let out = run(&c.path, &["hook", ev, "--from", tracewise_agent(agent), "--neutral"], payload, timeout)?;
    let raw: RawAnswer = serde_json::from_str(out.lines().last()?.trim()).ok()?;
    if raw.api != API {
        return None;
    }
    Some(Answer {
        event: raw.event,
        decision: match raw.decision.as_str() {
            "deny" => Decision::Deny,
            "ask" => Decision::Ask,
            _ => Decision::Allow,
        },
        reason: raw.reason.filter(|r| !r.is_empty()),
        context: raw.context,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::os::unix::fs::PermissionsExt;

    /// A fake `tracewise`: answers `companion --json` and `hook ... --neutral` from the strings given.
    fn fake(dir: &Path, companion: &str, hook: &str, sleep_s: &str) -> PathBuf {
        let p = dir.join("tracewise");
        std::fs::write(&p, format!("#!/bin/sh\ncat >/dev/null\nif [ \"$1\" = companion ]; then echo '{companion}'; else sleep {sleep_s}; echo '{hook}'; fi\n")).unwrap();
        std::fs::set_permissions(&p, std::fs::Permissions::from_mode(0o755)).unwrap();
        p
    }

    fn tmp(name: &str) -> PathBuf {
        let d = std::env::temp_dir().join(format!("am-companion-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        std::fs::create_dir_all(&d).unwrap();
        d
    }

    #[test]
    fn the_probe_accepts_only_the_api_it_knows() {
        let d = tmp("probe");
        assert_eq!(probe(&fake(&d, r#"{"api":1,"version":"0.3.0"}"#, "{}", "0")).map(|c| c.version), Some("0.3.0".to_string()));
        assert!(probe(&fake(&d, r#"{"api":2,"version":"9"}"#, "{}", "0")).is_none(), "a newer API is not used");
        assert!(probe(&fake(&d, "not json", "{}", "0")).is_none());
        assert!(probe(&d.join("missing")).is_none());
    }

    #[test]
    fn an_answer_is_read_and_a_slow_or_foreign_one_is_dropped() {
        let d = tmp("call");
        let c = Companion { path: fake(&d, r#"{"api":1}"#, r#"{"api":1,"event":"pre-edit","decision":"deny","reason":"held by codex","context":["a","b"]}"#, "0"), version: String::new() };
        let a = call(&c, "agy", "pre-tool", "{}", Duration::from_millis(500)).unwrap();
        assert_eq!((a.decision, a.reason.as_deref(), a.context.len()), (Decision::Deny, Some("held by codex"), 2));
        assert!(call(&c, "claude", "boot", "{}", Duration::from_millis(500)).is_none(), "an event Tracewise does not take is not forwarded");
        let slow = Companion { path: fake(&d, r#"{"api":1}"#, r#"{"api":1,"decision":"deny"}"#, "2"), version: String::new() };
        let started = Instant::now();
        assert!(call(&slow, "claude", "pre-tool", "{}", Duration::from_millis(120)).is_none(), "too slow: fail open");
        assert!(started.elapsed() < Duration::from_millis(900), "and the wait was cut short");
        let alien = Companion { path: fake(&d, r#"{"api":1}"#, r#"{"api":7,"decision":"deny"}"#, "0"), version: String::new() };
        assert!(call(&alien, "claude", "pre-tool", "{}", Duration::from_millis(500)).is_none());
    }

    #[test]
    fn agent_and_event_names_are_translated() {
        assert_eq!(tracewise_agent("agy"), "antigravity");
        assert_eq!(tracewise_agent("codex"), "codex");
        assert_eq!(tracewise_event("pre-tool"), Some("PreToolUse"));
        assert_eq!(tracewise_event("boot"), None);
    }

    #[test]
    fn detect_honours_off_and_caches_the_lookup() {
        let d = tmp("detect");
        let bin = fake(&d, r#"{"api":1,"version":"1.0.0"}"#, "{}", "0");
        let db = Database::open_in_memory().unwrap();
        let on = CompanionSettings { path: Some(bin.display().to_string()), ..CompanionSettings::default() };
        assert_eq!(detect(&on, &db).map(|c| c.version), Some("1.0.0".to_string()));
        std::fs::remove_file(&bin).unwrap();
        assert!(detect(&on, &db).is_some(), "the answer is cached for a minute");
        let off = CompanionSettings { tracewise: "off".into(), ..on };
        assert!(detect(&off, &db).is_none());
    }
}

/// Said wherever a file lock is asked for and Tracewise is not there: AutoMemory has no locks of its own.
pub const INSTALL_HINT: &str = "File locks come from Tracewise, which is not installed or not answering (https://github.com/cmdworks/tracewise). Memory works without it.";

/// Runs `tracewise -C <dir> <args…>` and parses its JSON output; `Err` carries a sentence for the person or agent. Used by the lock proxies.
pub fn json_in(settings: &CompanionSettings, db: &Database, dir: &Path, args: &[&str]) -> Result<serde_json::Value, String> {
    let c = detect(settings, db).ok_or_else(|| INSTALL_HINT.to_string())?;
    let d = dir.to_string_lossy();
    let mut full = vec!["-C", d.as_ref()];
    full.extend_from_slice(args);
    let out = run(&c.path, &full, "", Duration::from_millis(2000)).ok_or_else(|| format!("tracewise {} did not answer", args.join(" ")))?;
    serde_json::from_str(out.trim()).map_err(|e| format!("tracewise {} gave unreadable output: {e}", args.join(" ")))
}

/// One file held by an agent, as Tracewise reports it.
#[derive(Debug, Clone, serde::Serialize)]
pub struct HeldLock {
    pub scope: String,
    pub scope_type: &'static str,
    pub agent: String,
    pub session_id: String,
    pub goal: String,
    pub remaining_seconds: i64,
}

/// The folders to ask Tracewise about, each with its project's slug: one project's registered paths, or (no project given) every registered project's
/// paths, so "All projects" means all of them and not whichever folder the dashboard was started in. Falls back to the current folder when none is known.
pub fn project_dirs_named(db: &Database, project_id: Option<&str>) -> Vec<(Option<String>, PathBuf)> {
    let mut out: Vec<(Option<String>, PathBuf)> = Vec::new();
    let mut add = |slug: Option<String>, path: String| {
        let p = PathBuf::from(path);
        if p.is_dir() && !out.iter().any(|(_, q)| *q == p) {
            out.push((slug, p));
        }
    };
    match project_id {
        Some(id) => {
            let slug = db.get_project_by_id(id).ok().flatten().map(|p| p.slug);
            for x in db.get_project_paths(id).unwrap_or_default() {
                add(slug.clone(), x.local_path);
            }
        }
        None => {
            for proj in db.list_projects().unwrap_or_default().into_iter().filter(|p| p.id != "demo").take(10) {
                for x in db.get_project_paths(&proj.id).unwrap_or_default() {
                    add(Some(proj.slug.clone()), x.local_path);
                }
            }
        }
    }
    if out.is_empty() {
        out.extend(std::env::current_dir().ok().map(|d| (None, d)));
    }
    out
}

/// The project's folders (see [`project_dirs_named`]).
pub fn project_dirs(db: &Database, project_id: Option<&str>) -> Vec<PathBuf> {
    project_dirs_named(db, project_id).into_iter().map(|(_, d)| d).collect()
}

/// Files agents hold right now in a project, from `tracewise lock list --json` (one call per project folder). `Err` is the sentence to show
/// when there is no Tracewise: AutoMemory has no locks of its own.
pub fn held_locks(settings: &CompanionSettings, db: &Database, project_id: Option<&str>) -> Result<Vec<HeldLock>, String> {
    let mut out = Vec::new();
    for dir in project_dirs(db, project_id) {
        let v = json_in(settings, db, &dir, &["lock", "list", "--json"])?;
        for r in v.as_array().into_iter().flatten() {
            let s = |k: &str| r.get(k).and_then(|x| x.as_str()).unwrap_or("").to_string();
            out.push(HeldLock {
                scope: s("path"),
                scope_type: "file",
                agent: s("agent"),
                session_id: s("session"),
                goal: r.get("intent").and_then(|x| x.as_str()).map(String::from).unwrap_or_default(),
                remaining_seconds: r.get("expires_in_s").and_then(|x| x.as_i64()).unwrap_or(0),
            });
        }
    }
    Ok(out)
}

/// One recorded file change, as `tracewise log --json` reports it (docs/specs/dashboard-changes.md). `why` and `intent` are written by agents:
/// show them as plain text with the author, never as instructions.
#[derive(Debug, Clone, serde::Serialize)]
pub struct FileChange {
    pub id: String,
    pub time: String,
    pub agent: String,
    /// `agent`, `human`, `script`, `git`, ...
    pub kind: String,
    pub session: String,
    pub path: String,
    pub op: String,
    pub why: Option<String>,
    pub intent: Option<String>,
    pub source: String,
    pub confidence: String,
    /// Set when the change was made while another actor held the file.
    pub lock_conflict: Option<String>,
    /// The project's slug, when the change came from a registered project.
    pub project: Option<String>,
}

/// What to ask for: a session (its id or the first 8 or more characters of it), or up to five files; the newest `limit` changes either way.
#[derive(Debug, Clone, Default)]
pub struct ChangeFilter {
    pub session: Option<String>,
    pub paths: Vec<String>,
    pub limit: usize,
}

fn text(v: &serde_json::Value, k: &str) -> Option<String> {
    v.get(k).and_then(|x| x.as_str()).filter(|s| !s.is_empty()).map(String::from)
}

/// Reads the records of `tracewise log --json` (an array); anything that is not an object with an id and a path is skipped.
pub fn parse_changes(v: &serde_json::Value) -> Vec<FileChange> {
    v.as_array()
        .into_iter()
        .flatten()
        .filter_map(|r| {
            let id = text(r, "id")?;
            let path = text(r, "path")?;
            let actor = r.get("actor")?;
            Some(FileChange {
                id,
                time: r.pointer("/time/end").and_then(|x| x.as_str()).unwrap_or("").to_string(),
                agent: text(actor, "name").unwrap_or_else(|| "unknown".into()),
                kind: text(actor, "kind").unwrap_or_default(),
                session: text(actor, "session").unwrap_or_default(),
                path,
                op: text(r, "op").unwrap_or_else(|| "modify".into()),
                why: text(r, "why"),
                intent: text(r, "intent"),
                source: text(r, "source").unwrap_or_default(),
                confidence: text(r, "confidence").unwrap_or_default(),
                lock_conflict: r.get("lock_conflict").filter(|c| !c.is_null()).map(|c| c.as_str().map(String::from).unwrap_or_else(|| c.to_string())),
                project: None,
            })
        })
        .collect()
}

/// Recent file changes of a project, newest first, from Tracewise. `Err` is the sentence to show when there is no Tracewise.
pub fn file_changes(settings: &CompanionSettings, db: &Database, project_id: Option<&str>, filter: &ChangeFilter) -> Result<Vec<FileChange>, String> {
    let limit = filter.limit.clamp(1, 500);
    let mut out: Vec<FileChange> = Vec::new();
    for (slug, dir) in project_dirs_named(db, project_id) {
        let mut found: Vec<FileChange> = Vec::new();
        if filter.paths.is_empty() {
            // a session lives in a long log: read further back than the page will show
            let n = if filter.session.is_some() { 500 } else { limit };
            let n = n.to_string();
            match json_in(settings, db, &dir, &["log", "--json", "--limit", &n]) {
                Ok(v) => found.extend(parse_changes(&v)),
                // a registered folder that Tracewise does not watch is not an error for the others
                Err(e) if project_id.is_none() && e.contains("did not answer") => {}
                Err(e) => return Err(e),
            }
        } else {
            for p in filter.paths.iter().take(5) {
                match json_in(settings, db, &dir, &["log", p, "--json", "--limit", "3"]) {
                    Ok(v) => found.extend(parse_changes(&v)),
                    Err(e) if project_id.is_none() && e.contains("did not answer") => {}
                    Err(e) => return Err(e),
                }
            }
        }
        for c in &mut found {
            c.project = slug.clone();
        }
        out.extend(found);
    }
    if let Some(s) = filter.session.as_deref().filter(|s| s.len() >= 8) {
        out.retain(|c| c.session == s || c.session.starts_with(s) || s.starts_with(&c.session) && !c.session.is_empty());
    }
    out.sort_by(|a, b| b.time.cmp(&a.time));
    out.dedup_by(|a, b| a.id == b.id);
    out.truncate(limit);
    Ok(out)
}

#[cfg(test)]
mod change_tests {
    use super::*;

    #[test]
    fn records_are_read_and_odd_ones_skipped() {
        let v = serde_json::json!([
            {"id": "c1", "path": "src/a.rs", "op": "modify", "actor": {"kind": "agent", "name": "codex", "session": "s-1111111111"}, "time": {"end": "2026-11-16T08:00:00Z"}, "why": "fix the rounding", "intent": null, "lock_conflict": null, "source": "hook", "confidence": "exact"},
            {"id": "c2", "path": "b.txt", "actor": {"kind": "human", "name": "as"}, "time": {"end": "2026-11-16T09:00:00Z"}, "lock_conflict": {"holder": "codex"}},
            {"path": "no-id.txt"},
            "junk"
        ]);
        let c = parse_changes(&v);
        assert_eq!(c.len(), 2);
        assert_eq!((c[0].agent.as_str(), c[0].why.as_deref(), c[0].lock_conflict.as_deref()), ("codex", Some("fix the rounding"), None));
        assert_eq!(c[1].kind, "human");
        assert!(c[1].lock_conflict.as_deref().is_some_and(|s| s.contains("codex")), "a conflict object is kept as text");
        assert!(parse_changes(&serde_json::json!({"not": "an array"})).is_empty());
    }
}
