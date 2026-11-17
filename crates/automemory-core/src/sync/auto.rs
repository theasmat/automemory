//! Automatic, non-blocking sync through a git remote.
//!
//! One pass = pull the remote, apply other devices' delta files, write ours (encrypted when a
//! key is available), commit, push. Delta files are content-addressed and named per device, so
//! two machines never write the same path and git has nothing to conflict on. Hooks never wait for
//! this: they start `automemory sync auto` in the background (see [`should_spawn`]).

use crate::db::Database;
use crate::sync::deltas;
use anyhow::{Context, Result};
use chrono::Utc;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

const LOCK_FILE: &str = ".sync.lock";
const LOCK_STALE: Duration = Duration::from_secs(120);
const NETWORK_TIMEOUT: Duration = Duration::from_secs(25);
const SPAWN_INTERVAL_MS: i64 = 20_000;
/// The remote the sent-state in the database belongs to.
const REMOTE_IDENTITY_KEY: &str = "sync_remote_identity";

#[derive(Debug, Default, Clone)]
pub struct AutoReport {
    pub applied: usize,
    pub conflicts: usize,
    pub pulled: bool,
    pub pushed: bool,
    pub skipped_locked: bool,
    pub encrypted: bool,
    pub notes: Vec<String>,
    /// The vault's git history was replaced by a purge on another computer; this one followed it.
    pub vault_replaced: bool,
}

impl AutoReport {
    pub fn summary(&self) -> String {
        if self.skipped_locked {
            return "another sync is already running".to_string();
        }
        let mut s = format!(
            "applied {} record(s), {} conflict(s){}{}{}",
            self.applied,
            self.conflicts,
            if self.pulled { ", pulled" } else { "" },
            if self.pushed { ", pushed" } else { "" },
            if self.encrypted { ", encrypted" } else { "" },
        );
        for n in &self.notes {
            s.push_str(&format!("; {n}"));
        }
        s
    }
}

/// Vault folder and git remote from the configuration.
pub fn vault_settings() -> (PathBuf, Option<String>) {
    let root = crate::config::Config::default_config().vault_dir;
    let remote = crate::config::GlobalConfig::load()
        .ok()
        .and_then(|g| g.vault.remote_url)
        .filter(|u| !u.trim().is_empty());
    (root, remote)
}

/// The key delta files are encrypted with. Only when a remote is configured: a purely local vault
/// has nothing to protect from. Creates a key on first use (back it up with `sync pair`).
pub fn resolve_key(remote_configured: bool) -> Option<[u8; 32]> {
    remote_configured
        .then(|| crate::crypto::BlindCrypto::get_or_create_master_key(None).ok())
        .flatten()
}

/// The key this computer already has; never creates one (for status and checks).
pub fn existing_key() -> Option<[u8; 32]> {
    crate::crypto::BlindCrypto::existing_master_key()
}

/// Whether a hook should start a background sync now (a remote exists and none was started recently).
pub fn should_spawn(db: &Database) -> bool {
    if vault_settings().1.is_none() {
        return false;
    }
    let now = Utc::now().timestamp_millis();
    let last: i64 = db
        .sync_kv_get("last_auto_sync_spawn")
        .and_then(|v| v.parse().ok())
        .unwrap_or(0);
    if now - last < SPAWN_INTERVAL_MS {
        return false;
    }
    db.sync_kv_set("last_auto_sync_spawn", &now.to_string());
    true
}

struct LockGuard(PathBuf);
impl Drop for LockGuard {
    fn drop(&mut self) {
        let _ = std::fs::remove_file(&self.0);
    }
}

fn take_lock(vault_root: &Path) -> Option<LockGuard> {
    let path = vault_root.join(LOCK_FILE);
    let stale = std::fs::metadata(&path)
        .and_then(|m| m.modified())
        .ok()
        .and_then(|t| t.elapsed().ok())
        .is_some_and(|age| age > LOCK_STALE);
    if stale {
        let _ = std::fs::remove_file(&path);
    }
    std::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&path)
        .ok()
        .map(|_| LockGuard(path))
}

