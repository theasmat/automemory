//! Automatic file locking beyond the edit tools (docs/architecture/locking.md).
//!
//! * [`shell_write_targets`]: files a shell command is about to write, so they are leased
//!   before it runs.
//! * [`snapshot`] / [`reconcile`]: compare `git status` before and after a command to find
//!   files it changed that no lease covered (scripts, formatters, installers), lease them
//!   and report collisions.
//! * Coverage and alert tables back `automemory locks --coverage` and the dashboard.

use crate::config::LockSettings;
use crate::db::Database;
use crate::guardrails::ShellCommandTree;
use crate::models::{LeaseAcquireArgs, LeaseAcquireResult, LeaseConflict, LeaseScopeType};
use anyhow::Result;
use chrono::{Duration, Utc};
use rusqlite::params;
use std::collections::BTreeMap;
use std::path::Path;
use std::process::Command;

/// Identity of one hook invocation, for [`Database::replay_or_claim_hook`].
pub fn hook_key(agent: &str, event: &str, input: &str) -> String {
    use sha2::{Digest, Sha256};
    format!("{:x}", Sha256::digest(format!("{agent}\0{event}\0{input}").as_bytes()))
}

// ── Shell write prediction ──────────────────────────────────────────────────

/// Files the command writes, moves or deletes, as written in the command (relative or
/// absolute). Redirect targets, `tee`, `sed -i`, `mv`/`cp`/`rm`/`touch`/`truncate`,
/// `git checkout|restore -- <paths>`. Globs and anything the shell computes are left to
/// [`reconcile`].
pub fn shell_write_targets(command: &str) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    for tokens in ShellCommandTree::segment_tokens(command) {
        let words: Vec<&str> = tokens.iter().map(String::as_str).collect();
        redirect_targets(&words, &mut out);
        let words = strip_wrappers(&words);
        let Some((&bin, args)) = words.split_first() else { continue };
        let bin = bin.rsplit('/').next().unwrap_or(bin);
        let positional = |skip: usize| -> Vec<&str> {
            args.iter().filter(|a| !a.starts_with('-')).skip(skip).copied().collect()
        };
        match bin {
            "tee" => out.extend(positional(0).iter().map(|s| s.to_string())),
            "rm" | "unlink" | "rmdir" | "touch" | "truncate" | "shred" => {
                out.extend(positional(0).iter().map(|s| s.to_string()))
            }
            // The source of a move is removed, so both ends change; a copy only writes its last argument.
            "mv" => out.extend(positional(0).iter().map(|s| s.to_string())),
            "cp" | "install" | "ln" => out.extend(positional(0).last().map(|s| s.to_string())),
            "sed" | "perl" if args.iter().any(|a| in_place_flag(a)) => {
                // Files are the positionals left after the script (given by -e/-f, or first).
                let mut files: Vec<&str> = Vec::new();
                let mut script_given = false;
                let mut it = args.iter();
                while let Some(&a) = it.next() {
                    if a == "-e" || a == "-f" {
                        script_given = true;
                        it.next(); // its value
                    } else if !a.starts_with('-') {
                        files.push(a);
                    }
                }
                out.extend(files.iter().skip(usize::from(!script_given)).map(|s| s.to_string()));
            }
            "git" => {
                if let Some(pos) = args.iter().position(|a| *a == "--") {
                    if matches!(args.first().copied(), Some("checkout" | "restore")) {
                        out.extend(args[pos + 1..].iter().map(|s| s.to_string()));
                    }
                }
            }
            _ => {}
        }
    }
    out.retain(|p| is_plain_path(p));
    out.sort();
    out.dedup();
    out
}

fn in_place_flag(a: &str) -> bool {
    a == "-i" || a.starts_with("-i") && !a.starts_with("--") || a.starts_with("-pi") || a == "--in-place"
}

