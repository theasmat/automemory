//! Picking up where you left off: a short resume pack for a new session (same machine or another)
//! and the hand-off baton written when a session ends.

use crate::baton::build_packet;
use crate::db::Database;
use crate::models::{Baton, DecisionStatus, MemoryStatus};
use chrono::{DateTime, Utc};
use std::path::Path;
use std::process::Command;
use uuid::Uuid;

/// The pack stays small enough to read at a glance and to cost few tokens.
const RESUME_CHAR_BUDGET: usize = 1800;
const HANDOFF_CHAR_BUDGET: usize = 2200;

/// A readable name for this machine, stored once so it stays stable.
pub fn device_label(db: &Database) -> String {
    if let Some(l) = db.sync_kv_get("device_label") {
        return l;
    }
    let label = std::env::var("AUTOMEMORY_DEVICE_NAME")
        .ok()
        .filter(|s| !s.trim().is_empty())
        .or_else(|| {
            Command::new("hostname")
                .output()
                .ok()
                .filter(|o| o.status.success())
                .map(|o| String::from_utf8_lossy(&o.stdout).trim().to_string())
                .filter(|s| !s.is_empty())
        })
        .unwrap_or_else(|| "this machine".to_string());
    db.sync_kv_set("device_label", &label);
    label
}

fn git(dir: &Path, args: &[&str]) -> Option<String> {
    Command::new("git")
        .arg("-C")
        .arg(dir)
        .args(args)
        .output()
        .ok()
        .filter(|o| o.status.success())
        .map(|o| String::from_utf8_lossy(&o.stdout).trim().to_string())
}

/// `branch feat/x at abc1234, 3 file(s) uncommitted, 2 commit(s) not pushed`.
pub fn git_state(dir: &Path) -> Option<String> {
    let branch = git(dir, &["rev-parse", "--abbrev-ref", "HEAD"])?;
    let sha = git(dir, &["rev-parse", "--short", "HEAD"]).unwrap_or_default();
    let dirty: Vec<String> = git(dir, &["status", "--porcelain"])
        .unwrap_or_default()
        .lines()
        // `git()` trims the output, so the first line may have lost its leading status space.
        .map(|l| l.trim_start().split_once(' ').map_or(l, |(_, path)| path).trim().to_string())
        .collect();
    let unpushed = git(dir, &["rev-list", "--count", "@{u}..HEAD"]).and_then(|n| n.parse::<usize>().ok());
    let mut s = format!("branch {branch} at {sha}");
    if !dirty.is_empty() {
        let shown: Vec<&str> = dirty.iter().take(6).map(String::as_str).collect();
        s.push_str(&format!(
            ", {} file(s) uncommitted ({}{})",
            dirty.len(),
            shown.join(", "),
            if dirty.len() > 6 { ", …" } else { "" }
        ));
    }
    if let Some(n) = unpushed.filter(|n| *n > 0) {
        s.push_str(&format!(", {n} commit(s) not pushed"));
    }
    if !dirty.is_empty() || unpushed.is_some_and(|n| n > 0) {
        s.push_str(". Code travels by git, not AutoMemory: commit and push (or stash) before switching machines");
    }
    Some(s)
}

fn ago(when: DateTime<Utc>) -> String {
    let mins = (Utc::now() - when).num_minutes().max(0);
    match mins {
        0..=1 => "just now".to_string(),
        2..=59 => format!("{mins} min ago"),
        60..=1439 => format!("{} h ago", mins / 60),
        _ => format!("{} d ago", mins / 1440),
    }
}

fn clip(s: &str, max: usize) -> String {
    if s.chars().count() <= max {
        return s.to_string();
    }
    let mut out: String = s.chars().take(max).collect();
    out.push('…');
    out
}

/// The hand-off written when a session ends: the baton facts from the ledger plus where the
/// repository stands and which machine this was.
pub fn build_handoff(
    db: &Database,
    project_id: Option<&str>,
    session_id: &str,
    agent: &str,
    cwd: &Path,
    note: Option<&str>,
) -> anyhow::Result<Baton> {
    let mut packet = build_packet(db, project_id, session_id, agent, note)?;
    packet.push_str(&format!("\nDevice: {}", device_label(db)));
    if let Some(state) = git_state(cwd) {
        packet.push_str(&format!("\nRepository: {state}"));
    }
    packet = clip(&packet, HANDOFF_CHAR_BUDGET);
    let baton = Baton {
        id: Uuid::now_v7().to_string(),
        project_id: project_id.map(str::to_string),
        from_session: session_id.to_string(),
        to_session: None,
        agent: agent.to_string(),
        packet,
        created_at: Utc::now(),
    };
    db.save_baton(&baton)?;
    Ok(baton)
}

