//! `automemory backup git`: keep the encrypted vault on any git remote (GitHub, GitLab, Bitbucket, your own server) and prove it restores.
//! Setup is one guided command; after that the hooks and the daemon keep it current by themselves (docs/specs/git-backup.md).

use anyhow::{bail, Result};
use automemory_core::config::Config;
use automemory_core::db::Database;
use automemory_core::sync::backup_git as bg;
use std::io::{IsTerminal, Write};

fn ask(question: &str) -> bool {
    print!("{question} [y/N] ");
    let _ = std::io::stdout().flush();
    let mut a = String::new();
    std::io::stdin().read_line(&mut a).ok();
    matches!(a.trim().to_lowercase().as_str(), "y" | "yes")
}

fn read_line(prompt: &str) -> String {
    print!("{prompt}");
    let _ = std::io::stdout().flush();
    let mut a = String::new();
    std::io::stdin().read_line(&mut a).ok();
    a.trim().to_string()
}

fn db() -> Result<Database> {
    Database::open(&Config::default_config().db_path)
}

pub struct Args {
    pub action: Option<String>,
    pub url: Option<String>,
    pub yes: bool,
    pub allow_public: bool,
    pub phrase: Option<String>,
    pub json: bool,
}

pub fn run(a: Args) -> Result<()> {
    match a.action.as_deref().unwrap_or("status") {
        "status" => status(a.json),
        "setup" => setup(&a),
        "verify" => verify(),
        "now" => now(),
        "restore" => restore(&a),
        other => bail!("unknown action '{other}': use setup, status, now, verify or restore"),
    }
}

pub fn status(json: bool) -> Result<()> {
    let st = bg::status(&db()?);
    if json {
        println!("{}", serde_json::to_string_pretty(&st)?);
        return Ok(());
    }
    match st.state {
        "off" => {
            println!("Git backup: off. `automemory backup git setup` keeps an encrypted copy on GitHub, GitLab, Bitbucket or any git server.");
            return Ok(());
        }
        "ok" => println!("Git backup: working"),
        "partial" => println!("Git backup: working, but not everything can be backed up (see below)"),
        "never" => println!("Git backup: set up, waiting for the first successful sync (`automemory backup git now`)"),
        "stale" => println!("Git backup: ⚠ no successful sync for over a week"),
        _ => println!("Git backup: ⚠ failing"),
    }
    println!("  remote:   {} ({})", st.remote.as_deref().unwrap_or("?"), st.provider_name.unwrap_or("git"));
    println!("  last ok:  {}", st.last_ok.as_deref().unwrap_or("never"));
    if let Some(e) = &st.last_error {
        println!("  last problem: {e}{}", st.last_error_at.as_deref().map(|t| format!(" ({t})")).unwrap_or_default());
    }
    println!("  restore verified: {}", st.last_verified.as_deref().unwrap_or("never (`automemory backup git verify`)"));
    println!("  key on this computer: {}", if st.key_present { "yes" } else { "no (restoring elsewhere needs the recovery phrase)" });
    print_gaps(&st.gaps);
    Ok(())
}

fn print_gaps(gaps: &[bg::Gap]) {
    if gaps.is_empty() {
        return;
    }
    println!("\n  NOT backed up on this computer:");
    for g in gaps {
        let mut what = Vec::new();
        if g.notes > 0 {
            what.push(format!("{} note(s)", g.notes));
        }
        if g.decisions > 0 {
            what.push(format!("{} decision(s)", g.decisions));
        }
        if g.other > 0 {
            what.push(format!("{} hand-off(s), trace(s) or skill(s)", g.other));
        }
        println!("    • {}: {}\n        {}", g.project, what.join(", "), g.fix);
    }
}

pub fn now() -> Result<()> {
    super::sync::auto(true, true)?;
    status(false)
}

pub fn verify() -> Result<()> {
    let db = db()?;
    let (_, remote) = automemory_core::sync::auto::vault_settings();
    let Some(url) = remote else { bail!("no git backup is set up (`automemory backup git setup`)") };
    let Some(key) = automemory_core::sync::auto::existing_key() else { bail!("this computer has no vault key; restore with the recovery phrase first") };
    println!("Cloning the backup into a scratch folder and opening it with your key…");
    let v = bg::verify_remote(&db, &url, &key)?;
    println!("✓ The copy on the remote restores: {} note(s) and {} decision(s).", v.remote_notes, v.remote_decisions);
    if v.not_yet_backed_up > 0 {
        println!("  {} note(s) here are not on the remote yet; the next sync pass sends them (`automemory backup git now`).", v.not_yet_backed_up);
    }
    Ok(())
}

