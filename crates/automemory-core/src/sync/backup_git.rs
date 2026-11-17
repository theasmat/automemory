//! The git backup: any git remote (GitHub, GitLab, Bitbucket, Gitea, a server of your own, a folder) holds the encrypted vault, and AutoMemory keeps it
//! current by itself (docs/specs/git-backup.md). This module knows how to read a remote's address safely, probe it, remember how the last syncs went,
//! and prove the copy on the remote restores.

use super::auto::{git, AutoReport};
use crate::db::Database;
use anyhow::{bail, Context, Result};
use chrono::{DateTime, Duration as ChronoDuration, Utc};
use std::path::Path;
use std::time::Duration;

const KEY_OK: &str = "backup_last_ok";
const KEY_ERR: &str = "backup_last_error";
const KEY_VERIFIED: &str = "backup_last_verified";
/// After this long without a successful sync the backup counts as stale.
const STALE_AFTER_DAYS: i64 = 7;

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Provider {
    GitHub,
    GitLab,
    Bitbucket,
    /// Gitea, Forgejo, a company server, anything else that speaks git.
    Other,
    /// A folder or file URL on this machine or a network drive.
    Local,
}

impl Provider {
    pub fn name(self) -> &'static str {
        match self {
            Provider::GitHub => "GitHub",
            Provider::GitLab => "GitLab",
            Provider::Bitbucket => "Bitbucket",
            Provider::Other => "your git server",
            Provider::Local => "a local folder",
        }
    }
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct RemoteInfo {
    /// As given, trimmed.
    pub url: String,
    pub provider: Provider,
    pub host: String,
    pub ssh: bool,
}

/// Reads a remote's address. Accepts https, http, ssh, `git@host:owner/repo` and a folder (or file URL); refuses anything else, in particular git's
/// command-running transports (`ext::`, `fd::`), whitespace and control characters, because the address ends up in a git command and in the config.
pub fn parse_remote(raw: &str) -> Result<RemoteInfo, String> {
    let url = raw.trim().to_string();
    if url.is_empty() {
        return Err("give the address of an empty private git repository".into());
    }
    if url.chars().any(|c| c.is_whitespace() || c.is_control()) || url.contains(';') || url.contains('|') || url.contains('`') || url.contains('$') {
        return Err("the address has characters that do not belong in a git address".into());
    }
    let lower = url.to_lowercase();
    if lower.starts_with("ext::") || lower.starts_with("fd::") || lower.contains("::") && !lower.starts_with("http") {
        return Err("that kind of address runs commands and is not allowed; use https, ssh or a folder".into());
    }
    let (host, ssh, local) = if let Some(rest) = lower.strip_prefix("https://").or_else(|| lower.strip_prefix("http://")) {
        let authority = rest.split('/').next().unwrap_or("");
        (authority.rsplit('@').next().unwrap_or("").split(':').next().unwrap_or("").to_string(), false, false)
    } else if let Some(rest) = lower.strip_prefix("ssh://") {
        let authority = rest.split('/').next().unwrap_or("");
        (authority.rsplit('@').next().unwrap_or("").split(':').next().unwrap_or("").to_string(), true, false)
    } else if lower.starts_with("file://") || url.starts_with('/') || url.starts_with("./") || url.starts_with("../") || url.starts_with("~/") {
        (String::new(), false, true)
    } else if let Some((user_host, path)) = url.split_once(':') {
        // scp-like: git@github.com:owner/repo.git
        if path.is_empty() || user_host.contains('/') {
            return Err("that does not look like a git address (https://host/owner/repo.git or git@host:owner/repo.git)".into());
        }
        (user_host.rsplit('@').next().unwrap_or("").to_lowercase(), true, false)
    } else {
        return Err("that does not look like a git address (https://host/owner/repo.git or git@host:owner/repo.git)".into());
    };
    if !local && host.is_empty() {
        return Err("the address has no host name".into());
    }
    if host.starts_with('-') {
        return Err("the host name looks like an option, not a host".into());
    }
    let provider = if local { Provider::Local } else { provider_for_host(&host) };
    Ok(RemoteInfo { url, provider, host, ssh })
}

