//! `automemory companion`: the optional Tracewise companion (docs/specs/tracewise-companion.md). Shows its state, switches it on or off,
//! installs it from its checksummed release (macOS), and keeps the two products' hooks from running twice.

use anyhow::{anyhow, bail, Result};
use automemory_core::companion;
use automemory_core::config::{Config, GlobalConfig};
use automemory_core::db::Database;
use serde_json::Value;
use sha2::{Digest, Sha256};
use std::io::Write;
use std::time::Instant;

const DEFAULT_API: &str = "https://api.github.com/repos/cmdworks/tracewise/releases";

pub fn run(action: Option<&str>, yes: bool) -> Result<()> {
    match action.unwrap_or("status") {
        "status" => status(),
        mode @ ("auto" | "on" | "off") => set_mode(mode),
        "install" => install(yes),
        other => bail!("unknown action '{other}': use status, install, auto, on or off"),
    }
}

fn set_mode(mode: &str) -> Result<()> {
    let mut g = GlobalConfig::load().unwrap_or_default();
    g.companion.tracewise = mode.to_string();
    g.save()?;
    if let Ok(db) = Database::open(&Config::default_config().db_path) {
        db.sync_kv_delete("companion_probe"); // the next hook decides afresh
    }
    println!("✓ Tracewise companion: {mode}.");
    status()
}

/// Tracewise's own hooks for an agent run next to AutoMemory's nested call, so the same event would be handled twice.
pub fn duplicate_hooks(db: &Database) -> Vec<String> {
    let dir = std::env::current_dir().unwrap_or_default();
    let settings = GlobalConfig::load().unwrap_or_default().companion;
    companion::json_in(&settings, db, &dir, &["agent", "status", "--json"])
        .ok()
        .and_then(|v| v.as_array().cloned())
        .unwrap_or_default()
        .iter()
        .filter(|a| a["hooks"].as_bool() == Some(true))
        .filter_map(|a| a["name"].as_str().map(String::from))
        .collect()
}

fn status() -> Result<()> {
    let db = Database::open(&Config::default_config().db_path)?;
    let settings = GlobalConfig::load().unwrap_or_default().companion;
    println!("Tracewise companion (file locks and history; optional)");
    println!("  mode:  {}", settings.tracewise);
    if settings.tracewise.eq_ignore_ascii_case("off") {
        println!("  state: off, nothing is locked. `automemory companion auto` turns it back on.");
        return Ok(());
    }
    // a fresh probe, so the latency is measured now and not read from the one-minute cache
    db.sync_kv_delete("companion_probe");
    let t = Instant::now();
    match companion::detect(&settings, &db) {
        Some(c) => {
            println!("  state: ready, tracewise {} at {} (companion api {})", c.version, c.path.display(), companion::API);
            println!("  probe: {} ms (the nested call in a hook costs about 4 ms more than AutoMemory alone)", t.elapsed().as_millis());
            let dup = duplicate_hooks(&db);
            if dup.is_empty() {
                println!("  hooks: one registration per agent (AutoMemory's, with Tracewise nested)");
            } else {
                println!("  hooks: ⚠ Tracewise also has its own hooks for {} and they would run twice. `automemory connect <agent>` removes them.", dup.join(", "));
            }
        }
        None if !cfg!(target_os = "macos") => println!("  state: not available here: Tracewise runs on macOS only for now. Memory, decisions and hand-offs work; nothing is locked."),
        None => println!("  state: not installed or too old/new for companion api {}. Nothing is locked. `automemory companion install` installs it.", companion::API),
    }
    Ok(())
}

/// After `connect`: remove Tracewise's own hook registration for these agents (AutoMemory's hook calls Tracewise itself).
pub fn dedupe_hooks(agents: &[String]) {
    let Ok(db) = Database::open(&Config::default_config().db_path) else { return };
    let settings = GlobalConfig::load().unwrap_or_default().companion;
    let Some(c) = companion::detect(&settings, &db) else { return };
    let mine: std::collections::HashSet<String> = duplicate_hooks(&db).into_iter().collect();
    for a in agents {
        let tw = companion::tracewise_agent(a).to_string();
        if mine.contains(&tw) {
            let ok = std::process::Command::new(&c.path).args(["agent", "uninstall", &tw]).output().is_ok_and(|o| o.status.success());
            println!("{} Removed Tracewise's own hooks for {tw}: AutoMemory's hook now calls Tracewise itself.", if ok { "✓" } else { "⚠ could not remove" });
        }
    }
}