/// Runs git in the vault with a fixed identity (a fresh machine has none) and a time limit.
pub(super) fn git(dir: &Path, args: &[&str], limit: Duration) -> Result<(bool, String)> {
    let mut child = Command::new("git")
        .args(["-c", "user.name=AutoMemory", "-c", "user.email=automemory@localhost", "-c", "commit.gpgsign=false"])
        .args(args)
        .current_dir(dir)
        .env("GIT_TERMINAL_PROMPT", "0")
        // an ssh remote that wants a passphrase or an unknown-host answer must fail, not wait for a person who is not there
        .env("GIT_SSH_COMMAND", std::env::var("GIT_SSH_COMMAND").unwrap_or_else(|_| "ssh -o BatchMode=yes -o ConnectTimeout=15".to_string()))
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .with_context(|| format!("git {}", args.join(" ")))?;
    let start = Instant::now();
    loop {
        if child.try_wait()?.is_some() {
            break;
        }
        if start.elapsed() > limit {
            let _ = child.kill();
            let _ = child.wait();
            anyhow::bail!("git {} timed out after {}s", args.join(" "), limit.as_secs());
        }
        std::thread::sleep(Duration::from_millis(20));
    }
    let out = child.wait_with_output()?;
    let text = format!(
        "{}{}",
        String::from_utf8_lossy(&out.stdout),
        String::from_utf8_lossy(&out.stderr)
    );
    Ok((out.status.success(), text.trim().to_string()))
}

fn ensure_repo(vault_root: &Path, remote_url: Option<&str>) -> Result<()> {
    std::fs::create_dir_all(vault_root)?;
    if !vault_root.join(".git").exists() {
        let (ok, msg) = git(vault_root, &["init", "-q", "-b", "main"], Duration::from_secs(10))?;
        if !ok {
            anyhow::bail!("git init failed: {msg}");
        }
    }
    if let Some(url) = remote_url {
        crate::sync::GitSync::set_remote(vault_root, "origin", url)?;
    }
    Ok(())
}

/// Keeps the lock file and temp files out of git. Written after a pull: on a first pull an
/// untracked copy would block the checkout of the remote's own.
fn ensure_gitignore(vault_root: &Path) {
    let ignore = vault_root.join(".gitignore");
    if !ignore.exists() {
        let _ = std::fs::write(&ignore, format!("{LOCK_FILE}\n*.tmp*\n"));
    }
}

fn has_commits(vault_root: &Path) -> bool {
    git(vault_root, &["rev-parse", "--verify", "-q", "HEAD"], Duration::from_secs(5))
        .map(|(ok, _)| ok)
        .unwrap_or(false)
}

/// Brings in the remote's commits. Returns a note when it could not (offline, no remote branch yet).
fn pull(vault_root: &Path, report: &mut AutoReport) {
    match git(vault_root, &["fetch", "-q", "origin", "main"], NETWORK_TIMEOUT) {
        Ok((true, _)) => {}
        Ok((false, msg)) => {
            // An empty remote has no main yet: nothing to pull, push will create it.
            if !msg.contains("couldn't find remote ref") {
                report.notes.push(format!("fetch failed: {}", msg.lines().last().unwrap_or("")));
            }
            return;
        }
        Err(e) => {
            report.notes.push(format!("offline: {e}"));
            return;
        }
    }
    let merged = if has_commits(vault_root) {
        git(vault_root, &["pull", "-q", "--rebase", "--autostash", "origin", "main"], NETWORK_TIMEOUT)
    } else {
        let _ = std::fs::remove_file(vault_root.join(".gitignore")); // ours, regenerated after
        git(vault_root, &["checkout", "-q", "-B", "main", "origin/main"], NETWORK_TIMEOUT)
    };
    match merged {
        Ok((true, _)) => report.pulled = true,
        Ok((false, msg)) => {
            let _ = git(vault_root, &["rebase", "--abort"], Duration::from_secs(10));
            // Histories with nothing in common: another computer purged the vault. Everything in it is derived from the
            // databases, so follow the remote and send everything again.
            let unrelated = git(vault_root, &["merge-base", "HEAD", "origin/main"], Duration::from_secs(10)).map(|(ok, _)| !ok).unwrap_or(false);
            if unrelated && git(vault_root, &["reset", "-q", "--hard", "origin/main"], NETWORK_TIMEOUT).map(|(ok, _)| ok).unwrap_or(false) {
                report.pulled = true;
                report.vault_replaced = true;
            } else {
                report.notes.push(format!("pull failed: {}", msg.lines().last().unwrap_or("")));
            }
        }
        Err(e) => report.notes.push(format!("pull failed: {e}")),
    }
}

