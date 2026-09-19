use anyhow::{Context, Result};
use std::path::Path;
use std::process::Command;

pub struct GitSync;

impl GitSync {
    pub fn is_git_repo<P: AsRef<Path>>(dir: P) -> bool {
        dir.as_ref().join(".git").exists()
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
        Ok(())
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

        let status = Command::new("git")
            .args(["push", remote_name, branch_name])
            .current_dir(dir.as_ref())
            .status()
            .context("Failed to run git push")?;

        if !status.success() {
            anyhow::bail!("git push exited with error");
        }
        Ok(())
    }

    pub fn pull<P: AsRef<Path>>(dir: P, remote: Option<&str>, branch: Option<&str>) -> Result<()> {
        let remote_name = remote.unwrap_or("origin");
        let branch_name = branch.unwrap_or("main");

        let status = Command::new("git")
            .args(["pull", "--rebase", remote_name, branch_name])
            .current_dir(dir.as_ref())
            .status()
            .context("Failed to run git pull")?;

        if !status.success() {
            anyhow::bail!("git pull exited with error");
        }
        Ok(())
    }
}
