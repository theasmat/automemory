use crate::db::Database;
use crate::models::{Baton, DecisionStatus};
use anyhow::Result;
use chrono::Utc;
use uuid::Uuid;

/// Hard cap: PRD handoff packets stay under 400 tokens (~4 chars/token).
pub const BATON_CHAR_BUDGET: usize = 1600;

/// Builds a handoff packet from ledger facts (exit codes, files, leases,
/// decisions), never from raw agent reasoning.
pub fn build_packet(
    db: &Database,
    project_id: Option<&str>,
    from_session: &str,
    agent: &str,
    note: Option<&str>,
) -> Result<String> {
    let mut lines = vec![format!("baton from {from_session} ({agent})")];

    // Recent session activity: counts + distinct files + last outcomes.
    let events = db.get_session_events(from_session).unwrap_or_default();
    let recent: Vec<_> = events.iter().rev().take(12).collect();
    if !recent.is_empty() {
        let failed = recent
            .iter()
            .filter(|e| {
                e.tool.as_ref().is_some_and(|t| {
                    t.status != "success" || t.exit_code.is_some_and(|c| c != 0)
                })
            })
            .count();
        let mut files: Vec<String> = Vec::new();
        for e in recent.iter().rev() {
            if let Some(tool) = e.tool.as_ref().and_then(|t| t.input.as_ref()) {
                for f in event_files(tool) {
                    if !files.contains(&f) {
                        files.push(f);
                    }
                }
            }
        }
        lines.push(format!(
            "did: {} tool calls, {} failed",
            recent.len(),
            failed
        ));
        if !files.is_empty() {
            lines.push(format!("files: {}", files.iter().take(8).cloned().collect::<Vec<_>>().join(", ")));
        }
        let mut outcomes: Vec<String> = recent
            .iter()
            .take(3)
            .map(|e| match &e.tool {
                Some(t) => format!("{}:{}", short_tool(&t.name), t.status),
                None => e.event_type.clone(),
            })
            .collect();
        outcomes.reverse();
        lines.push(format!("last: {}", outcomes.join(" | ")));
    }

    // Leases still held by this session transfer awareness, not ownership.
    let held: Vec<String> = db
        .list_active_leases(project_id)
        .unwrap_or_default()
        .into_iter()
        .filter(|l| l.session_id == from_session)
        .map(|l| format!("{} ({})", l.scope, l.goal))
        .collect();
    if !held.is_empty() {
        lines.push(format!("holding: {}", held.join("; ")));
    }

    // Active decisions and unresolved blockers ground the next agent.
    let decisions = db
        .list_decisions(project_id, Some(DecisionStatus::Active), 3)
        .unwrap_or_default();
    for d in decisions {
        lines.push(format!("decision[{}]: {}", d.topic, d.decision));
    }
    let blockers = db.list_failures(project_id, Some("unresolved"), 3).unwrap_or_default();
    for f in blockers {
        lines.push(format!("blocker: {}", f.task_title));
    }

    if let Some(n) = note.filter(|n| !n.trim().is_empty()) {
        lines.push(format!("next: {}", n.trim()));
    }

    let mut packet = lines.join("\n");
    if packet.len() > BATON_CHAR_BUDGET {
        let mut idx = BATON_CHAR_BUDGET;
        while !packet.is_char_boundary(idx) {
            idx = idx.saturating_sub(1);
        }
        packet.truncate(idx);
        packet.push_str("\n…[truncated]");
    }
    Ok(packet)
}

pub fn pass_baton(
    db: &Database,
    project_id: Option<&str>,
    from_session: &str,
    to_session: Option<&str>,
    agent: &str,
    note: Option<&str>,
) -> Result<Baton> {
    let packet = build_packet(db, project_id, from_session, agent, note)?;
    let baton = Baton {
        id: Uuid::now_v7().to_string(),
        project_id: project_id.map(str::to_string),
        from_session: from_session.to_string(),
        to_session: to_session.map(str::to_string),
        agent: agent.to_string(),
        packet,
        created_at: Utc::now(),
    };
    db.save_baton(&baton)?;
    Ok(baton)
}

/// File paths touched by one tool input. Mirrors the hook's target
/// extraction keys; kept local so core gains no CLI dependency.
fn event_files(input: &serde_json::Value) -> Vec<String> {
    let mut out = Vec::new();
    for key in [
        "file_path",
        "filePath",
        "path",
        "file",
        "TargetFile",
        "AbsolutePath",
        "relative_path",
        "filename",
    ] {
        if let Some(v) = input.get(key) {
            if let Some(s) = v.as_str() {
                if !s.is_empty() {
                    out.push(s.to_string());
                }
            } else if let Some(arr) = v.as_array() {
                for item in arr.iter().filter_map(|i| i.as_str()).take(3) {
                    out.push(item.to_string());
                }
            }
        }
    }
    out
}

fn short_tool(name: &str) -> &str {
    match name {
        "read" | "read_file" | "view_file" => "read",
        "write" | "write_to_file" | "create_file" => "write",
        "edit" | "edit_file" | "replace_file_content" => "edit",
        "command" | "run_command" => "cmd",
        other => other,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::Database;
    use crate::models::FailureRecord;

    #[test]
    fn packet_caps_at_budget_and_carries_note() {
        let db = Database::open_in_memory().unwrap();
        let packet = build_packet(
            &db,
            None,
            "sess-a",
            "claude-code",
            Some(&"x".repeat(5000)),
        )
        .unwrap();
        assert!(packet.len() <= BATON_CHAR_BUDGET + 20);
        assert!(packet.contains("baton from sess-a"));
        assert!(packet.contains("next:"));
    }

    #[test]
    fn packet_grounds_in_decisions_and_blockers() {
        let db = Database::open_in_memory().unwrap();
        db.record_decision(crate::models::DecisionRecordArgs {
            project_id: None,
            topic: "auth",
            decision: "JWT in cookie",
            reason: "rotation",
            alternatives: &[],
            dissent: None,
            agent: "human",
            symbols: &[],
            evidence: &[],
        })
        .unwrap();
        db.save_failure(&FailureRecord {
            id: "f1".to_string(),
            project_id: None,
            task_title: "flaky redis test".to_string(),
            category: None,
            failed_tool: None,
            error_message: "timeout".to_string(),
            root_cause_hypothesis: None,
            fix_applied: None,
            verification_result: None,
            status: "unresolved".to_string(),
            evidence_event_ids: vec![],
            created_at: Utc::now(),
            updated_at: Utc::now(),
        })
        .unwrap();
        let packet = build_packet(&db, None, "sess-a", "agent", Some("finish auth")).unwrap();
        assert!(packet.contains("decision[auth]: JWT in cookie"));
        assert!(packet.contains("blocker: flaky redis test"));
        assert!(packet.contains("next: finish auth"));
    }
}