/// `sudo`, `env X=1`, `nohup`, `time` in front of the real command.
fn strip_wrappers<'a, 'b>(words: &'b [&'a str]) -> &'b [&'a str] {
    let mut w = words;
    while let Some((&first, rest)) = w.split_first() {
        match first {
            "sudo" | "nohup" | "time" | "command" => w = rest,
            "env" => {
                w = rest;
                while w.first().is_some_and(|x| x.contains('=') || x.starts_with('-')) {
                    w = &w[1..];
                }
            }
            _ => break,
        }
    }
    w
}

/// `> file`, `>> file`, `>file`, `2> file`; never `/dev/null`-style targets or `&1`.
fn redirect_targets(words: &[&str], out: &mut Vec<String>) {
    let mut i = 0;
    while i < words.len() {
        let w = words[i];
        let (is_redirect, inline) = match w.trim_start_matches(|c: char| c.is_ascii_digit()) {
            ">" | ">>" | ">|" | "&>" => (true, ""),
            s if s.starts_with(">>") => (true, &s[2..]),
            s if s.starts_with('>') && !s.starts_with(">&") => (true, &s[1..]),
            _ => (false, ""),
        };
        if is_redirect {
            let target = if inline.is_empty() { words.get(i + 1).copied().unwrap_or("") } else { inline };
            if !target.is_empty() {
                out.push(target.to_string());
            }
        }
        i += 1;
    }
}

/// A literal path we can lease: not a device, not a glob, not computed by the shell.
fn is_plain_path(p: &str) -> bool {
    !p.is_empty()
        && !p.starts_with("/dev/")
        && !p.starts_with('&')
        && !p.starts_with('-')
        && !matches!(p, "." | ".." | "~" | "/")
        && !p.contains(['*', '?', '[', '$', '`', '(', ')', '{', '<', '>', '|', ';'])
}

// ── Workspace snapshots and reconciliation ──────────────────────────────────

/// Paths git reports as changed, with size and mtime so a second edit of an already-dirty
/// file is noticed.
pub type Snapshot = BTreeMap<String, (u64, u128)>;

/// `git status --porcelain -z` for `root`. `None` outside a git repository.
pub fn snapshot(root: &Path) -> Option<Snapshot> {
    let out = Command::new("git")
        .arg("-C")
        .arg(root)
        .args(["status", "--porcelain", "-z", "--untracked-files=all"])
        .output()
        .ok()
        .filter(|o| o.status.success())?;
    let mut snap = Snapshot::new();
    let mut entries = out.stdout.split(|b| *b == 0).filter(|e| e.len() > 3);
    while let Some(e) = entries.next() {
        let status = &e[..2];
        let path = String::from_utf8_lossy(&e[3..]).to_string();
        if status[0] == b'R' || status[0] == b'C' {
            entries.next(); // the original name follows a rename/copy entry
        }
        let meta = std::fs::metadata(root.join(&path)).ok();
        let size = meta.as_ref().map_or(0, |m| m.len());
        let mtime = meta
            .and_then(|m| m.modified().ok())
            .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
            .map_or(0, |d| d.as_nanos());
        snap.insert(path, (size, mtime));
    }
    Some(snap)
}

/// Files that are new or different in `after` compared to `before`, including files that
/// became clean again (a revert).
pub fn changed_between(before: &Snapshot, after: &Snapshot) -> Vec<String> {
    let mut changed: Vec<String> = after
        .iter()
        .filter(|(p, v)| before.get(*p) != Some(v))
        .map(|(p, _)| p.clone())
        .collect();
    changed.extend(before.keys().filter(|p| !after.contains_key(*p)).cloned());
    changed.sort();
    changed
}

/// Stored snapshots older than this are ignored: the command that took them is long gone.
const STALE_SNAPSHOT_MINUTES: i64 = 15;

impl Database {
    pub fn begin_snapshot(&self, session_id: &str, project_id: Option<&str>, snap: &Snapshot) -> Result<()> {
        self.conn().execute(
            "INSERT INTO workspace_snapshots (session_id, project_id, taken_at, in_flight, data)
             VALUES (?1, ?2, ?3, 1, ?4)
             ON CONFLICT(session_id) DO UPDATE SET project_id = ?2, taken_at = ?3, in_flight = 1, data = ?4",
            params![session_id, project_id, Utc::now().to_rfc3339(), serde_json::to_string(snap)?],
        )?;
        Ok(())
    }

