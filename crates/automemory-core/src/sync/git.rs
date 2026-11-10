use anyhow::{Context, Result};
use std::path::Path;
use std::process::Command;

pub struct GitSync;

impl GitSync {
    pub fn is_git_repo<P: AsRef<Path>>(dir: P) -> bool {
        dir.as_ref().join(".git").exists()
    }

    /// Resolves the canonical repository root for any path inside a Git repository
    /// or a Git worktree. For linked worktrees, parses the `.git` pointer file or
    /// invokes `git rev-parse --git-common-dir` so that multiple worktrees of the
    /// same repository resolve to the exact same canonical root directory.
    pub fn resolve_git_canonical_root<P: AsRef<Path>>(dir: P) -> Option<std::path::PathBuf> {
        let path = dir.as_ref();
        let canon = path.canonicalize().ok().unwrap_or_else(|| path.to_path_buf());

        // 1. Walk up to locate .git (file or directory)
        let mut curr = Some(canon.as_path());
        let mut worktree_root = None;
        while let Some(d) = curr {
            let git_entry = d.join(".git");
            if git_entry.exists() {
                worktree_root = Some(d.to_path_buf());
                if git_entry.is_file() {
                    // Linked worktree: .git file contains "gitdir: <path>"
                    if let Ok(content) = std::fs::read_to_string(&git_entry) {
                        for line in content.lines() {
                            let trimmed = line.trim();
                            if let Some(gitdir_str) = trimmed.strip_prefix("gitdir:") {
                                let gitdir_path = std::path::PathBuf::from(gitdir_str.trim());
                                let resolved = if gitdir_path.is_absolute() {
                                    gitdir_path
                                } else {
                                    d.join(gitdir_path)
                                };
                                // gitdir points to <main_repo>/.git/worktrees/<name>
                                // resolved.parent() -> .git/worktrees
                                // resolved.parent().parent() -> .git
                                // resolved.parent().parent().parent() -> <main_repo>
                                if let Some(main_repo) = resolved
                                    .parent()
                                    .and_then(|p| p.parent())
                                    .and_then(|p| p.parent())
                                {
                                    if let Ok(main_canon) = main_repo.canonicalize() {
                                        return Some(main_canon);
                                    }
                                }
                            }
                        }
                    }
                } else if git_entry.is_dir() {
                    // Main repository root
                    return Some(d.to_path_buf());
                }
                break;
            }
            curr = d.parent();
        }

        // 2. Fallback via git rev-parse --git-common-dir if gitdir parsing didn't find the main repo
        let output = Command::new("git")
            .args(["rev-parse", "--git-common-dir"])
            .current_dir(path)
            .output()
            .ok()?;
        if output.status.success() {
            let common_str = String::from_utf8_lossy(&output.stdout).trim().to_string();
            if !common_str.is_empty() {
                let common_path = std::path::PathBuf::from(&common_str);
                let full_common = if common_path.is_absolute() {
                    common_path
                } else {
                    path.join(common_path)
                };
                if let Ok(canon_common) = full_common.canonicalize() {
                    // The parent of the common .git directory is the canonical repo root
                    if let Some(parent) = canon_common.parent() {
                        return Some(parent.to_path_buf());
                    }
                }
            }
        }

        worktree_root
    }

    pub fn init_repo<P: AsRef<Path>>(dir: P) -> Result<()> {
        let status = Command::new("git")
            .arg("init")
            .current_dir(dir.as_ref())
            .status()
            .context("Failed to run git init")?;
        if !status.success() {
            anyhow::bail!("git init exited with non-zero status");
        }
        let _ = Command::new("git")
            .args(["branch", "-M", "main"])
            .current_dir(dir.as_ref())
            .status();
        Ok(())
    }

    pub fn set_remote<P: AsRef<Path>>(dir: P, remote_name: &str, remote_url: &str) -> Result<()> {
        let path = dir.as_ref();
        let output = Command::new("git")
            .args(["remote", "get-url", remote_name])
            .current_dir(path)
            .output();

        match output {
            Ok(out) if out.status.success() => {
                let current_url = String::from_utf8_lossy(&out.stdout).trim().to_string();
                if current_url != remote_url {
                    Command::new("git")
                        .args(["remote", "set-url", remote_name, remote_url])
                        .current_dir(path)
                        .status()
                        .context("Failed to set-url for remote")?;
                }
            }
            _ => {
                Command::new("git")
                    .args(["remote", "add", remote_name, remote_url])
                    .current_dir(path)
                    .status()
                    .context("Failed to add remote")?;
            }
        }
        Ok(())
    }

    pub fn get_remote_url<P: AsRef<Path>>(dir: P, remote_name: &str) -> Option<String> {
        let path = dir.as_ref();
        let output = Command::new("git")
            .args(["remote", "get-url", remote_name])
            .current_dir(path)
            .output()
            .ok()?;
        if output.status.success() {
            Some(String::from_utf8_lossy(&output.stdout).trim().to_string())
        } else {
            None
        }
    }

    pub fn commit_all<P: AsRef<Path>>(dir: P, message: &str) -> Result<bool> {
        let path = dir.as_ref();
        Command::new("git")
            .args(["add", "-A"])
            .current_dir(path)
            .status()
            .context("Failed to run git add")?;

        let output = Command::new("git")
            .args(["commit", "-m", message])
            .current_dir(path)
            .output()
            .context("Failed to run git commit")?;

        Ok(output.status.success())
    }

    pub fn push<P: AsRef<Path>>(dir: P, remote: Option<&str>, branch: Option<&str>) -> Result<()> {
        let remote_name = remote.unwrap_or("origin");
        let branch_name = branch.unwrap_or("main");
        if remote_name.starts_with('-') || branch_name.starts_with('-') {
            anyhow::bail!("remote and branch names may not start with '-'");
        }

        let _ = Command::new("git")
            .args(["branch", "-M", branch_name])
            .current_dir(dir.as_ref())
            .status();

        let output = Command::new("git")
            .args(["push", "-u", remote_name, branch_name])
            .current_dir(dir.as_ref())
            .output()
            .context("Failed to run git push")?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            anyhow::bail!("git push exited with error: {}", stderr.trim());
        }
        Ok(())
    }

    pub fn pull<P: AsRef<Path>>(dir: P, remote: Option<&str>, branch: Option<&str>) -> Result<()> {
        let remote_name = remote.unwrap_or("origin");
        let branch_name = branch.unwrap_or("main");
        if remote_name.starts_with('-') || branch_name.starts_with('-') {
            anyhow::bail!("remote and branch names may not start with '-'");
        }

        let output = Command::new("git")
            .args(["pull", "--rebase", remote_name, branch_name])
            .current_dir(dir.as_ref())
            .output()
            .context("Failed to run git pull")?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            anyhow::bail!("git pull exited with error: {}", stderr.trim());
        }
        Ok(())
    }
}