pub fn setup(a: &Args) -> Result<()> {
    let interactive = std::io::stdin().is_terminal();
    if !interactive && !a.yes {
        bail!("not a terminal: pass the address and --yes (and --allow-public if the repository is public)");
    }
    println!("Git backup: an encrypted copy of your memory on a git server of your choice.\n  Everything is encrypted on this computer before it is pushed; the server only ever sees ciphertext.\n");
    let url = match a.url.clone() {
        Some(u) => u,
        None => {
            println!("First make an EMPTY, PRIVATE repository:");
            for p in [bg::Provider::GitHub, bg::Provider::GitLab, bg::Provider::Bitbucket] {
                println!("  • {}", bg::create_hint(p));
            }
            println!("  • Any other git server works too.\n");
            read_line("Repository address (https://… or git@host:owner/repo.git): ")
        }
    };
    let mut info = match bg::parse_remote(&url) {
        Ok(i) => i,
        Err(e) => bail!("{e}"),
    };
    bg::resolve_alias(&mut info);
    println!("→ {} ({})", bg::redact_url(&info.url), info.provider.name());
    if bg::has_embedded_secret(&info.url) {
        println!("⚠ The address contains a password or token. It would be stored in plain text in ~/.automemory/config.toml and in the vault's git config.\n  Better: an ssh address, or sign in once with your git credential helper (`gh auth login`, `glab auth login`).");
        if !a.yes && !ask("Use it anyway?") {
            bail!("nothing changed");
        }
    }
    print!("  checking the repository… ");
    let _ = std::io::stdout().flush();
    let p = bg::probe(&info.url);
    if !p.reachable {
        println!("✗");
        println!("  {}", p.error.unwrap_or_else(|| "it could not be reached".into()));
        println!("  {}", bg::create_hint(info.provider));
        bail!("nothing changed");
    }
    println!("✓ reachable");
    if p.public == Some(true) {
        println!("⚠ Anyone can read this repository. Your data is encrypted, but a backup repository should be private (history, file names and timing are visible).");
        if !a.allow_public && !(interactive && !a.yes && ask("Use it anyway?")) {
            bail!("make the repository private and run this again (or pass --allow-public)");
        }
    }
    if !p.empty {
        println!("⚠ The repository is not empty. If it already holds an AutoMemory vault from another computer, restore from it instead:\n    automemory backup git restore {} --phrase \"<recovery phrase>\"", bg::redact_url(&info.url));
        if !a.yes && !ask("Use it as the backup for this computer anyway? (a vault from another computer would be merged)") {
            bail!("nothing changed");
        }
    }
    super::sync::remote(&info.url)?;
    let Some(key) = automemory_core::sync::auto::resolve_key(true) else { bail!("could not create or read the vault key on this computer") };
    let phrase = automemory_core::bip39::master_key_to_mnemonic_24(&key)?;
    println!("\nYour recovery phrase (24 words). It is the ONLY way to restore this backup on another computer, and nobody can recover it for you:\n");
    println!("  {phrase}\n");
    if interactive && !a.yes {
        let words: Vec<&str> = phrase.split_whitespace().collect();
        let n = 1 + (std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.subsec_nanos() as usize).unwrap_or(0) % words.len());
        loop {
            let got = read_line(&format!("Save it somewhere safe (a password manager), then type word number {n} to confirm: "));
            if got.eq_ignore_ascii_case(words[n - 1]) {
                break;
            }
            if got.is_empty() || got == "skip" {
                println!("⚠ Skipped. Run `automemory sync pair` any time to see the phrase again.");
                break;
            }
            println!("  not the right word; try again, or type skip");
        }
    } else {
        println!("⚠ Save this phrase now (`automemory sync pair` shows it again).");
    }
    println!("\nFirst backup…");
    super::sync::auto(true, true)?;
    let db = db()?;
    match bg::verify_remote(&db, &info.url, &key) {
        Ok(v) if v.not_yet_backed_up > 0 => println!("⚠ The remote restores, but holds {} note(s) and {} decision(s) while this computer has {} and {}: {} note(s) are not on it yet. `automemory backup git now` sends them.", v.remote_notes, v.remote_decisions, v.local_notes, v.local_decisions, v.not_yet_backed_up),
        Ok(v) => println!("✓ Verified: the copy on the remote restores ({} note(s), {} decision(s)).", v.remote_notes, v.remote_decisions),
        Err(e) => println!("⚠ The first backup could not be verified yet: {e}\n  `automemory backup git now` retries; `automemory backup git status` shows what is wrong."),
    }
    {
        let gaps = bg::coverage_gaps(&db);
        if !gaps.is_empty() {
            println!("\n⚠ Not everything is covered:");
            print_gaps(&gaps);
        }
    }
    println!("\nFrom now on it runs by itself: after your agents work, in the background. `automemory doctor` and the dashboard show when it last worked.");
    println!("On another computer: `automemory backup git restore {} --phrase \"<recovery phrase>\"`", bg::redact_url(&info.url));
    Ok(())
}