/// What a new session should know first, or `None` when there is nothing worth saying.
pub fn build_resume(db: &Database, project_id: Option<&str>, session_id: &str, cwd: &Path) -> Option<String> {
    let mut lines = Vec::new();

    // The latest hand-off from any other session, on any machine.
    if let Ok(Some(b)) = db.latest_baton(project_id, None) {
        if b.from_session != session_id {
            let head: Vec<&str> = b.packet.lines().take(14).collect();
            lines.push(format!("Last hand-off ({}, {}):\n  {}", b.agent, ago(b.created_at), head.join("\n  ")));
        }
    }

    // The newest session checkpoint from another session.
    let checkpoint = db
        .list_memories(project_id, Some(MemoryStatus::Active), None, 200)
        .unwrap_or_default()
        .into_iter()
        .filter(|m| m.summary.starts_with("Session checkpoint"))
        .filter(|m| m.metadata.get("session").and_then(|s| s.as_str()) != Some(session_id))
        .max_by_key(|m| m.updated_at);
    if let Some(m) = checkpoint {
        let device = m.metadata.get("device").and_then(|d| d.as_str()).unwrap_or("another session");
        lines.push(format!("Last session ({device}, {}): {}", ago(m.updated_at), clip(&m.content, 300)));
    }

    // Decisions in force.
    let decisions = db.list_decisions(project_id, Some(DecisionStatus::Active), 3).unwrap_or_default();
    if !decisions.is_empty() {
        let items: Vec<String> = decisions
            .iter()
            .map(|d| format!("{}: {}", d.topic, clip(&d.decision, 100)))
            .collect();
        lines.push(format!("Decisions in force: {}", items.join("; ")));
    }

    if lines.is_empty() {
        return None;
    }
    if let Some(state) = git_state(cwd) {
        lines.push(format!("Now: {state}"));
    }
    Some(clip(&format!("Resuming this project.\n{}", lines.join("\n")), RESUME_CHAR_BUDGET))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn repo(dir: &Path) {
        let _ = std::fs::remove_dir_all(dir);
        std::fs::create_dir_all(dir).unwrap();
        let run = |a: &[&str]| {
            assert!(Command::new("git")
                .args(["-c", "user.name=t", "-c", "user.email=t@t"])
                .args(a)
                .current_dir(dir)
                .output()
                .unwrap()
                .status
                .success());
        };
        run(&["init", "-q", "-b", "main"]);
        std::fs::write(dir.join("a.rs"), "x").unwrap();
        run(&["add", "-A"]);
        run(&["commit", "-qm", "init"]);
    }

    #[test]
    fn handoff_records_the_repository_state_and_resume_shows_it_to_the_next_session() {
        let dir = std::env::temp_dir().join(format!("am-resume-{}", std::process::id()));
        repo(&dir);
        std::fs::write(dir.join("a.rs"), "changed").unwrap(); // uncommitted work
        let db = Database::open_in_memory().unwrap();

        // Nothing to resume on a fresh database.
        assert!(build_resume(&db, None, "s2", &dir).is_none());

        let baton = build_handoff(&db, None, "s1", "claude", &dir, Some("finish the checkout form")).unwrap();
        assert!(baton.packet.contains("finish the checkout form"));
        assert!(baton.packet.contains("Repository: branch main at "), "{}", baton.packet);
        assert!(baton.packet.contains("1 file(s) uncommitted (a.rs)"), "{}", baton.packet);
        assert!(baton.packet.contains("commit and push (or stash)"));
        assert!(baton.packet.contains("Device: "));

        let pack = build_resume(&db, None, "s2", &dir).expect("a new session sees the hand-off");
        assert!(pack.contains("Last hand-off (claude, just now)") && pack.contains("finish the checkout form"), "{pack}");
        assert!(pack.contains("Now: branch main"));
        assert!(pack.len() <= RESUME_CHAR_BUDGET + 4);
        // The session that wrote it is not told about itself.
        assert!(build_resume(&db, None, "s1", &dir).is_none_or(|p| !p.contains("Last hand-off")));
        let _ = std::fs::remove_dir_all(&dir);
    }
}