    /// The snapshot taken before this session's command, marking it no longer in flight.
    pub fn finish_snapshot(&self, session_id: &str) -> Result<Option<Snapshot>> {
        let conn = self.conn();
        let row: Option<(String, i64)> = conn
            .query_row(
                "SELECT data, in_flight FROM workspace_snapshots WHERE session_id = ?1",
                params![session_id],
                |r| Ok((r.get(0)?, r.get(1)?)),
            )
            .ok();
        conn.execute("UPDATE workspace_snapshots SET in_flight = 0 WHERE session_id = ?1", params![session_id])?;
        Ok(row.filter(|(_, in_flight)| *in_flight == 1).and_then(|(d, _)| serde_json::from_str(&d).ok()))
    }

    /// Whether another session has a command running right now in this project.
    pub fn other_command_in_flight(&self, session_id: &str, project_id: Option<&str>) -> bool {
        let cutoff = (Utc::now() - Duration::minutes(STALE_SNAPSHOT_MINUTES)).to_rfc3339();
        self.conn()
            .query_row(
                "SELECT COUNT(*) FROM workspace_snapshots
                 WHERE in_flight = 1 AND session_id != ?1 AND taken_at > ?2
                   AND (project_id IS ?3 OR ?3 IS NULL)",
                params![session_id, cutoff, project_id],
                |r| r.get::<_, i64>(0),
            )
            .map(|n| n > 0)
            .unwrap_or(false)
    }

    pub fn drop_snapshot(&self, session_id: &str) {
        let _ = self.conn().execute("DELETE FROM workspace_snapshots WHERE session_id = ?1", params![session_id]);
    }

    /// `source`: `edit` (edit tool, leased first), `shell-predicted` (leased before the command
    /// ran), `reconciled` (found after the fact), `unattributed` (another command was running too).
    pub fn record_write(&self, project_id: Option<&str>, session_id: &str, path: &str, source: &str) {
        let _ = self.conn().execute(
            "INSERT INTO lock_coverage (ts, project_id, session_id, path, source) VALUES (?1, ?2, ?3, ?4, ?5)",
            params![Utc::now().to_rfc3339(), project_id, session_id, path, source],
        );
    }

    /// Writes seen in the last `days`, per source.
    pub fn coverage_since(&self, days: i64) -> Result<Vec<(String, i64)>> {
        let since = (Utc::now() - Duration::days(days)).to_rfc3339();
        let conn = self.conn();
        let mut stmt = conn.prepare(
            "SELECT source, COUNT(*) FROM lock_coverage WHERE ts > ?1 GROUP BY source ORDER BY source",
        )?;
        let rows = stmt.query_map(params![since], |r| Ok((r.get(0)?, r.get(1)?)))?;
        Ok(rows.flatten().collect())
    }

    /// Keep a message for the session's agent until a hook that can show it runs.
    pub fn push_note(&self, session_id: &str, text: &str) {
        let _ = self.conn().execute(
            "INSERT INTO session_notes (session_id, ts, text) VALUES (?1, ?2, ?3)",
            params![session_id, Utc::now().to_rfc3339(), text],
        );
    }

    /// The session's waiting notes, oldest first; each is delivered once.
    pub fn drain_notes(&self, session_id: &str) -> Vec<String> {
        let conn = self.conn();
        let notes: Vec<String> = conn
            .prepare("SELECT text FROM session_notes WHERE session_id = ?1 ORDER BY id")
            .and_then(|mut s| s.query_map(params![session_id], |r| r.get(0)).map(|rows| rows.flatten().collect()))
            .unwrap_or_default();
        let _ = conn.execute("DELETE FROM session_notes WHERE session_id = ?1", params![session_id]);
        notes
    }

