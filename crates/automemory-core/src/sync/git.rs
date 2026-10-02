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
