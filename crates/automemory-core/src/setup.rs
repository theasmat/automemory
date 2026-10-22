//! Getting started checklist and setup verification.
//!
//! Provides one single source of truth for "what is set up and what is next" across
//! the CLI (`automemory next`, `automemory doctor`), the REST API (`GET /api/setup`),
//! and the dashboard UI.

use crate::db::Database;
use crate::onboarding;
use serde::{Deserialize, Serialize};
use std::path::Path;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct Step {
    pub id: String,
    pub title: String,
    pub done: bool,
    pub why: String,
    pub command: String,
    pub link: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct LearnTopic {
    pub id: &'static str,
    pub title: &'static str,
    pub summary: &'static str,
    pub content: &'static str,
}

pub const LEARN_TOPICS: &[LearnTopic] = &[
    LearnTopic {
        id: "memory",
        title: "Memory & Preferences",
        summary: "Store shared facts, preferences, lessons, and patterns across agents",
        content: "AutoMemory shares verified facts, preferences, patterns, and warnings across agents.\nMemories can be global or scoped to a project. Pinned memories act as invariants.\n\nSave a memory or preference:\n  automemory note \"We use pnpm workspaces across all packages\"\n  automemory note \"Production API uses port 8080\" -t preference\n\nList and search saved memories:\n  automemory memory list\n  automemory memory search \"database\"\n\nArchive an outdated memory:\n  automemory memory archive <id>",
    },
    LearnTopic {
        id: "decisions",
        title: "Architectural Decisions",
        summary: "Record binding architectural choices with reasons and alternatives",
        content: "Architectural decisions are immutable records with rationale, alternatives, and dissent.\nActive decisions outrank notes and are always presented to agents first.\n\nRecord an architectural decision:\n  automemory decision record --topic \"Storage\" --decision \"SQLite WAL\" --reason \"Fast local ACID\"\n\nList recorded decisions:\n  automemory decision list\n  automemory decision get <id>\n\nDecisions are automatically exported into your project's AGENTS.md instruction file.",
    },
    LearnTopic {
        id: "locks",
        title: "Advisory File Locks",
        summary: "Prevent concurrent agents from overwriting the same files",
        content: "Advisory file and symbol locks prevent concurrent agents from overwriting the same code.\nWhen agent A edits a file, an automatic lease is held; agent B gets an immediate warning or deny.\n\nInspect active locks and recent lock coverage:\n  automemory locks\n  automemory locks --coverage\n\nManually release or override a lock:\n  automemory unlock src/main.rs\n  automemory unlock --force src/main.rs",
    },
    LearnTopic {
        id: "handoff",
        title: "Session Continuity & Handoffs",
        summary: "Pass context packets between sessions under a 400-token budget",
        content: "Session continuity ensures a new agent session starts exactly where the last one stopped.\nBaton packets pass git status, recent edits, open decisions, and optional notes under 400 tokens.\n\nLeave a handoff for the next session or agent:\n  automemory handoff \"Finished auth API refactor; tests pass\"\n\nResume from the last session's handoff packet:\n  automemory resume\n\nAgents automatically receive the resume packet on their first prompt.",
    },
    LearnTopic {
        id: "sync",
        title: "Encrypted Multi-Machine Sync",
        summary: "Synchronize memory vaults via git with client-side AES-256 encryption",
        content: "Sync multi-machine memory through a private, encrypted git repository (your memory vault).\nAll data is client-side encrypted with AES-256-GCM before leaving this machine.\n\nSet up your private sync remote repository:\n  automemory sync remote https://github.com/your-org/memory-vault.git\n\nTrigger an immediate background sync cycle:\n  automemory sync auto\n\nPair a second machine using your recovery phrase:\n  automemory sync pair",
    },
    LearnTopic {
        id: "modes",
        title: "Autopilot Safety Modes",
        summary: "Control autonomous agent authority: vibe, review, or strict",
        content: "AutoMemory operates in three distinct autopilot guardrail modes:\n  - vibe:   Autonomous development; non-blocking warnings on risk.\n  - review: High-risk mutations (rm -rf, git push -f) generate review proposals.\n  - strict: High-risk operations are denied; strict lease collision blocking.\n\nView or change the current autopilot mode:\n  automemory mode\n  automemory mode set review\n  automemory mode set strict",
    },
    LearnTopic {
        id: "export-rules",
        title: "Instruction Files (AGENTS.md)",
        summary: "Keep AGENTS.md, CLAUDE.md and .cursorrules up to date automatically",
        content: "Instruction files let every AI tool (Claude, Codex, Cursor, Gemini) start informed.\nAutoMemory writes a marked block into AGENTS.md, CLAUDE.md, and .cursorrules without tool calls.\n\nUpdate all detected instruction files:\n  automemory export-rules\n\nVerify if instruction files are up to date:\n  automemory export-rules --check\n\nOnly the <!-- automemory:begin --> block is touched; user text is preserved.",
    },
    LearnTopic {
        id: "embeddings",
        title: "Hybrid Retrieval & Embeddings",
        summary: "Combine fast FTS5 keyword matching with vector semantic search",
        content: "AutoMemory supports hybrid search combining SQLite FTS5 BM25 keyword matching with vectors.\nChoose between fast offline keyword recall or semantic embeddings (local, Ollama, OpenAI).\n\nCheck current embedding engine status:\n  automemory embeddings status\n\nReindex vector embeddings for all project memories:\n  automemory embeddings reindex\n\nConfigure embedding provider:\n  automemory embeddings set-provider local",
    },
];

/// Compute the getting started checklist for the current environment.
pub fn checklist(db: &Database, home: &Path, cwd: &Path) -> Vec<Step> {
    let ob = onboarding::load(db);

    let agent_done = has_connected_agent(home, cwd, &ob);
    let rules_done = has_instruction_files(cwd, &ob);
    let passwd_done = crate::auth::stored_password_hash().is_some() || ob.is_done("password");
    let sync_done = crate::sync::auto::vault_settings().1.is_some() || ob.is_done("sync");

    let moment_connected = ob.is_done("connected") || agent_done;
    let moment_remembered = ob.is_done("remembered") || has_active_memories(db);
    let moment_decided = ob.is_done("decided") || has_active_decisions(db);
    let moment_locked = ob.is_done("saw_lock_block") || ob.is_done("locked") || ob.is_done("protected") || has_lock_block(db);
    let moment_reviewed = ob.is_done("reviewed_suggestion") || ob.is_done("reviewed") || has_reviewed_proposals(db);

    vec![
        Step {
            id: "agent".to_string(),
            title: "Agent connected".to_string(),
            done: agent_done,
            why: if agent_done {
                "An agent is connected and registered."
            } else {
                "No agent connected yet."
            }
            .to_string(),
            command: "automemory connect claude".to_string(),
            link: "/agents".to_string(),
        },
        Step {
            id: "instruction_files".to_string(),
            title: "Instruction files".to_string(),
            done: rules_done,
            why: "Write verified memory to AGENTS.md so agents start informed without tool calls."
                .to_string(),
            command: "automemory export-rules".to_string(),
            link: "/brain".to_string(),
        },
        Step {
            id: "password".to_string(),
            title: "Password set".to_string(),
            done: passwd_done,
            why: "Protect safety guardrails from unauthorized agent modification.".to_string(),
            command: "automemory rule passwd".to_string(),
            link: "/safety".to_string(),
        },
        Step {
            id: "sync".to_string(),
            title: "Sync remote set".to_string(),
            done: sync_done,
            why: "Synchronize memory and decisions across multiple machines.".to_string(),
            command: "automemory sync remote <url>".to_string(),
            link: "/sync".to_string(),
        },
        Step {
            id: "connected".to_string(),
            title: "Connected".to_string(),
            done: moment_connected,
            why: "Confirm your agent is registered and communicating with AutoMemory.".to_string(),
            command: "automemory doctor".to_string(),
            link: "/agents".to_string(),
        },
        Step {
            id: "remembered".to_string(),
            title: "Remembered a note".to_string(),
            done: moment_remembered,
            why: "I told it something once (note) and agents share it.".to_string(),
            command: "automemory note \"We use pnpm\"".to_string(),
            link: "/brain".to_string(),
        },
        Step {
            id: "decided".to_string(),
            title: "Recorded a decision".to_string(),
            done: moment_decided,
            why: "Architectural decisions outrank notes and are shown first.".to_string(),
            command: "automemory decision record --topic Storage --decision \"SQLite WAL\"".to_string(),
            link: "/brain".to_string(),
        },
        Step {
            id: "saw_lock_block".to_string(),
            title: "Protected by lock".to_string(),
            done: moment_locked,
            why: "Two agents wanted the same file and one was stopped with a clear reason.".to_string(),
            command: "automemory lock list".to_string(),
            link: "/agents".to_string(),
        },
        Step {
            id: "reviewed_suggestion".to_string(),
            title: "Reviewed a suggestion".to_string(),
            done: moment_reviewed,
            why: "I reviewed a suggestion and know where mode, password and sync live.".to_string(),
            command: "automemory review".to_string(),
            link: "/inbox".to_string(),
        },
    ]
}

/// Returns the first undone step in the checklist, if any.
pub fn next_step<'a>(steps: &'a [Step]) -> Option<&'a Step> {
    steps.iter().find(|s| !s.done)
}

