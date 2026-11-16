//! `automemory update`: replaces this binary with the latest release, after showing what it will do and checking
//! the download against the release's `.sha256` file. Never silent: `--check` only reports, otherwise it asks.

use anyhow::{anyhow, bail, Context, Result};
use serde_json::Value;
use sha2::{Digest, Sha256};
use std::io::Write;
use std::path::Path;

const DEFAULT_API: &str = "https://api.github.com/repos/theasmat/automemory-bin/releases/latest";

fn triple() -> Result<String> {
    let arch = match std::env::consts::ARCH {
        "aarch64" => "aarch64",
        "x86_64" => "x86_64",
        other => bail!("no prebuilt release for CPU {other}"),
    };
    let os = match std::env::consts::OS {
        "macos" => "apple-darwin",
        "linux" => "unknown-linux-gnu",
        "windows" => "pc-windows-msvc",
        other => bail!("`update` supports macOS, Linux and Windows; on {other} use the install script"),
    };
    Ok(format!("{arch}-{os}"))
}

/// Windows releases are zip files holding `automemory.exe`; the others are tar.gz holding `automemory`.
fn archive_name() -> Result<String> {
    Ok(format!("automemory-{}.{}", triple()?, if cfg!(windows) { "zip" } else { "tar.gz" }))
}

const BIN_FILE: &str = if cfg!(windows) { "automemory.exe" } else { "automemory" };

/// `0.1.10` > `0.1.9`: numeric, left to right. Anything that is not a plain version counts as not newer.
pub fn is_newer(latest: &str, current: &str) -> bool {
    let parse = |v: &str| -> Option<Vec<u64>> { v.trim_start_matches('v').split('.').map(|p| p.parse().ok()).collect() };
    match (parse(latest), parse(current)) {
        (Some(l), Some(c)) => l > c,
        _ => false,
    }
}

/// The sha256 a `shasum` line carries (`<hex>  <file>`), only if it names this archive.
pub fn expected_hash(line: &str, archive: &str) -> Option<String> {
    let mut it = line.split_whitespace();
    let (hash, name) = (it.next()?, it.next()?);
    (name.trim_start_matches('*') == archive && hash.len() == 64 && hash.chars().all(|c| c.is_ascii_hexdigit())).then(|| hash.to_lowercase())
}

pub(crate) fn get(client: &reqwest::blocking::Client, url: &str) -> Result<reqwest::blocking::Response> {
    let r = client.get(url).header("User-Agent", "automemory-update").send().with_context(|| format!("cannot reach {url}"))?;
    if !r.status().is_success() {
        bail!("{url} answered {}", r.status());
    }
    Ok(r)
}