pub fn restore(a: &Args) -> Result<()> {
    let url = match a.url.clone() {
        Some(u) => u,
        None if std::io::stdin().is_terminal() => read_line("Repository address of your backup: "),
        None => bail!("pass the repository address"),
    };
    let info = bg::parse_remote(&url).map_err(anyhow::Error::msg)?;
    let phrase = match a.phrase.clone() {
        Some(p) => Some(p),
        None if std::io::stdin().is_terminal() => {
            #[cfg(unix)]
            let p = automemory_core::auth::prompt_secure("Recovery phrase (12 or 24 words, not shown): ").unwrap_or_default();
            #[cfg(not(unix))]
            let p = read_line("Recovery phrase (12 or 24 words): ");
            (!p.trim().is_empty()).then_some(p)
        }
        None => None,
    };
    println!("Restoring from {} ({})…", bg::redact_url(&info.url), info.provider.name());
    let pr = bg::probe(&info.url);
    if !pr.reachable {
        bail!("{}", pr.error.unwrap_or_else(|| "the repository could not be reached".into()));
    }
    if pr.empty {
        bail!("that repository is empty: there is nothing to restore from it");
    }
    let db = db()?;
    // Prove the phrase opens this backup BEFORE it replaces anything: a wrong phrase must not overwrite the key already on this computer.
    if let Some(words) = phrase.as_deref() {
        let key = automemory_core::bip39::mnemonic_to_master_key(words).map_err(|e| anyhow::anyhow!("that is not a valid recovery phrase: {e}"))?;
        match bg::verify_remote(&db, &info.url, &key) {
            Ok(v) if v.remote_notes + v.remote_decisions > 0 => println!("✓ The phrase opens the backup ({} note(s), {} decision(s)).", v.remote_notes, v.remote_decisions),
            Ok(_) => bail!("the phrase does not open this backup (or the backup holds nothing); nothing was changed on this computer"),
            Err(e) => bail!("the backup could not be opened with that phrase: {e}\n  nothing was changed on this computer"),
        }
    }
    super::sync::bootstrap(&info.url, phrase, false)?;
    if let Some(key) = automemory_core::sync::auto::existing_key() {
        if let Ok(v) = bg::verify_remote(&db, &info.url, &key) {
            println!("✓ This computer now holds {} note(s) and {} decision(s); the remote holds {} and {}.", v.local_notes, v.local_decisions, v.remote_notes, v.remote_decisions);
        }
    }
    println!("Connect your agents here with `automemory connect all -g`.");
    Ok(())
}

/// Why a setup did not go through; the dashboard shows the sentence and, for some, a "use it anyway" button.
#[derive(Debug)]
pub enum SetupIssue {
    Invalid(String),
    Unreachable(String),
    /// Anyone can read the repository.
    Public,
    /// The repository already holds branches (maybe a vault from another computer).
    NotEmpty,
    /// The address carries a password or token.
    Secret,
    Failed(String),
}

pub struct SetupOk {
    pub provider: &'static str,
    pub remote: String,
    pub phrase: String,
    pub verified: Option<bg::Verification>,
    pub note: Option<String>,
}

/// The same steps as `setup`, without prompts, for the dashboard: check the address, point the vault at it, make the key, back up once, prove the copy restores.
pub fn setup_core(url: &str, allow_public: bool, allow_non_empty: bool, allow_secret: bool) -> Result<SetupOk, SetupIssue> {
    let info = bg::parse_remote(url).map_err(SetupIssue::Invalid)?;
    if bg::has_embedded_secret(&info.url) && !allow_secret {
        return Err(SetupIssue::Secret);
    }
    let p = bg::probe(&info.url);
    if !p.reachable {
        return Err(SetupIssue::Unreachable(p.error.unwrap_or_else(|| "it could not be reached".into())));
    }
    if p.public == Some(true) && !allow_public {
        return Err(SetupIssue::Public);
    }
    if !p.empty && !allow_non_empty {
        return Err(SetupIssue::NotEmpty);
    }
    super::sync::remote(&info.url).map_err(|e| SetupIssue::Failed(e.to_string()))?;
    let key = automemory_core::sync::auto::resolve_key(true).ok_or_else(|| SetupIssue::Failed("could not create or read the vault key on this computer".into()))?;
    let phrase = automemory_core::bip39::master_key_to_mnemonic_24(&key).map_err(|e| SetupIssue::Failed(e.to_string()))?;
    let note = match super::sync::auto(true, true) {
        Ok(()) => None,
        Err(e) => Some(format!("the first backup did not finish: {e}")),
    };
    let db = Database::open(&Config::default_config().db_path).map_err(|e| SetupIssue::Failed(e.to_string()))?;
    let verified = bg::verify_remote(&db, &info.url, &key).ok();
    Ok(SetupOk { provider: info.provider.name(), remote: bg::redact_url(&info.url), phrase, verified, note })
}

/// Turn the git backup off: the vault stays on this computer, the remote copy is left where it is.
pub fn turn_off() -> Result<()> {
    super::sync::remote("--clear")
}