fn has_connected_agent(home: &Path, cwd: &Path, ob: &onboarding::OnboardingState) -> bool {
    if ob.is_done("agent") || ob.is_done("agent_connected") {
        return true;
    }
    // Check cwd
    let cwd_files = [
        ".agents/hooks.json",
        ".agents/mcp_config.json",
        ".mcp.json",
        ".cursorrules",
        ".cursor/rules/automemory.mdc",
        ".clinerules/automemory.md",
        ".windsurf/rules/automemory.md",
        ".roo/rules/automemory.md",
        ".claude/settings.local.json",
        ".codex/hooks.json",
    ];
    for f in cwd_files {
        let p = cwd.join(f);
        if p.exists() {
            if let Ok(content) = std::fs::read_to_string(&p) {
                if content.contains("automemory") || content.contains("HOOK_MARKER") {
                    return true;
                }
            }
        }
    }
    // Check home
    let home_files = [
        ".claude/settings.json",
        ".claude.json",
        ".codex/hooks.json",
        ".gemini/config/hooks.json",
        ".gemini/config/mcp_config.json",
        ".config/zed/settings.json",
        ".continue/config.json",
        ".config/goose/config.yaml",
        ".aider.conf.yml",
    ];
    for f in home_files {
        let p = home.join(f);
        if p.exists() {
            if let Ok(content) = std::fs::read_to_string(&p) {
                if content.contains("automemory") {
                    return true;
                }
            }
        }
    }
    if home.join(".gemini/config/plugins/automemory").exists()
        || home.join(".gemini/config/skills/automemory").exists()
        || home.join(".gemini/antigravity-cli/skills/automemory").exists()
    {
        return true;
    }
    false
}