fn push(vault_root: &Path, report: &mut AutoReport) {
    ensure_gitignore(vault_root);
    let _ = git(vault_root, &["add", "-A"], Duration::from_secs(20));
    let dirty = git(vault_root, &["status", "--porcelain"], Duration::from_secs(10))
        .map(|(_, out)| !out.is_empty())
        .unwrap_or(false);
    if dirty {
        let _ = git(vault_root, &["commit", "-q", "-m", "AutoMemory sync"], Duration::from_secs(20));
    }
    if !has_commits(vault_root) {
        return;
    }
    for attempt in 0..2 {
        match git(vault_root, &["push", "-q", "-u", "origin", "main"], NETWORK_TIMEOUT) {
            Ok((true, _)) => {
                report.pushed = true;
                return;
            }
            Ok((false, msg)) if attempt == 0 && (msg.contains("rejected") || msg.contains("fetch first")) => {
                // Another machine pushed first: take its commits and try once more.
                pull(vault_root, report);
            }
            Ok((false, msg)) => {
                report.notes.push(format!("push failed: {}", msg.lines().last().unwrap_or("")));
                return;
            }
            Err(e) => {
                report.notes.push(format!("push failed: {e}"));
                return;
            }
        }
    }
}

/// Replaces the vault's git history with one commit and force-pushes it, so old (possibly sensitive) delta files are
/// gone from the repository. Compact first (`deltas::compact`). The host may keep old objects until it collects garbage.
pub fn purge_history(vault_root: &Path, remote_url: Option<&str>) -> Result<()> {
    let steps: [&[&str]; 4] = [
        &["checkout", "-q", "--orphan", "am-purge"],
        &["add", "-A"],
        &["commit", "-q", "-m", "AutoMemory vault (history purged)"],
        &["branch", "-q", "-D", "main"],
    ];
    for s in steps {
        let (ok, msg) = git(vault_root, s, Duration::from_secs(30))?;
        if !ok && s[0] != "branch" {
            anyhow::bail!("git {} failed: {}", s[0], msg.trim());
        }
    }
    let (ok, msg) = git(vault_root, &["branch", "-q", "-m", "main"], Duration::from_secs(10))?;
    if !ok {
        anyhow::bail!("git branch failed: {}", msg.trim());
    }
    if remote_url.is_some() {
        let (ok, msg) = git(vault_root, &["push", "-q", "--force", "-u", "origin", "main"], NETWORK_TIMEOUT)?;
        if !ok {
            anyhow::bail!("force push failed: {}", msg.trim());
        }
    }
    Ok(())
}

