//! Pre-merge file-overlap check (PRD §8): which other branches change the same files
//! as this one? Leases catch overlap while agents work; this catches it at merge time,
//! including work in other worktrees. Pure git, file-level.

use anyhow::{bail, Result};
use std::collections::BTreeSet;
use std::path::Path;
use std::process::Command;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BranchOverlap {
    pub branch: String,
    pub files: Vec<String>,
}

fn git(repo: &Path, args: &[&str]) -> Result<String> {
    let out = Command::new("git").arg("-C").arg(repo).args(args).output()?;
    if !out.status.success() {
        bail!("git {} failed: {}", args.join(" "), String::from_utf8_lossy(&out.stderr).trim());
    }
    Ok(String::from_utf8_lossy(&out.stdout).to_string())
}

/// Files `branch` changed since it diverged from `base` (three-dot diff).
pub fn changed_files(repo: &Path, base: &str, branch: &str) -> Result<BTreeSet<String>> {
    let range = format!("{base}...{branch}");
    Ok(git(repo, &["diff", "--name-only", &range])?
        .lines()
        .map(str::trim)
        .filter(|l| !l.is_empty())
        .map(String::from)
        .collect())
}

/// Local branches (other than `current` and `base`) that change files `current` also changes.
pub fn overlapping_branches(repo: &Path, base: &str, current: &str) -> Result<Vec<BranchOverlap>> {
    let mine = changed_files(repo, base, current)?;
    if mine.is_empty() {
        return Ok(Vec::new());
    }
    let mut out = Vec::new();
    for branch in git(repo, &["for-each-ref", "--format=%(refname:short)", "refs/heads"])?.lines() {
        let branch = branch.trim();
        if branch.is_empty() || branch == current || branch == base {
            continue;
        }
        let theirs = changed_files(repo, base, branch)?;
        let files: Vec<String> = mine.intersection(&theirs).cloned().collect();
        if !files.is_empty() {
            out.push(BranchOverlap { branch: branch.to_string(), files });
        }
    }
    Ok(out)
}

/// The branch currently checked out in `repo`.
pub fn current_branch(repo: &Path) -> Result<String> {
    Ok(git(repo, &["rev-parse", "--abbrev-ref", "HEAD"])?.trim().to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    fn sh(dir: &Path, args: &[&str]) {
        let ok = Command::new("git")
            .args(["-c", "user.name=t", "-c", "user.email=t@t"])
            .args(args)
            .current_dir(dir)
            .output()
            .unwrap()
            .status
            .success();
        assert!(ok, "git {args:?}");
    }

    fn commit_file(dir: &Path, name: &str, body: &str, msg: &str) {
        if let Some(p) = Path::new(name).parent() {
            fs::create_dir_all(dir.join(p)).unwrap();
        }
        fs::write(dir.join(name), body).unwrap();
        sh(dir, &["add", "-A"]);
        sh(dir, &["commit", "-qm", msg]);
    }

    #[test]
    fn finds_branches_touching_the_same_files() {
        let dir = std::env::temp_dir().join(format!("am-overlap-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        sh(&dir, &["init", "-q", "-b", "main"]);
        commit_file(&dir, "src/a.rs", "a", "base");
        commit_file(&dir, "src/b.rs", "b", "base b");

        sh(&dir, &["checkout", "-qb", "feature-x"]);
        commit_file(&dir, "src/a.rs", "a-x", "x edits a");
        sh(&dir, &["checkout", "-q", "main"]);
        sh(&dir, &["checkout", "-qb", "feature-y"]);
        commit_file(&dir, "src/a.rs", "a-y", "y edits a");
        commit_file(&dir, "src/c.rs", "c", "y adds c");
        sh(&dir, &["checkout", "-q", "main"]);
        sh(&dir, &["checkout", "-qb", "feature-z"]);
        commit_file(&dir, "src/b.rs", "b-z", "z edits b");

        let hits = overlapping_branches(&dir, "main", "feature-x").unwrap();
        assert_eq!(hits, [BranchOverlap { branch: "feature-y".into(), files: vec!["src/a.rs".into()] }]);
        assert!(overlapping_branches(&dir, "main", "feature-z").unwrap().is_empty());
        assert_eq!(current_branch(&dir).unwrap(), "feature-z");
        let _ = fs::remove_dir_all(&dir);
    }
}