fn has_instruction_files(cwd: &Path, ob: &onboarding::OnboardingState) -> bool {
    if ob.is_done("instruction_files") {
        return true;
    }
    const TARGETS: &[&str] = &[
        "AGENTS.md",
        "CLAUDE.md",
        "GEMINI.md",
        ".github/copilot-instructions.md",
        ".cursor/rules/automemory.mdc",
        ".windsurf/rules/automemory.md",
        ".clinerules/automemory.md",
        ".roo/rules/automemory.md",
        ".kiro/steering/automemory.md",
    ];
    for t in TARGETS {
        let p = cwd.join(t);
        if p.exists() {
            if let Ok(content) = std::fs::read_to_string(&p) {
                if content.contains("<!-- automemory:begin") {
                    return true;
                }
            }
        }
    }
    false
}

fn has_active_memories(db: &Database) -> bool {
    let conn = db.conn();
    let count: i64 = conn
        .query_row(
            "SELECT COUNT(*) FROM memories WHERE status = 'active' AND (metadata IS NULL OR (json_extract(metadata, '$.demo') IS NOT 1 AND json_extract(metadata, '$.demo') IS NOT true))",
            [],
            |r| r.get(0),
        )
        .unwrap_or(0);
    count > 0
}

fn has_active_decisions(db: &Database) -> bool {
    let conn = db.conn();
    let count: i64 = conn
        .query_row(
            "SELECT COUNT(*) FROM decisions WHERE status = 'active' AND (project_id IS NULL OR project_id != 'demo')",
            [],
            |r| r.get(0),
        )
        .unwrap_or(0);
    count > 0
}