/// One sync pass. Local-only (no remote) still applies and writes delta files in the vault folder.
pub fn run(
    db: &Database,
    vault_root: &Path,
    remote_url: Option<&str>,
    do_pull: bool,
    do_push: bool,
    key: Option<&[u8; 32]>,
) -> Result<AutoReport> {
    let mut report = AutoReport { encrypted: key.is_some(), ..Default::default() };
    std::fs::create_dir_all(vault_root)?;
    let Some(_guard) = take_lock(vault_root) else {
        report.skipped_locked = true;
        return Ok(report);
    };
    if let Some(url) = remote_url {
        // What this computer believes it already sent belongs to ONE remote. A different remote (a recreated repository, another provider) starts empty,
        // so everything must be sent again; the first time a remote is seen it is only remembered.
        match db.sync_kv_get(REMOTE_IDENTITY_KEY) {
            Some(prev) if prev != url => {
                deltas::forget_sent(db);
                db.sync_kv_set(REMOTE_IDENTITY_KEY, url);
            }
            None => db.sync_kv_set(REMOTE_IDENTITY_KEY, url),
            _ => {}
        }
    }
    if remote_url.is_some() {
        ensure_repo(vault_root, remote_url)?;
        if do_pull {
            pull(vault_root, &mut report);
        }
    }
    if report.vault_replaced {
        deltas::forget_sent(db);
    }
    let applied = deltas::sync_cycle_keyed(db, vault_root, key)?;
    // A device was revoked: this computer now holds the rotated vault key; make it the one the keychain returns.
    if let Some(new_key) = super::epochs::take_pending_key(db) {
        if crate::keychain::set_master_key(&new_key).is_ok() {
            super::epochs::clear_pending_key(db);
        }
    }
    report.applied = applied.applied;
    report.conflicts = applied.conflicts;
    if remote_url.is_some() && do_push {
        push(vault_root, &mut report);
    }
    if remote_url.is_some() {
        super::backup_git::record_run(db, &report);
    }
    Ok(report)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::*;

    fn sh(dir: &Path, args: &[&str]) {
        let ok = Command::new("git").args(args).current_dir(dir).output().unwrap().status.success();
        assert!(ok, "git {args:?}");
    }

    fn project(db: &Database, id: &str) {
        let now = Utc::now();
        db.save_project(&Project {
            id: id.into(), slug: "shop".into(), display_name: "shop".into(), created_at: now, updated_at: now,
            capture_policy: Default::default(), context_budget: Default::default(),
            document_config: Default::default(), sync_config: Default::default(),
        }).unwrap();
        db.set_project_key(id, "git:github.com/me/shop").unwrap();
    }

    fn decision_memory(project: &str, id: &str, text: &str) -> Memory {
        Memory {
            id: id.into(), scope: MemoryScope::Project, project_id: Some(project.into()),
            memory_type: MemoryType::Decision, content: text.into(), summary: text.into(),
            confidence: 0.9, importance: 0.5, freshness: 1.0, status: MemoryStatus::Active,
            source_kind: MemorySourceKind::Agent, version: Some(1), created_at: Utc::now(),
            updated_at: Utc::now(), last_recalled_at: None, tags: vec![], entities: vec![],
            metadata: serde_json::json!({}),
        }
    }

    #[test]
    fn two_machines_converge_through_a_git_remote() {
        let root = std::env::temp_dir().join(format!("am-auto-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        let remote = root.join("remote.git");
        std::fs::create_dir_all(&remote).unwrap();
        sh(&remote, &["init", "-q", "--bare", "-b", "main"]);
        let url = remote.to_string_lossy().to_string();
        let key = [3u8; 32];

        let (a, b) = (Database::open_in_memory().unwrap(), Database::open_in_memory().unwrap());
        project(&a, "id-a");
        a.save_memory_direct(&decision_memory("id-a", "m-1", "checkout uses idempotency keys")).unwrap();
        let (va, vb) = (root.join("vault-a"), root.join("vault-b"));

        // Machine A: nothing to pull yet (empty remote), writes and pushes.
        let ra = run(&a, &va, Some(&url), true, true, Some(&key)).unwrap();
        assert!(ra.pushed, "{}", ra.summary());

        // Machine B: a different folder, pulls, applies, and creates the project from the key.
        let rb = run(&b, &vb, Some(&url), true, true, Some(&key)).unwrap();
        assert!(rb.pulled && rb.applied == 1, "{}", rb.summary());
        let pid = b.project_id_for_key("git:github.com/me/shop").expect("project created on B");
        assert_eq!(b.get_memory_by_id("m-1").unwrap().unwrap().project_id.as_deref(), Some(pid.as_str()));

        // B edits and pushes; A picks it up on its next pass.
        let mut m = b.get_memory_by_id("m-1").unwrap().unwrap();
        m.summary = "checkout uses idempotency keys and retries".into();
        m.updated_at = Utc::now();
        b.save_memory_direct(&m).unwrap();
        assert!(run(&b, &vb, Some(&url), true, true, Some(&key)).unwrap().pushed);
        let ra = run(&a, &va, Some(&url), true, true, Some(&key)).unwrap();
        assert_eq!((ra.applied, ra.conflicts), (1, 0), "{}", ra.summary());
        assert_eq!(a.get_memory_by_id("m-1").unwrap().unwrap().summary, "checkout uses idempotency keys and retries");

        // The remote holds ciphertext only.
        let (_, listing) = git(&vb, &["ls-files"], Duration::from_secs(5)).unwrap();
        assert!(listing.contains(".enc") && !listing.contains(".jsonl"), "{listing}");
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn a_new_remote_gets_everything_again_not_just_what_is_new() {
        let root = std::env::temp_dir().join(format!("am-auto-newremote-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        let mut remotes = Vec::new();
        for n in ["one.git", "two.git"] {
            let r = root.join(n);
            std::fs::create_dir_all(&r).unwrap();
            sh(&r, &["init", "-q", "--bare", "-b", "main"]);
            remotes.push(r.to_string_lossy().to_string());
        }
        let key = [5u8; 32];
        let a = Database::open_in_memory().unwrap();
        project(&a, "id-a");
        a.save_memory_direct(&decision_memory("id-a", "m-1", "first remote note")).unwrap();
        let va = root.join("vault-a");
        assert!(run(&a, &va, Some(&remotes[0]), true, true, Some(&key)).unwrap().pushed);

        // The same computer is pointed at a second, empty repository (a provider change, or the repository was recreated).
        let vb = root.join("vault-a2");
        assert!(run(&a, &vb, Some(&remotes[1]), true, true, Some(&key)).unwrap().pushed);
        let fresh = Database::open_in_memory().unwrap();
        let vc = root.join("vault-c");
        let r = run(&fresh, &vc, Some(&remotes[1]), true, true, Some(&key)).unwrap();
        assert!(r.pulled && r.applied >= 1, "the second remote must hold the existing note, not just new ones: {}", r.summary());
        assert!(fresh.get_memory_by_id("m-1").unwrap().is_some());
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn a_second_sync_is_refused_while_one_runs_and_offline_is_not_an_error() {
        let root = std::env::temp_dir().join(format!("am-auto-lock-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        let vault = root.join("vault");
        std::fs::create_dir_all(&vault).unwrap();
        let db = Database::open_in_memory().unwrap();

        std::fs::write(vault.join(LOCK_FILE), "").unwrap();
        let r = run(&db, &vault, None, true, true, None).unwrap();
        assert!(r.skipped_locked);
        std::fs::remove_file(vault.join(LOCK_FILE)).unwrap();

        // Unreachable remote: a note, never an error, and local work still lands in the vault.
        let r = run(&db, &vault, Some("/nonexistent/remote.git"), true, true, None).unwrap();
        assert!(!r.notes.is_empty() && !r.pushed, "{}", r.summary());
        assert!(!vault.join(LOCK_FILE).exists(), "lock released");
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn a_purge_leaves_one_commit_and_one_file_and_the_other_computer_follows() {
        let root = std::env::temp_dir().join(format!("am-auto-purge-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        let remote = root.join("remote.git");
        std::fs::create_dir_all(&remote).unwrap();
        sh(&remote, &["init", "-q", "--bare", "-b", "main"]);
        let url = remote.to_string_lossy().to_string();
        let key = [5u8; 32];
        let (a, b) = (Database::open_in_memory().unwrap(), Database::open_in_memory().unwrap());
        project(&a, "id-a");
        let (va, vb) = (root.join("vault-a"), root.join("vault-b"));

        a.save_memory_direct(&decision_memory("id-a", "m-1", "first")).unwrap();
        run(&a, &va, Some(&url), true, true, Some(&key)).unwrap();
        let mut m = a.get_memory_by_id("m-1").unwrap().unwrap();
        m.summary = "second".into();
        m.updated_at = Utc::now();
        a.save_memory_direct(&m).unwrap();
        run(&a, &va, Some(&url), true, true, Some(&key)).unwrap();
        run(&b, &vb, Some(&url), true, true, Some(&key)).unwrap();
        assert_eq!(b.get_memory_by_id("m-1").unwrap().unwrap().summary, "second");

        // A compacts and purges (after a pass that pulls everything, as `sync purge-history` does).
        run(&a, &va, Some(&url), true, true, Some(&key)).unwrap();
        let removed = deltas::compact(&a, &va, Some(&key)).unwrap();
        assert!(removed >= 2, "old delta files are gone ({removed})");
        purge_history(&va, Some(&url)).unwrap();
        let probe = root.join("probe");
        sh(&root, &["clone", "-q", &url, "probe"]);
        let (_, commits) = git(&probe, &["rev-list", "--count", "HEAD"], Duration::from_secs(5)).unwrap();
        assert_eq!(commits.trim(), "1");
        let files = std::fs::read_dir(probe.join("deltas")).unwrap().count();
        assert_eq!(files, 1, "one snapshot file");

        // B edits, then syncs: git rebases onto the rewritten history without help, and A receives the edit.
        let mut m = b.get_memory_by_id("m-1").unwrap().unwrap();
        m.summary = "third".into();
        m.updated_at = Utc::now() + chrono::Duration::seconds(3);
        b.save_memory_direct(&m).unwrap();
        let rb = run(&b, &vb, Some(&url), true, true, Some(&key)).unwrap();
        assert!(rb.pulled && rb.pushed, "{}", rb.summary());
        run(&a, &va, Some(&url), true, true, Some(&key)).unwrap();
        assert_eq!(a.get_memory_by_id("m-1").unwrap().unwrap().summary, "third");
        let _ = std::fs::remove_dir_all(&root);
    }
}