pub fn run(check: bool, yes: bool) -> Result<()> {
    let current = env!("CARGO_PKG_VERSION");
    let api = std::env::var("AUTOMEMORY_UPDATE_API").unwrap_or_else(|_| DEFAULT_API.to_string());
    let client = reqwest::blocking::Client::builder().timeout(std::time::Duration::from_secs(120)).build()?;
    let release: Value = get(&client, &api)?.json()?;
    let tag = release["tag_name"].as_str().ok_or_else(|| anyhow!("the release has no tag"))?;
    if !is_newer(tag, current) {
        println!("✓ Up to date ({current}).");
        return Ok(());
    }
    println!("A newer version is available: {tag} (you have {current}).");
    if check {
        println!("Run `automemory update` to install it.");
        return Ok(());
    }
    let archive = archive_name()?;
    let asset = |name: &str| release["assets"].as_array().and_then(|a| a.iter().find(|x| x["name"] == name)).and_then(|x| x["browser_download_url"].as_str()).map(String::from);
    let (Some(archive_url), Some(sum_url)) = (asset(&archive), asset(&format!("{archive}.sha256"))) else {
        bail!("release {tag} has no {archive} with a .sha256 file; not installing an unchecked download");
    };
    let exe = std::env::current_exe()?.canonicalize()?;
    let _ = std::fs::remove_file(exe.with_extension("old")); // left by an earlier Windows update
    if !yes {
        print!("Replace {} with {tag}? [y/N] ", exe.display());
        std::io::stdout().flush()?;
        let mut answer = String::new();
        std::io::stdin().read_line(&mut answer)?;
        if !answer.trim().eq_ignore_ascii_case("y") {
            println!("Nothing changed.");
            return Ok(());
        }
    }
    let want = expected_hash(&get(&client, &sum_url)?.text()?, &archive).ok_or_else(|| anyhow!("the checksum file does not describe {archive}"))?;
    let bytes = get(&client, &archive_url)?.bytes()?;
    let got = format!("{:x}", Sha256::digest(&bytes));
    if got != want {
        bail!("checksum mismatch (expected {want}, got {got}); nothing was installed");
    }
    let dir = exe.parent().ok_or_else(|| anyhow!("no folder for the binary"))?;
    let work = dir.join(format!(".automemory-update-{}", std::process::id()));
    std::fs::create_dir_all(&work)?;
    let result = install(&bytes, &work, &exe);
    let _ = std::fs::remove_dir_all(&work);
    result?;
    println!("✓ Updated to {tag}. Restart `automemory serve` and any agent that has it open.");
    Ok(())
}

fn install(archive: &[u8], work: &Path, exe: &Path) -> Result<()> {
    let tar = work.join(if cfg!(windows) { "a.zip" } else { "a.tar.gz" });
    std::fs::write(&tar, archive)?;
    // `tar` ships with macOS, Linux and Windows 10+ (bsdtar reads zip files too).
    let flags = if cfg!(windows) { "-xf" } else { "-xzf" };
    let ok = std::process::Command::new("tar").arg(flags).arg(&tar).arg("-C").arg(work).status()?.success();
    let new = work.join(BIN_FILE);
    if !ok || !new.is_file() {
        bail!("the archive did not contain `{BIN_FILE}`");
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(&new, std::fs::Permissions::from_mode(0o755))?;
    }
    // Same folder, so the rename is atomic and a running copy keeps working until it exits.
    let staged = exe.with_extension("new");
    std::fs::copy(&new, &staged)?;
    if cfg!(target_os = "macos") {
        let _ = std::process::Command::new("codesign").args(["-s", "-", "-f"]).arg(&staged).output();
        let _ = std::process::Command::new("xattr").args(["-d", "com.apple.quarantine"]).arg(&staged).output();
    }
    if cfg!(windows) {
        // A running .exe cannot be overwritten but can be renamed; the old copy is deleted on the next update.
        let old = exe.with_extension("old");
        let _ = std::fs::remove_file(&old);
        std::fs::rename(exe, &old)?;
        if let Err(e) = std::fs::rename(&staged, exe) {
            let _ = std::fs::rename(&old, exe); // put the working binary back
            return Err(e.into());
        }
        return Ok(());
    }
    std::fs::rename(&staged, exe)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn versions_compare_numerically() {
        assert!(is_newer("v0.1.10", "0.1.9") && is_newer("1.0.0", "0.9.9"));
        assert!(!is_newer("v0.1.0", "0.1.0") && !is_newer("0.0.9", "0.1.0") && !is_newer("nightly", "0.1.0"));
    }

    #[test]
    fn only_a_checksum_line_naming_the_archive_counts() {
        let h = "a".repeat(64);
        assert_eq!(expected_hash(&format!("{h}  automemory-x.tar.gz"), "automemory-x.tar.gz"), Some(h.clone()));
        assert_eq!(expected_hash(&format!("{h}  other.tar.gz"), "automemory-x.tar.gz"), None);
        assert_eq!(expected_hash("deadbeef  automemory-x.tar.gz", "automemory-x.tar.gz"), None);
    }
}