fn provider_for_host(host: &str) -> Provider {
    if host == "github.com" || host.ends_with(".github.com") {
        Provider::GitHub
    } else if host == "gitlab.com" || host.starts_with("gitlab.") {
        Provider::GitLab
    } else if host == "bitbucket.org" || host.starts_with("bitbucket.") {
        Provider::Bitbucket
    } else {
        Provider::Other
    }
}

/// An ssh remote often uses a name from `~/.ssh/config` (`git@github-work:me/vault.git`). Asks ssh what that name really is (`ssh -G`, which only
/// prints the configuration, it connects to nothing) so the provider is recognised.
pub fn resolve_alias(info: &mut RemoteInfo) {
    resolve_alias_with(info, &[]);
}

fn resolve_alias_with(info: &mut RemoteInfo, extra: &[&str]) {
    if !info.ssh || info.provider != Provider::Other || info.host.is_empty() {
        return;
    }
    let Ok(mut child) = std::process::Command::new("ssh").args(extra).args(["-G", "--", &info.host]).stdin(std::process::Stdio::null()).stdout(std::process::Stdio::piped()).stderr(std::process::Stdio::null()).spawn() else { return };
    let start = std::time::Instant::now();
    loop {
        match child.try_wait() {
            Ok(Some(_)) => break,
            Ok(None) if start.elapsed() < Duration::from_secs(3) => std::thread::sleep(Duration::from_millis(10)),
            _ => {
                let _ = child.kill();
                let _ = child.wait();
                return;
            }
        }
    }
    let mut out = String::new();
    if let Some(mut so) = child.stdout.take() {
        use std::io::Read;
        let _ = so.read_to_string(&mut out);
    }
    if let Some(real) = out.lines().find_map(|l| l.strip_prefix("hostname ")).map(|h| h.trim().to_lowercase()).filter(|h| !h.is_empty() && *h != info.host) {
        info.provider = provider_for_host(&real);
        info.host = real;
    }
}

/// The address with any password or token inside it replaced, for showing on screen.
pub fn redact_url(url: &str) -> String {
    if let Some((scheme, rest)) = url.split_once("://") {
        if let Some((auth, tail)) = rest.split_once('@') {
            if !auth.contains('/') && auth.contains(':') {
                let user = auth.split(':').next().unwrap_or("");
                return format!("{scheme}://{user}:***@{tail}");
            }
        }
    }
    url.to_string()
}

/// True when the address carries a password or token (it would sit in plain text in the config and in the vault's git config).
pub fn has_embedded_secret(url: &str) -> bool {
    redact_url(url) != url
}

