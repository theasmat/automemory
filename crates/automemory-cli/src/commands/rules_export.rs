//! `automemory export-rules`: writes what is verified about the project into the instruction
//! file each agent already reads, so a session in any agent or IDE starts informed without
//! calling a tool. Only our marked block is ever touched; the rest of a file is the user's.

use anyhow::Result;
use automemory_core::db::Database;
use std::fs;
use std::path::{Path, PathBuf};

const BEGIN: &str = "<!-- automemory:begin (generated; change it with `automemory note` / `automemory decision`) -->";
const END: &str = "<!-- automemory:end -->";

/// `detect` is a path whose presence shows the agent is used in this repository.
pub struct Target {
    pub agent: &'static str,
    pub path: &'static str,
    detect: &'static str,
    /// Front matter for files we create whole (the agent needs it to load the rule).
    front: &'static str,
}

pub const TARGETS: &[Target] = &[
    Target { agent: "agents", path: "AGENTS.md", detect: ".", front: "" },
    Target { agent: "claude", path: "CLAUDE.md", detect: ".claude", front: "" },
    Target { agent: "gemini-cli", path: "GEMINI.md", detect: ".gemini", front: "" },
    Target { agent: "copilot", path: ".github/copilot-instructions.md", detect: ".vscode/mcp.json", front: "" },
    Target { agent: "cursor", path: ".cursor/rules/automemory.mdc", detect: ".cursor", front: "---\ndescription: Project memory from AutoMemory\nalwaysApply: true\n---\n" },
    Target { agent: "windsurf", path: ".windsurf/rules/automemory.md", detect: ".windsurf", front: "" },
    Target { agent: "cline", path: ".clinerules/automemory.md", detect: ".clinerules", front: "" },
    Target { agent: "roo", path: ".roo/rules/automemory.md", detect: ".roo", front: "" },
    Target { agent: "kiro", path: ".kiro/steering/automemory.md", detect: ".kiro", front: "" },
];

fn block(view: &str) -> String {
    let body = if view.trim().is_empty() { "_Nothing verified yet._\n".to_string() } else { format!("{}\n", view.trim_end()) };
    format!(
        "{BEGIN}\n## Project memory (AutoMemory)\nShared by every agent on this project. Start from this instead of re-exploring; for more call `memory.context` (MCP). Record non-trivial choices with `decision.record`. File locks are automatic.\n\n{body}{END}\n"
    )
}

/// The text without our block (and a trailing blank line left by it).
fn strip(text: &str) -> String {
    let (Some(a), Some(z)) = (text.find("<!-- automemory:begin"), text.find(END)) else { return text.to_string() };
    if z < a {
        return text.to_string();
    }
    let rest = text[z + END.len()..].trim_start_matches('\n');
    let head = text[..a].trim_end_matches('\n');
    match (head.is_empty(), rest.is_empty()) {
        (true, true) => String::new(),
        (true, false) => rest.to_string(),
        (false, true) => format!("{head}\n"),
        (false, false) => format!("{head}\n\n{rest}"),
    }
}

/// The file's new text: our block replaced in place, or appended.
fn splice(existing: &str, front: &str, view: &str) -> String {
    let b = block(view);
    if let (Some(a), Some(z)) = (existing.find("<!-- automemory:begin"), existing.find(END)) {
        if a < z {
            return format!("{}{b}{}", &existing[..a], existing[z + END.len()..].trim_start_matches('\n'));
        }
    }
    if existing.trim().is_empty() {
        return format!("{front}{b}");
    }
    format!("{}\n\n{b}", existing.trim_end())
}

#[derive(Debug, PartialEq)]
pub enum Outcome {
    Created,
    Updated,
    Unchanged,
}

/// False when the file is a symlink or sits behind one that leaves the repository: a hostile repo could point
/// `AGENTS.md` at `~/.ssh/...` to have its content copied into the repo (or `.cursor` at a system folder).
fn stays_inside(root: &Path, path: &Path) -> bool {
    if fs::symlink_metadata(path).is_ok_and(|m| m.file_type().is_symlink()) {
        return false;
    }
    let Ok(root) = root.canonicalize() else { return false };
    path.ancestors().skip(1).find_map(|a| a.canonicalize().ok()).is_some_and(|a| a.starts_with(&root))
}