    /// Atomically claim a hook invocation. `Some(output)` means an identical one (same agent,
    /// event and payload) ran within the last moments and this is its reply: print it and stop.
    /// `None` means this process owns the invocation and must call [`Database::finish_hook`].
    pub fn replay_or_claim_hook(&self, key: &str) -> Option<String> {
        const WINDOW_MS: i64 = 1500;
        let now = Utc::now().timestamp_millis();
        let claimed = {
            let conn = self.conn();
            if conn.execute_batch("BEGIN IMMEDIATE").is_err() {
                return None;
            }
            let prior: Option<i64> = conn
                .query_row("SELECT ts FROM hook_claims WHERE key = ?1", params![key], |r| r.get(0))
                .ok();
            let fresh = prior.is_none_or(|ts| now - ts > WINDOW_MS);
            if fresh {
                let _ = conn.execute(
                    "INSERT INTO hook_claims (key, ts, output) VALUES (?1, ?2, NULL)
                     ON CONFLICT(key) DO UPDATE SET ts = ?2, output = NULL",
                    params![key, now],
                );
                let _ = conn.execute("DELETE FROM hook_claims WHERE ts < ?1", params![now - 60_000]);
            }
            let _ = conn.execute_batch("COMMIT");
            fresh
        };
        if claimed {
            return None;
        }
        // A twin is (or was just) running: wait briefly for its reply.
        for _ in 0..150 {
            let out: Option<String> = self
                .conn()
                .query_row("SELECT output FROM hook_claims WHERE key = ?1", params![key], |r| r.get(0))
                .ok()
                .flatten();
            if out.is_some() {
                return out;
            }
            std::thread::sleep(std::time::Duration::from_millis(20));
        }
        None
    }

    pub fn finish_hook(&self, key: &str, output: &str) {
        let _ = self.conn().execute("UPDATE hook_claims SET output = ?2 WHERE key = ?1", params![key, output]);
    }

    pub fn record_alert(&self, project_id: Option<&str>, path: &str, agent: &str, session: &str, message: &str) {
        let _ = self.conn().execute(
            "INSERT INTO lock_alerts (ts, project_id, path, holder_agent, holder_session, message) VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
            params![Utc::now().to_rfc3339(), project_id, path, agent, session, message],
        );
    }

    pub fn recent_alerts(&self, limit: usize) -> Vec<(String, String, String)> {
        let conn = self.conn();
        let Ok(mut stmt) = conn.prepare("SELECT ts, path, message FROM lock_alerts ORDER BY id DESC LIMIT ?1") else {
            return Vec::new();
        };
        stmt.query_map(params![limit as i64], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)))
            .map(|rows| rows.flatten().collect())
            .unwrap_or_default()
    }
}

/// `covered` is the share of writes a lease covered when they happened.
pub fn coverage_summary(rows: &[(String, i64)]) -> (i64, i64, i64) {
    let get = |k: &str| rows.iter().find(|(s, _)| s == k).map_or(0, |(_, n)| *n);
    let before = get("edit") + get("shell-predicted");
    let after = get("reconciled");
    let unknown = get("unattributed");
    (before, after, unknown)
}

#[derive(Debug, Default)]
pub struct Reconciled {
    /// Files this command changed that had no lease beforehand (now leased).
    pub newly_leased: Vec<String>,
    /// Files the command changed that another session holds.
    pub conflicts: Vec<LeaseConflict>,
}