/// How to make the empty private repository, for the provider.
pub fn create_hint(p: Provider) -> String {
    match p {
        Provider::GitHub => "On GitHub: New repository, name it (for example automemory-vault), choose Private, leave it empty. Or: `gh repo create automemory-vault --private`.".into(),
        Provider::GitLab => "On GitLab: New project, Create blank project, set Visibility to Private, and untick \"Initialize with a README\". Or: `glab repo create automemory-vault --private`.".into(),
        Provider::Bitbucket => "On Bitbucket: Create, Repository, set it to private, and leave it empty (no README, no .gitignore). Bitbucket has no official command line for this.".into(),
        Provider::Other => "On your git server: create an empty private repository, then use its https or ssh address.".into(),
        Provider::Local => "A folder works as a remote: `git init --bare <folder>`. It protects against a broken disk only if the folder is on another drive or a share.".into(),
    }
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct Probe {
    pub reachable: bool,
    /// Reachable and holds no branches yet (what a new backup repository should be).
    pub empty: bool,
    /// Anyone can read it without signing in (https remotes only; `None` when it cannot be told).
    pub public: Option<bool>,
    pub error: Option<String>,
}

fn scratch_dir() -> std::path::PathBuf {
    let d = std::env::temp_dir().join(format!("am-git-probe-{}-{}", std::process::id(), Utc::now().timestamp_nanos_opt().unwrap_or(0)));
    let _ = std::fs::create_dir_all(&d);
    d
}

/// Asks the remote what it holds, with the person's own git credentials, and (https) again with none, to find out whether it is public.
pub fn probe(url: &str) -> Probe {
    let dir = scratch_dir();
    let limit = Duration::from_secs(25);
    let own = git(&dir, &["ls-remote", "--heads", url], limit);
    let out = match own {
        Ok((true, text)) => Probe { reachable: true, empty: text.trim().is_empty(), public: None, error: None },
        Ok((false, text)) => Probe { reachable: false, empty: false, public: None, error: Some(friendly_git_error(&text)) },
        Err(e) => Probe { reachable: false, empty: false, public: None, error: Some(friendly_git_error(&e.to_string())) },
    };
    let mut out = out;
    if out.reachable && url.to_lowercase().starts_with("http") {
        // an empty credential.helper list and no prompt: if this still works, the repository is readable by anyone
        let anon = git(&dir, &["-c", "credential.helper=", "-c", "core.askPass=", "ls-remote", "--heads", url], limit);
        out.public = Some(matches!(anon, Ok((true, _))));
    }
    let _ = std::fs::remove_dir_all(&dir);
    out
}

/// A sentence for what git said, with the usual fixes.
pub fn friendly_git_error(raw: &str) -> String {
    let l = raw.to_lowercase();
    let line = raw.lines().rev().find(|l| !l.trim().is_empty()).unwrap_or("").trim().to_string();
    if l.contains("permission denied (publickey)") || l.contains("host key verification failed") {
        "ssh was refused: add your ssh key to the account, or connect once with `ssh -T git@<host>` to accept the host, or use the https address".into()
    } else if l.contains("could not read username") || l.contains("authentication failed") || l.contains("401") || l.contains("403") || l.contains("terminal prompts disabled") {
        "the host wants a sign-in: sign in with your git credential helper (for GitHub `gh auth login`, for GitLab `glab auth login`, or save a personal access token with `git credential approve`), or use an ssh address".into()
    } else if l.contains("not found") || l.contains("does not appear to be a git repository") || l.contains("404") {
        "that repository was not found: check the address, and that you created it and may write to it".into()
    } else if l.contains("could not resolve host") || l.contains("timed out") || l.contains("network is unreachable") {
        "the host could not be reached: check the network and the address".into()
    } else if line.is_empty() {
        "git could not reach that address".into()
    } else {
        format!("git said: {line}")
    }
}

/// Called after every sync pass that had a remote: remembers when it worked and when it did not.
pub fn record_run(db: &Database, report: &AutoReport) {
    if report.skipped_locked {
        return;
    }
    let now = Utc::now().to_rfc3339();
    if report.notes.is_empty() && (report.pulled || report.pushed) {
        db.sync_kv_set(KEY_OK, &now);
        db.sync_kv_delete(KEY_ERR);
    } else if !report.notes.is_empty() {
        db.sync_kv_set(KEY_ERR, &format!("{now}|{}", report.notes.join("; ")));
    }
}

pub fn record_verified(db: &Database) {
    db.sync_kv_set(KEY_VERIFIED, &Utc::now().to_rfc3339());
}

/// Data on this computer that the backup does not carry, and why. What cannot be matched to a project on another computer is not sent.
#[derive(Debug, Clone, serde::Serialize)]
pub struct Gap {
    /// The project's name, or the id it is filed under.
    pub project: String,
    /// `no_key` (registered project whose folder is not here), `unknown_project` (filed under an id that is no project), `unregistered_folder`.
    pub reason: &'static str,
    pub notes: i64,
    pub decisions: i64,
    /// Hand-offs, traces, failures and skills.
    pub other: i64,
    pub fix: String,
}

/// Looks for knowledge that would be lost with this computer: rows filed under a project that has no sync key, or under an id that is no registered project.
pub fn coverage_gaps(db: &Database) -> Vec<Gap> {
    let ids: Vec<String> = {
        let conn = db.conn();
        let mut st = match conn.prepare(
            "SELECT project_id FROM memories WHERE project_id IS NOT NULL UNION SELECT project_id FROM decisions WHERE project_id IS NOT NULL
             UNION SELECT project_id FROM batons WHERE project_id IS NOT NULL UNION SELECT project_id FROM success_traces WHERE project_id IS NOT NULL
             UNION SELECT project_id FROM failure_records WHERE project_id IS NOT NULL UNION SELECT project_id FROM procedural_skills WHERE project_id IS NOT NULL",
        ) {
            Ok(s) => s,
            Err(_) => return Vec::new(),
        };
        st.query_map([], |r| r.get::<_, String>(0)).map(|rows| rows.flatten().collect()).unwrap_or_default()
    };
    let count = |sql: &str, pid: &str| -> i64 { db.conn().query_row(sql, [pid], |r| r.get(0)).unwrap_or(0) };
    let mut gaps = Vec::new();
    for pid in ids {
        if pid == "demo" {
            continue;
        }
        let project = db.get_project_by_id(&pid).ok().flatten();
        let (name, reason, fix) = match &project {
            Some(p) if db.project_key_of(&pid).is_some() => {
                let _ = p;
                continue;
            }
            Some(p) => {
                let paths = db.get_project_paths(&pid).unwrap_or_default();
                // a folder that is here gets its key during the next backup pass: not a gap, just not done yet
                if paths.is_empty() || paths.iter().any(|x| Path::new(&x.local_path).is_dir()) {
                    continue;
                }
                let folder = paths.first().map(|x| x.local_path.clone()).unwrap_or_else(|| "(none registered)".into());
                (p.slug.clone(), "no_key", format!("its folder {folder} is not on this computer, so it cannot be matched to a repository. If the folder moved, register it again (`automemory project register --path <new folder>`); if the project is finished, remove it (`automemory project remove {}`)", p.slug))
            }
            None if pid.starts_with("folder-") => (pid.clone(), "unregistered_folder", format!("written in a folder that is not a registered project; register the folder (`automemory project register <path>`), then `automemory project adopt {pid} <project>`")),
            None => (pid.clone(), "unknown_project", format!("filed under '{pid}', which is not a registered project; move it with `automemory project adopt {pid} <project>`")),
        };
        let notes = count("SELECT COUNT(*) FROM memories WHERE project_id = ?1 AND COALESCE(json_extract(metadata, '$.demo'), 0) != 1", &pid);
        let decisions = count("SELECT COUNT(*) FROM decisions WHERE project_id = ?1", &pid);
        let other = count("SELECT COUNT(*) FROM batons WHERE project_id = ?1", &pid)
            + count("SELECT COUNT(*) FROM success_traces WHERE project_id = ?1", &pid)
            + count("SELECT COUNT(*) FROM failure_records WHERE project_id = ?1", &pid)
            + count("SELECT COUNT(*) FROM procedural_skills WHERE project_id = ?1", &pid);
        if notes + decisions + other > 0 {
            gaps.push(Gap { project: name, reason, notes, decisions, other, fix });
        }
    }
    gaps.sort_by_key(|g| std::cmp::Reverse(g.notes + g.decisions + g.other));
    gaps
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct BackupStatus {
    /// `off` (no remote), `never` (set up, no successful sync yet), `ok`, `partial` (working, but some data cannot be backed up: see `gaps`), `stale` (no success for a week), `failing` (the last attempt failed).
    pub state: &'static str,
    pub remote: Option<String>,
    pub provider: Option<Provider>,
    pub provider_name: Option<&'static str>,
    pub last_ok: Option<String>,
    pub last_error: Option<String>,
    pub last_error_at: Option<String>,
    pub last_verified: Option<String>,
    pub key_present: bool,
    /// Knowledge on this computer that the backup does not carry.
    pub gaps: Vec<Gap>,
}

fn parse_ts(s: &str) -> Option<DateTime<Utc>> {
    DateTime::parse_from_rfc3339(s).ok().map(|d| d.with_timezone(&Utc))
}

pub fn status(db: &Database) -> BackupStatus {
    let (_, remote) = super::auto::vault_settings();
    let last_ok = db.sync_kv_get(KEY_OK);
    let (err_at, err_msg) = db
        .sync_kv_get(KEY_ERR)
        .map(|v| match v.split_once('|') {
            Some((a, b)) => (Some(a.to_string()), Some(b.to_string())),
            None => (None, Some(v)),
        })
        .unwrap_or((None, None));
    let info = remote.as_deref().and_then(|r| parse_remote(r).ok()).map(|mut i| {
        resolve_alias(&mut i);
        i
    });
    let gaps = if remote.is_some() { coverage_gaps(db) } else { Vec::new() };
    let state = if remote.is_none() {
        "off"
    } else if last_ok.is_none() && err_msg.is_none() {
        "never"
    } else if err_at.as_deref().and_then(parse_ts) > last_ok.as_deref().and_then(parse_ts) {
        "failing"
    } else if last_ok.as_deref().and_then(parse_ts).is_some_and(|t| Utc::now() - t > ChronoDuration::days(STALE_AFTER_DAYS)) {
        "stale"
    } else if last_ok.is_none() {
        "failing"
    } else if !gaps.is_empty() {
        "partial"
    } else {
        "ok"
    };
    BackupStatus {
        state,
        remote: remote.as_deref().map(redact_url),
        provider: info.as_ref().map(|i| i.provider),
        provider_name: info.as_ref().map(|i| i.provider.name()),
        last_ok,
        last_error: err_msg,
        last_error_at: err_at,
        last_verified: db.sync_kv_get(KEY_VERIFIED),
        // only looked up once a backup is set up, and never created here: a status must not leave a secret in the keychain
        key_present: remote.is_some() && super::auto::existing_key().is_some(),
        gaps,
    }
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct Verification {
    pub remote_notes: i64,
    pub remote_decisions: i64,
    pub local_notes: i64,
    pub local_decisions: i64,
    /// Notes here that the remote does not hold yet (a pass is due, or the push failed).
    pub not_yet_backed_up: i64,
}

fn count_decisions(db: &Database) -> i64 {
    db.conn().query_row("SELECT COUNT(*) FROM decisions WHERE status = 'active'", [], |r| r.get(0)).unwrap_or(0)
}

/// Proves the copy on the remote restores: clones it to a scratch folder, replays it with the key into a scratch database and counts what is in it.
/// Nothing on this computer changes except the "verified" time.
pub fn verify_remote(db: &Database, url: &str, key: &[u8; 32]) -> Result<Verification> {
    let dir = scratch_dir();
    let result = (|| -> Result<Verification> {
        let target = dir.join("vault");
        let (ok, msg) = git(&dir, &["clone", "-q", "--depth", "1", "--branch", "main", url, target.to_string_lossy().as_ref()], Duration::from_secs(60))?;
        if !ok {
            bail!("{}", friendly_git_error(&msg));
        }
        let scratch = Database::open_in_memory()?;
        super::deltas::sync_cycle_keyed(&scratch, &target, Some(key)).context("the copy on the remote could not be opened with this key (wrong recovery phrase, or the vault was re-keyed)")?;
        let (rn, _) = scratch.count_memories(None)?;
        let (ln, _) = db.count_memories(None)?;
        Ok(Verification {
            remote_notes: rn,
            remote_decisions: count_decisions(&scratch),
            local_notes: ln,
            local_decisions: count_decisions(db),
            not_yet_backed_up: (ln - rn).max(0),
        })
    })();
    let _ = std::fs::remove_dir_all(&dir);
    if result.is_ok() {
        record_verified(db);
    }
    result
}

/// Used by tests and `doctor`: whether `dir` is a git repository with a remote called origin.
pub fn vault_has_origin(dir: &Path) -> bool {
    git(dir, &["remote", "get-url", "origin"], Duration::from_secs(5)).map(|(ok, _)| ok).unwrap_or(false)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn addresses_are_read_by_provider_and_unsafe_ones_refused() {
        let p = |u: &str| parse_remote(u).map(|i| (i.provider, i.host, i.ssh));
        assert_eq!(p("https://github.com/me/vault.git"), Ok((Provider::GitHub, "github.com".into(), false)));
        assert_eq!(p("git@github.com:me/vault.git"), Ok((Provider::GitHub, "github.com".into(), true)));
        assert_eq!(p("https://gitlab.com/me/vault.git").unwrap().0, Provider::GitLab);
        assert_eq!(p("https://gitlab.example.org/me/vault.git").unwrap().0, Provider::GitLab);
        assert_eq!(p("git@bitbucket.org:me/vault.git").unwrap().0, Provider::Bitbucket);
        assert_eq!(p("ssh://git@git.mycompany.io:2222/vault.git"), Ok((Provider::Other, "git.mycompany.io".into(), true)));
        assert_eq!(p("https://me:tok3n@gitea.example.com/me/vault.git").unwrap().1, "gitea.example.com");
        assert_eq!(p("/Volumes/Backup/vault.git").unwrap().0, Provider::Local);
        assert_eq!(p("file:///srv/vault.git").unwrap().0, Provider::Local);
        for bad in ["", "   ", "ext::sh -c touch /tmp/x", "fd::17", "https://github.com/a b.git", "https://x.com/a;rm -rf", "just-a-word", "https:///nohost", "git@host:"] {
            assert!(parse_remote(bad).is_err(), "should refuse {bad:?}");
        }
    }

    #[test]
    fn tokens_inside_an_address_are_hidden_on_screen_and_noticed() {
        assert_eq!(redact_url("https://me:tok3n@github.com/me/v.git"), "https://me:***@github.com/me/v.git");
        assert_eq!(redact_url("https://github.com/me/v.git"), "https://github.com/me/v.git");
        assert_eq!(redact_url("git@github.com:me/v.git"), "git@github.com:me/v.git");
        assert!(has_embedded_secret("https://me:tok3n@github.com/me/v.git"));
        assert!(!has_embedded_secret("https://github.com/me/v.git"));
    }

    #[test]
    fn an_ssh_alias_is_resolved_to_its_provider_and_option_like_hosts_are_refused() {
        let dir = std::env::temp_dir().join(format!("am-ssh-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let cfg = dir.join("config");
        std::fs::write(&cfg, "Host work-gh\n  HostName github.com\n  User git\nHost mine\n  HostName git.example.org\n").unwrap();
        let mut info = parse_remote("git@work-gh:me/vault.git").unwrap();
        assert_eq!(info.provider, Provider::Other, "an alias alone says nothing");
        resolve_alias_with(&mut info, &["-F", &cfg.to_string_lossy()]);
        assert_eq!((info.provider, info.host.as_str()), (Provider::GitHub, "github.com"));
        let mut other = parse_remote("git@mine:me/vault.git").unwrap();
        resolve_alias_with(&mut other, &["-F", &cfg.to_string_lossy()]);
        assert_eq!((other.provider, other.host.as_str()), (Provider::Other, "git.example.org"));
        assert!(parse_remote("git@-oProxyCommand=x:me/vault.git").is_err());
        assert!(parse_remote("ssh://-oProxyCommand=x/me/v.git").is_err());
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn data_the_backup_cannot_carry_is_found_and_explained() {
        use crate::models::*;
        let db = Database::open_in_memory().unwrap();
        let now = Utc::now();
        let mk = |id: &str, slug: &str| Project { id: id.into(), slug: slug.into(), display_name: slug.into(), created_at: now, updated_at: now, capture_policy: Default::default(), context_budget: Default::default(), document_config: Default::default(), sync_config: Default::default() };
        db.save_project(&mk("p-keyed", "keyed")).unwrap();
        db.set_project_key("p-keyed", "git:h/o/keyed").unwrap();
        db.save_project(&mk("p-nokey", "nokey")).unwrap();
        db.add_project_path("p-nokey", "/definitely/not/here", true).unwrap();
        // a project whose folder is here but has no key yet is only waiting for the next pass
        let here = std::env::temp_dir();
        db.save_project(&mk("p-pending", "pending")).unwrap();
        db.add_project_path("p-pending", &here.to_string_lossy(), true).unwrap();
        for (id, pid) in [("a", "p-keyed"), ("b", "p-nokey"), ("f", "p-pending"), ("c", "auto"), ("d", "folder-0123456789abcdef"), ("e", "demo")] {
            db.save_memory_direct(&note(pid, id, "x")).unwrap();
        }
        let g = coverage_gaps(&db);
        let reasons: Vec<(&str, &str)> = g.iter().map(|x| (x.project.as_str(), x.reason)).collect();
        assert_eq!(g.len(), 3, "{reasons:?}");
        assert!(reasons.contains(&("nokey", "no_key")) && reasons.contains(&("auto", "unknown_project")) && reasons.contains(&("folder-0123456789abcdef", "unregistered_folder")), "{reasons:?}");
        assert!(g.iter().find(|x| x.project == "auto").unwrap().fix.contains("project adopt auto"));
        assert!(!reasons.iter().any(|(p, _)| *p == "keyed" || *p == "demo" || *p == "pending"), "a keyed project, a project waiting for its key, and sample data are not gaps");
    }

    #[test]
    fn git_errors_become_a_fix() {
        assert!(friendly_git_error("git@github.com: Permission denied (publickey).").contains("ssh key"));
        assert!(friendly_git_error("fatal: could not read Username for 'https://github.com': terminal prompts disabled").contains("sign-in"));
        assert!(friendly_git_error("remote: Repository not found.").contains("not found"));
        assert!(friendly_git_error("fatal: unable to access: Could not resolve host: github.com").contains("network"));
    }

    fn sh(dir: &Path, args: &[&str]) {
        assert!(std::process::Command::new("git").args(args).current_dir(dir).output().unwrap().status.success(), "git {args:?}");
    }

    fn project(db: &Database, id: &str) {
        use crate::models::*;
        let now = Utc::now();
        db.save_project(&Project {
            id: id.into(), slug: "shop".into(), display_name: "shop".into(), created_at: now, updated_at: now,
            capture_policy: Default::default(), context_budget: Default::default(),
            document_config: Default::default(), sync_config: Default::default(),
        }).unwrap();
        db.set_project_key(id, "git:github.com/me/shop").unwrap();
    }

    fn note(project: &str, id: &str, text: &str) -> crate::models::Memory {
        use crate::models::*;
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
    fn the_copy_on_the_remote_is_proven_to_restore_and_a_wrong_key_is_caught() {
        let root = std::env::temp_dir().join(format!("am-bgv-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        let remote = root.join("remote.git");
        std::fs::create_dir_all(&remote).unwrap();
        sh(&remote, &["init", "-q", "--bare", "-b", "main"]);
        let url = remote.to_string_lossy().to_string();
        let key = [7u8; 32];
        let a = Database::open_in_memory().unwrap();
        project(&a, "id-a");
        a.save_memory_direct(&note("id-a", "m-1", "checkout uses idempotency keys")).unwrap();
        let va = root.join("vault-a");
        let r = super::super::auto::run(&a, &va, Some(&url), true, true, Some(&key)).unwrap();
        assert!(r.pushed, "{}", r.summary());
        assert!(a.sync_kv_get(KEY_OK).is_some(), "a successful pass is remembered");

        let v = verify_remote(&a, &url, &key).unwrap();
        assert_eq!((v.remote_notes, v.local_notes, v.not_yet_backed_up), (1, 1, 0), "{v:?}");
        assert!(a.sync_kv_get(KEY_VERIFIED).is_some());

        a.save_memory_direct(&note("id-a", "m-2", "a second note, not pushed yet")).unwrap();
        let v = verify_remote(&a, &url, &key).unwrap();
        assert_eq!((v.remote_notes, v.local_notes, v.not_yet_backed_up), (1, 2, 1), "{v:?}");

        let wrong = verify_remote(&a, &url, &[9u8; 32]);
        let notes_after_wrong_key = wrong.map(|v| v.remote_notes).unwrap_or(0);
        assert_eq!(notes_after_wrong_key, 0, "with the wrong key nothing of the remote can be read");
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn a_folder_remote_is_probed_and_the_state_follows_the_runs() {
        let root = std::env::temp_dir().join(format!("am-bg-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        let remote = root.join("remote.git");
        std::fs::create_dir_all(&remote).unwrap();
        sh(&remote, &["init", "-q", "--bare", "-b", "main"]);
        let url = remote.to_string_lossy().to_string();
        let p = probe(&url);
        assert!(p.reachable && p.empty && p.public.is_none(), "{p:?}");
        let gone = probe(&root.join("nope.git").to_string_lossy());
        assert!(!gone.reachable && gone.error.is_some());

        let db = Database::open_in_memory().unwrap();
        record_run(&db, &AutoReport { pushed: true, ..Default::default() });
        assert!(db.sync_kv_get(KEY_OK).is_some() && db.sync_kv_get(KEY_ERR).is_none());
        record_run(&db, &AutoReport { notes: vec!["push failed: offline".into()], ..Default::default() });
        assert!(db.sync_kv_get(KEY_ERR).is_some_and(|e| e.contains("offline")));
        record_run(&db, &AutoReport { skipped_locked: true, ..Default::default() });
        assert!(db.sync_kv_get(KEY_ERR).is_some(), "a skipped pass changes nothing");
        record_run(&db, &AutoReport { pulled: true, ..Default::default() });
        assert!(db.sync_kv_get(KEY_ERR).is_none(), "a success clears the failure");
        let _ = std::fs::remove_dir_all(&root);
    }
}