fn write_one(root: &Path, t: &Target, view: &str, create: bool) -> Result<Option<Outcome>> {
    let path = root.join(t.path);
    if !stays_inside(root, &path) {
        return Ok(None);
    }
    let existing = fs::read_to_string(&path).ok();
    if existing.is_none() && !create {
        return Ok(None);
    }
    let old = existing.clone().unwrap_or_default();
    let new = splice(&old, t.front, view);
    if existing.is_some() && new == old {
        return Ok(Some(Outcome::Unchanged));
    }
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    let tmp = path.with_extension(format!("tmp{}", std::process::id()));
    fs::write(&tmp, &new)?;
    fs::rename(&tmp, &path)?;
    Ok(Some(if existing.is_some() { Outcome::Updated } else { Outcome::Created }))
}

/// Removes our block from one file; a file left with nothing (or only front matter we wrote) is deleted.
pub fn remove_one(root: &Path, t: &Target) -> Result<bool> {
    let path = root.join(t.path);
    if !stays_inside(root, &path) {
        return Ok(false);
    }
    let Ok(text) = fs::read_to_string(&path) else { return Ok(false) };
    if !text.contains("<!-- automemory:begin") {
        return Ok(false);
    }
    let rest = strip(&text);
    if rest.trim().is_empty() || (!t.front.is_empty() && rest.trim() == t.front.trim()) {
        fs::remove_file(&path)?;
    } else {
        fs::write(&path, rest)?;
    }
    Ok(true)
}

/// Which targets to write: the named agent, all of them, or those the repository already uses.
fn chosen(root: &Path, agent: Option<&str>, all: bool) -> Result<Vec<&'static Target>> {
    if let Some(a) = agent {
        let t: Vec<_> = TARGETS.iter().filter(|t| t.agent == a || (a == "codex" && t.agent == "agents")).collect();
        if t.is_empty() {
            anyhow::bail!("no instruction file known for '{a}'. Known: {}", TARGETS.iter().map(|t| t.agent).collect::<Vec<_>>().join(", "));
        }
        return Ok(t);
    }
    Ok(TARGETS.iter().filter(|t| all || t.agent == "agents" || root.join(t.detect).exists() || root.join(t.path).exists()).collect())
}

pub(crate) fn repo_root(cwd: &Path) -> PathBuf {
    std::process::Command::new("git")
        .arg("-C").arg(cwd).args(["rev-parse", "--show-toplevel"])
        .output().ok()
        .filter(|o| o.status.success())
        .map(|o| PathBuf::from(String::from_utf8_lossy(&o.stdout).trim()))
        .unwrap_or_else(|| cwd.to_path_buf())
}

/// What `export-rules` does for this project, as data for the dashboard. `write` applies it.
pub fn plan(db: &Database, root: &Path, project: Option<&str>, write: bool) -> serde_json::Value {
    let view = automemory_core::resume::build_rules_view(db, project);
    let mut files = Vec::new();
    for t in chosen(root, None, false).unwrap_or_default() {
        let old = fs::read_to_string(root.join(t.path)).ok();
        let new = splice(old.as_deref().unwrap_or(""), t.front, &view);
        let status = match &old {
            None => "created",
            Some(o) if *o == new => "unchanged",
            Some(_) => "updated",
        };
        if write && status != "unchanged" {
            let _ = write_one(root, t, &view, true);
        }
        files.push(serde_json::json!({ "path": t.path, "agent": t.agent, "status": status }));
    }
    serde_json::json!({ "applied": write, "files": files, "block": block(&view) })
}