/// After a command: lease what it changed, and report files another session holds.
/// When another session's command was running at the same time the change cannot be
/// attributed, so it is only counted.
pub fn reconcile(
    db: &Database,
    settings: &LockSettings,
    project_id: Option<&str>,
    session_id: &str,
    agent: &str,
    before: &Snapshot,
    after: &Snapshot,
) -> Reconciled {
    let mut result = Reconciled::default();
    let ambiguous = db.other_command_in_flight(session_id, project_id);
    let active_leases = db.list_active_leases(project_id).unwrap_or_default();
    for path in changed_between(before, after) {
        if settings.is_ignored(&path) {
            continue;
        }
        if ambiguous {
            db.record_write(project_id, session_id, &path, "unattributed");
            continue;
        }
        let held_by_me = active_leases
            .iter()
            .any(|l| l.session_id == session_id && crate::db::queries::leases::scope_covers(&l.scope, &l.scope_type, &path));
        let args = LeaseAcquireArgs {
            project_id,
            agent,
            session_id,
            scope: &path,
            scope_type: LeaseScopeType::File,
            goal: &format!("changed by a command: {path}"),
            ttl_seconds: settings.idle_ttl_seconds,
        };
        match db.acquire_lease(args) {
            Ok(LeaseAcquireResult::Conflict { conflict }) => {
                db.record_write(project_id, session_id, &path, "reconciled");
                result.conflicts.push(conflict);
            }
            Ok(_) => {
                db.record_write(project_id, session_id, &path, if held_by_me { "shell-predicted" } else { "reconciled" });
                if !held_by_me {
                    result.newly_leased.push(path);
                }
            }
            Err(_) => {}
        }
    }
    result
}

#[cfg(test)]
mod tests {
    use super::*;

    fn targets(cmd: &str) -> Vec<String> {
        shell_write_targets(cmd)
    }

    #[test]
    fn predicts_the_files_a_shell_command_writes() {
        assert_eq!(targets("echo hi > src/a.txt"), ["src/a.txt"]);
        assert_eq!(targets("echo hi >> log.txt"), ["log.txt"]);
        assert_eq!(targets("cat x | tee -a out.txt other.txt"), ["other.txt", "out.txt"]);
        assert_eq!(targets("sed -i 's/a/b/' src/a.rs src/b.rs"), ["src/a.rs", "src/b.rs"]);
        assert_eq!(targets("sed -i.bak -e 's/a/b/' src/a.rs"), ["src/a.rs"]);
        assert_eq!(targets("perl -pi -e 's/a/b/' src/a.rs"), ["src/a.rs"]);
        assert_eq!(targets("mv old.rs new.rs"), ["new.rs", "old.rs"]);
        assert_eq!(targets("cp a.rs b.rs"), ["b.rs"]); // the source is only read
        assert_eq!(targets("rm -f stale.txt"), ["stale.txt"]);
        assert_eq!(targets("git checkout -- src/a.rs"), ["src/a.rs"]);
        assert_eq!(targets("git restore --staged -- src/a.rs"), ["src/a.rs"]);
        assert_eq!(targets("cd src && sudo sed -i 's/a/b/' lib.rs"), ["lib.rs"]);
    }

    #[test]
    fn reads_devices_globs_and_computed_names_are_not_predicted() {
        for cmd in [
            "ls -la", "cat src/a.rs", "grep -r x src", "cargo build", "echo hi > /dev/null",
            "cmd 2>&1", "rm -rf build/*", "echo x > $OUT", "sed 's/a/b/' src/a.rs", "git status",
            "git checkout main",
        ] {
            assert!(targets(cmd).is_empty(), "{cmd}: {:?}", targets(cmd));
        }
    }

    #[test]
    fn snapshots_diff_into_changed_files() {
        let mut before = Snapshot::new();
        before.insert("a.rs".into(), (10, 1));
        before.insert("gone.rs".into(), (5, 1));
        let mut after = Snapshot::new();
        after.insert("a.rs".into(), (12, 2)); // edited again while already dirty
        after.insert("new.rs".into(), (3, 3));
        assert_eq!(changed_between(&before, &after), ["a.rs", "gone.rs", "new.rs"]);
        assert!(changed_between(&after, &after).is_empty());
    }