fn archive_part() -> Result<&'static str> {
    if !cfg!(target_os = "macos") {
        bail!("Tracewise runs on macOS only for now; there is nothing to install here (memory works without it)");
    }
    Ok(if std::env::consts::ARCH == "aarch64" { "macos-arm64" } else { "macos-x86_64" })
}

fn install(yes: bool) -> Result<()> {
    let part = archive_part()?;
    let db = Database::open(&Config::default_config().db_path)?;
    let settings = GlobalConfig::load().unwrap_or_default().companion;
    db.sync_kv_delete("companion_probe"); // look afresh, not at the one-minute cache
    if companion::detect(&settings, &db).is_some() {
        println!("✓ Tracewise is already installed and answers the companion api.");
        return Ok(());
    }
    let api = std::env::var("AUTOMEMORY_TRACEWISE_API").unwrap_or_else(|_| DEFAULT_API.to_string());
    let client = reqwest::blocking::Client::builder().timeout(std::time::Duration::from_secs(120)).build()?;
    let releases: Value = super::update::get(&client, &api)?.json()?;
    // newest release (pre-releases included) that has this Mac's archive and its checksum file
    let found = releases.as_array().into_iter().flatten().filter(|r| r["draft"] != true).find_map(|r| {
        let assets = r["assets"].as_array()?;
        let a = assets.iter().find(|a| a["name"].as_str().is_some_and(|n| n.contains(part) && n.ends_with(".tar.gz") && !n.contains("desktop")))?;
        let name = a["name"].as_str()?.to_string();
        let url = a["browser_download_url"].as_str()?.to_string();
        let sum = assets.iter().find(|s| s["name"] == format!("{name}.sha256"))?["browser_download_url"].as_str()?.to_string();
        Some((r["tag_name"].as_str()?.to_string(), name, url, sum))
    });
    let (tag, name, url, sum_url) = found.ok_or_else(|| anyhow!("no Tracewise release has a checksummed {part} archive"))?;
    println!("Tracewise {tag} ({name}): `tracewise setup` installs programs in ~/.tracewise/bin, adds one block to ~/.zshrc and starts a background file watcher (no sudo).");
    if !yes {
        print!("Download, verify and set it up? [y/N] ");
        std::io::stdout().flush()?;
        let mut a = String::new();
        std::io::stdin().read_line(&mut a)?;
        if !a.trim().eq_ignore_ascii_case("y") {
            println!("Nothing changed.");
            return Ok(());
        }
    }
    let want = super::update::expected_hash(&super::update::get(&client, &sum_url)?.text()?, &name).ok_or_else(|| anyhow!("the checksum file does not describe {name}"))?;
    let bytes = super::update::get(&client, &url)?.bytes()?;
    let got = format!("{:x}", Sha256::digest(&bytes));
    if got != want {
        bail!("checksum mismatch (expected {want}, got {got}); nothing was installed");
    }
    let work = std::env::temp_dir().join(format!("automemory-tracewise-{}", std::process::id()));
    std::fs::create_dir_all(&work)?;
    let tar = work.join("t.tar.gz");
    std::fs::write(&tar, &bytes)?;
    let ok = std::process::Command::new("tar").arg("-xzf").arg(&tar).arg("-C").arg(&work).status()?.success();
    let bin = std::fs::read_dir(&work)?.flatten().map(|e| e.path().join("tracewise")).find(|p| p.is_file()).or_else(|| Some(work.join("tracewise")).filter(|p| p.is_file()));
    let result = match (ok, bin) {
        (true, Some(b)) => {
            let _ = std::process::Command::new("xattr").args(["-dr", "com.apple.quarantine"]).arg(&work).output();
            std::process::Command::new(&b).arg("setup").stdin(std::process::Stdio::null()).status().map(|s| s.success())
        }
        _ => Ok(false),
    };
    let _ = std::fs::remove_dir_all(&work);
    if !result.unwrap_or(false) {
        bail!("`tracewise setup` did not finish; see its output above");
    }
    println!("✓ Tracewise installed. Open a new terminal, then run `automemory connect all -g` so each agent's hook calls it.");
    Ok(())
}