pub fn execute(agent: Option<String>, all: bool, check: bool) -> Result<()> {
    let db = Database::open(&automemory_core::config::Config::default_config().db_path)?;
    let cwd = std::env::current_dir()?;
    let root = repo_root(&cwd);
    let project = super::continuity::project_for(&db, &root);
    let view = automemory_core::resume::build_rules_view(&db, project.as_deref());
    let mut stale = 0;
    for t in chosen(&root, agent.as_deref(), all)? {
        if check {
            let old = fs::read_to_string(root.join(t.path)).unwrap_or_default();
            if old.contains("<!-- automemory:begin") && splice(&old, t.front, &view) != old {
                println!("  ✗ out of date: {}", t.path);
                stale += 1;
            }
            continue;
        }
        match write_one(&root, t, &view, true)? {
            Some(Outcome::Created) => println!("  ✓ created   {}", t.path),
            Some(Outcome::Updated) => println!("  ✓ updated   {}", t.path),
            _ => println!("  · unchanged {}", t.path),
        }
    }
    if check {
        anyhow::ensure!(stale == 0, "{stale} instruction file(s) out of date: run `automemory export-rules`");
        println!("Instruction files are current.");
    }
    Ok(())
}

/// Called from hooks: refreshes files that already carry our block, never creates new ones.
pub fn refresh_existing(db: &Database, project_id: Option<&str>, cwd: &Path) {
    let root = repo_root(cwd);
    let view = automemory_core::resume::build_rules_view(db, project_id);
    for t in TARGETS {
        let _ = write_one(&root, t, &view, false);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[cfg(unix)]
    #[test]
    fn a_symlinked_instruction_file_is_never_read_or_written_through() {
        let dir = std::env::temp_dir().join(format!("am-symlink-{}", std::process::id()));
        let (repo, outside) = (dir.join("repo"), dir.join("outside"));
        fs::create_dir_all(&repo).unwrap();
        fs::create_dir_all(&outside).unwrap();
        fs::write(outside.join("secret"), "TOP SECRET").unwrap();
        std::os::unix::fs::symlink(outside.join("secret"), repo.join("AGENTS.md")).unwrap();
        std::os::unix::fs::symlink(&outside, repo.join(".cursor")).unwrap();
        let agents = TARGETS.iter().find(|t| t.agent == "agents").unwrap();
        let cursor = TARGETS.iter().find(|t| t.agent == "cursor").unwrap();
        assert!(write_one(&repo, agents, "- x", true).unwrap().is_none());
        assert!(write_one(&repo, cursor, "- x", true).unwrap().is_none());
        assert_eq!(fs::read_to_string(outside.join("secret")).unwrap(), "TOP SECRET");
        assert!(!outside.join("rules").exists());
        assert!(fs::symlink_metadata(repo.join("AGENTS.md")).unwrap().file_type().is_symlink(), "the link is left alone");
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn splice_appends_replaces_and_strips_without_touching_user_text() {
        let user = "# My rules\n\nUse tabs.\n";
        let a = splice(user, "", "- one");
        assert!(a.starts_with(user.trim_end()) && a.contains("- one"));
        let b = splice(&a, "", "- two");
        assert!(b.contains("- two") && !b.contains("- one"), "{b}");
        assert_eq!(b.matches("automemory:begin").count(), 1);
        assert_eq!(splice(&b, "", "- two"), b, "idempotent");
        assert_eq!(strip(&b), user);
        // Text after the block survives a replace.
        let c = format!("{a}\n\nTrailing user note.\n");
        let d = splice(&c, "", "- three");
        assert!(d.contains("Trailing user note.") && d.contains("- three") && !d.contains("- one"), "{d}");
        assert_eq!(strip("only ours").as_str(), "only ours");
    }

    #[test]
    fn dedicated_files_get_front_matter_and_are_deleted_on_remove() {
        let dir = std::env::temp_dir().join(format!("am-rules-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        let cursor = TARGETS.iter().find(|t| t.agent == "cursor").unwrap();
        assert_eq!(write_one(&dir, cursor, "- x", false).unwrap(), None, "refresh never creates");
        assert_eq!(write_one(&dir, cursor, "- x", true).unwrap(), Some(Outcome::Created));
        let text = fs::read_to_string(dir.join(cursor.path)).unwrap();
        assert!(text.starts_with("---\n") && text.contains("alwaysApply: true") && text.contains("- x"));
        assert_eq!(write_one(&dir, cursor, "- x", true).unwrap(), Some(Outcome::Unchanged));
        assert!(remove_one(&dir, cursor).unwrap());
        assert!(!dir.join(cursor.path).exists());
        let _ = fs::remove_dir_all(&dir);
    }
}