    #[test]
    fn reconcile_leases_unpredicted_writes_and_reports_collisions() {
        let db = Database::open_in_memory().unwrap();
        let settings = LockSettings::default();
        let before = Snapshot::new();
        let mut after = Snapshot::new();
        after.insert("src/gen.rs".into(), (1, 1));
        after.insert("target/debug/x".into(), (1, 1)); // ignored by default
        after.insert("held.rs".into(), (1, 1));

        // Another session already holds held.rs.
        db.acquire_lease(LeaseAcquireArgs {
            project_id: Some("p"), agent: "codex", session_id: "other", scope: "held.rs",
            scope_type: LeaseScopeType::File, goal: "refactor", ttl_seconds: 60,
        })
        .unwrap();

        let r = reconcile(&db, &settings, Some("p"), "me", "claude", &before, &after);
        assert_eq!(r.newly_leased, ["src/gen.rs"]);
        assert_eq!(r.conflicts.len(), 1);
        assert_eq!(r.conflicts[0].holder_agent, "codex");
        let cov = db.coverage_since(1).unwrap();
        assert_eq!(cov, [("reconciled".to_string(), 2)]); // ignored file not counted

        // A command from another session running at the same time makes it ambiguous.
        db.begin_snapshot("other", Some("p"), &Snapshot::new()).unwrap();
        let mut again = Snapshot::new();
        again.insert("other_file.rs".into(), (1, 1));
        let r = reconcile(&db, &settings, Some("p"), "me", "claude", &before, &again);
        assert!(r.newly_leased.is_empty() && r.conflicts.is_empty());
        assert!(db.coverage_since(1).unwrap().iter().any(|(s, n)| s == "unattributed" && *n == 1));
    }

    #[test]
    fn a_real_git_repo_snapshot_sees_a_script_write() {
        let dir = std::env::temp_dir().join(format!("am-snap-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let git = |a: &[&str]| {
            Command::new("git").args(["-c", "user.name=t", "-c", "user.email=t@t"]).args(a).current_dir(&dir).output().unwrap()
        };
        git(&["init", "-q"]);
        std::fs::write(dir.join("tracked.rs"), "a").unwrap();
        git(&["add", "-A"]);
        git(&["commit", "-qm", "init"]);

        let before = snapshot(&dir).unwrap();
        assert!(before.is_empty());
        std::fs::write(dir.join("tracked.rs"), "changed by a script").unwrap();
        std::fs::write(dir.join("new.rs"), "x").unwrap();
        let after = snapshot(&dir).unwrap();
        assert_eq!(changed_between(&before, &after), ["new.rs", "tracked.rs"]);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn ignore_patterns() {
        let s = LockSettings::default();
        for p in ["target/debug/a", "app/node_modules/x/y.js", "Cargo.lock", "a.pyc", "sub/.DS_Store"] {
            assert!(s.is_ignored(p), "{p}");
        }
        for p in ["src/targets.rs", "src/lib.rs", "docs/build-notes.md"] {
            assert!(!s.is_ignored(p), "{p}");
        }
    }
}

#[cfg(test)]
mod dedupe_tests {
    use super::*;

    #[test]
    fn an_identical_hook_call_replays_the_first_reply_once() {
        let db = Database::open_in_memory().unwrap();
        let key = hook_key("agy", "pre-tool", r#"{"conversationId":"c","stepIdx":3}"#);
        assert_eq!(db.replay_or_claim_hook(&key), None, "first call owns the invocation");
        db.finish_hook(&key, r#"{"decision":"allow"}"#);
        assert_eq!(db.replay_or_claim_hook(&key).as_deref(), Some(r#"{"decision":"allow"}"#), "twin gets the reply");
        // A different payload, event or agent is independent.
        for other in [
            hook_key("agy", "pre-tool", r#"{"conversationId":"c","stepIdx":4}"#),
            hook_key("agy", "post-tool", r#"{"conversationId":"c","stepIdx":3}"#),
            hook_key("claude", "pre-tool", r#"{"conversationId":"c","stepIdx":3}"#),
        ] {
            assert_eq!(db.replay_or_claim_hook(&other), None);
        }
    }

    #[test]
    fn a_stale_claim_is_taken_over() {
        let db = Database::open_in_memory().unwrap();
        let key = hook_key("a", "e", "{}");
        db.replay_or_claim_hook(&key);
        db.finish_hook(&key, "old");
        db.conn().execute("UPDATE hook_claims SET ts = ts - 5000", []).unwrap();
        assert_eq!(db.replay_or_claim_hook(&key), None, "outside the window the call is new");
    }
}