/// A tool call was stopped because another agent held the file (recorded in the audit log).
fn has_lock_block(db: &Database) -> bool {
    db.conn()
        .query_row(
            "SELECT COUNT(*) FROM audit_log WHERE action = 'hook.deny' AND details LIKE '%locked by agent%'",
            [],
            |r| r.get::<_, i64>(0),
        )
        .unwrap_or(0)
        > 0
}

fn has_reviewed_proposals(db: &Database) -> bool {
    let conn = db.conn();
    let prop_count: i64 = conn
        .query_row(
            "SELECT COUNT(*) FROM review_proposals WHERE status != 'pending' AND (project_id IS NULL OR project_id != 'demo')",
            [],
            |r| r.get(0),
        )
        .unwrap_or(0);
    prop_count > 0
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::*;
    use chrono::Utc;
    use uuid::Uuid;

    #[test]
    fn test_checklist_and_next_step_in_memory() {
        let db = Database::open_in_memory().expect("open_in_memory failed");
        let test_id = Uuid::new_v4().simple().to_string();
        let home = std::env::temp_dir().join(format!("am-setup-test-home-{test_id}"));
        let cwd = std::env::temp_dir().join(format!("am-setup-test-cwd-{test_id}"));
        let _ = std::fs::create_dir_all(&home);
        let _ = std::fs::create_dir_all(&cwd);

        // 1. Initial state: nothing done
        let steps = checklist(&db, &home, &cwd);
        assert_eq!(steps.len(), 9);
        assert!(!steps[0].done); // agent
        let next = next_step(&steps).expect("expected next step");
        assert_eq!(next.id, "agent");
        assert_eq!(next.why, "No agent connected yet.");

        // 2. Mark agent done via onboarding
        onboarding::mark_done(&db, "agent").unwrap();
        let steps = checklist(&db, &home, &cwd);
        assert!(steps[0].done);
        let next = next_step(&steps).expect("expected next step");
        assert_eq!(next.id, "instruction_files");

        // 3. Create instruction file with automemory:begin marker
        std::fs::write(
            cwd.join("AGENTS.md"),
            "<!-- automemory:begin -->\nShared memory\n<!-- automemory:end -->",
        )
        .unwrap();
        let steps = checklist(&db, &home, &cwd);
        assert!(steps[1].done); // instruction_files is done

        // 4. Save a memory
        let mem = Memory {
            id: Uuid::now_v7().to_string(),
            scope: MemoryScope::Global,
            project_id: None,
            memory_type: MemoryType::Fact,
            content: "We use pnpm".to_string(),
            summary: "pnpm preference".to_string(),
            confidence: 1.0,
            importance: 0.8,
            freshness: 1.0,
            status: MemoryStatus::Active,
            source_kind: MemorySourceKind::User,
            version: Some(1),
            created_at: Utc::now(),
            updated_at: Utc::now(),
            last_recalled_at: None,
            tags: vec![],
            entities: vec![],
            metadata: serde_json::json!({}),
        };
        db.save_memory(&mem).unwrap();
        let steps = checklist(&db, &home, &cwd);
        let remembered_step = steps.iter().find(|s| s.id == "remembered").unwrap();
        assert!(remembered_step.done);

        // 5. Record a decision
        db.record_decision(DecisionRecordArgs {
            topic: "Storage",
            decision: "SQLite WAL",
            reason: "Fast local-first ACID",
            project_id: None,
            agent: "human",
            dissent: None,
            alternatives: &[],
            symbols: &[],
            evidence: &[],
        })
        .unwrap();
        let steps = checklist(&db, &home, &cwd);
        let decided_step = steps.iter().find(|s| s.id == "decided").unwrap();
        assert!(decided_step.done);

        let _ = std::fs::remove_dir_all(&home);
        let _ = std::fs::remove_dir_all(&cwd);
    }
}
