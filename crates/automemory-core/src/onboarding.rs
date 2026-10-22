//! Onboarding progress tracking.
//!
//! Stores user/dashboard onboarding progress in the `sync_kv` table under key `onboarding`.
//! Shared across CLI and daemon so state is synchronized with zero staleness.

use crate::db::Database;
use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

pub const ONBOARDING_KV_KEY: &str = "onboarding";

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[derive(Default)]
pub struct OnboardingState {
    #[serde(default)]
    pub completed_steps: Vec<String>,
    #[serde(default)]
    pub dismissed: bool,
    #[serde(default)]
    pub dismissed_flags: HashMap<String, bool>,
    #[serde(default)]
    pub demo_loaded: bool,
    #[serde(default)]
    pub tour_step: usize,
}


impl OnboardingState {
    pub fn is_done(&self, step_id: &str) -> bool {
        self.completed_steps.iter().any(|s| s == step_id)
    }

    pub fn is_dismissed(&self, flag: &str) -> bool {
        self.dismissed || *self.dismissed_flags.get(flag).unwrap_or(&false)
    }
}

/// Load the current onboarding state from `sync_kv`. Returns default if not present or corrupt.
pub fn load(db: &Database) -> OnboardingState {
    db.sync_kv_get(ONBOARDING_KV_KEY)
        .and_then(|raw| serde_json::from_str(&raw).ok())
        .unwrap_or_default()
}

/// Save onboarding state to `sync_kv`.
pub fn save(db: &Database, state: &OnboardingState) -> Result<()> {
    let json = serde_json::to_string(state)?;
    db.sync_kv_set(ONBOARDING_KV_KEY, &json);
    Ok(())
}

/// Mark a step as completed in `onboarding` state.
pub fn mark_done(db: &Database, step_id: &str) -> Result<OnboardingState> {
    let mut state = load(db);
    if !state.is_done(step_id) {
        state.completed_steps.push(step_id.to_string());
        save(db, &state)?;
    }
    Ok(state)
}

/// Dismiss the onboarding tour or a specific flag.
pub fn dismiss(db: &Database, flag: Option<&str>) -> Result<OnboardingState> {
    let mut state = load(db);
    if let Some(f) = flag {
        state.dismissed_flags.insert(f.to_string(), true);
    }
    state.dismissed = true;
    save(db, &state)?;
    Ok(state)
}

/// Reset onboarding progress to default.
pub fn reset(db: &Database) -> Result<OnboardingState> {
    let state = OnboardingState::default();
    save(db, &state)?;
    Ok(state)
}

/// Update demo_loaded flag.
pub fn set_demo_loaded(db: &Database, loaded: bool) -> Result<OnboardingState> {
    let mut state = load(db);
    state.demo_loaded = loaded;
    save(db, &state)?;
    Ok(state)
}

/// Update current tour step.
pub fn set_tour_step(db: &Database, step: usize) -> Result<OnboardingState> {
    let mut state = load(db);
    state.tour_step = step;
    save(db, &state)?;
    Ok(state)
}

/// Seeds a sample project that fills every page: 3 decisions, 8 notes, 3 suggestions, 3 agents with
/// activity and 2 file locks, a hand-off, a worked trace, 2 problems (1 fixed) and a learned skill, tagged with `metadata.demo = true`.
pub fn seed_demo(db: &Database) -> Result<()> {
    use crate::models::*;
    use chrono::Utc;
    use uuid::Uuid;

    // 1. Project
    let proj = Project {
        id: "demo".to_string(),
        slug: "demo".to_string(),
        display_name: "Demo Project (E-Commerce)".to_string(),
        created_at: Utc::now(),
        updated_at: Utc::now(),
        capture_policy: CapturePolicy::default(),
        context_budget: ContextBudget::default(),
        document_config: DocumentConfig::default(),
        sync_config: SyncConfig::default(),
    };
    let _ = db.save_project(&proj);

    // 2. Three decisions
    let decisions = [
        (
            "Database Engine",
            "SQLite with WAL mode",
            "Local-first ACID compliance and zero external infrastructure",
            vec!["PostgreSQL".to_string(), "DuckDB".to_string()],
        ),
        (
            "Package Manager",
            "Use pnpm for all web clients",
            "Fast content-addressable storage and monorepo workspace support",
            vec!["npm".to_string(), "yarn".to_string()],
        ),
        (
            "Agent Safety Gating",
            "Human password required for guardrail mutations",
            "Prevent autonomous privilege escalation across tools",
            vec!["Token file".to_string(), "Keyring only".to_string()],
        ),
    ];
    for (topic, decision, reason, alts) in decisions {
        db.record_decision(DecisionRecordArgs {
            project_id: Some("demo"),
            topic,
            decision,
            reason,
            alternatives: &alts,
            dissent: None,
            agent: "demo",
            symbols: &[],
            evidence: &[],
        })?;
    }

    // 3. Eight notes covering every kind
    let notes = [
        (MemoryType::Fact, "Frontend runs Nuxt 3 with Tailwind CSS and Vite", "Nuxt 3 stack with Tailwind and Vite"),
        (MemoryType::Fact, "Payments go through Stripe webhooks; the handler lives in src/webhook.rs", "Stripe webhooks handle payments"),
        (MemoryType::Preference, "Code style prefers functional Rust and descriptive errors with Try: hints", "Functional Rust and Try: error hints"),
        (MemoryType::Preference, "Commit messages are imperative and under 72 characters", "Short imperative commit messages"),
        (MemoryType::Lesson, "Always run git status before editing files in clients/web", "Git status check before web edits"),
        (MemoryType::Pattern, "Database operations use transaction-isolated queries", "Transaction-isolated database queries"),
        (MemoryType::Warning, "Never commit plaintext API tokens to source control", "Secret redaction and no plaintext tokens"),
        (MemoryType::Warning, "The checkout form breaks if the cart total is rounded before tax", "Round the cart total after tax"),
    ];
    for (mtype, content, summary) in notes {
        let mem = Memory {
            id: format!("demo-mem-{}", Uuid::new_v4().simple()),
            scope: MemoryScope::Project,
            project_id: Some("demo".to_string()),
            memory_type: mtype,
            content: content.to_string(),
            summary: summary.to_string(),
            confidence: 1.0,
            importance: 0.7,
            freshness: 1.0,
            status: MemoryStatus::Active,
            source_kind: MemorySourceKind::User,
            version: Some(1),
            created_at: Utc::now(),
            updated_at: Utc::now(),
            last_recalled_at: None,
            tags: vec!["demo".to_string()],
            entities: vec![],
            metadata: serde_json::json!({"demo": true}),
        };
        db.save_memory(&mem)?;
    }

    // 4. Three pending suggestions (what the Inbox is for)
    let proposals = [
        ("pattern_promotion", "demo-target-1", "Promote the frequent build command to a shared project skill"),
        ("memory_promotion", "demo-target-2", "Remember: integration tests need STRIPE_TEST_KEY set"),
        ("preference_promotion", "demo-target-3", "Prefer small pull requests (under 300 changed lines)"),
    ];
    for (kind, target, description) in proposals {
        let prop = ReviewProposal {
            id: format!("demo-prop-{}", Uuid::new_v4().simple()),
            project_id: Some("demo".to_string()),
            proposal_type: kind.to_string(),
            target_id: target.to_string(),
            description: description.to_string(),
            suggested_action: "approve".to_string(),
            status: "pending".to_string(),
            created_at: Utc::now(),
            decided_at: None,
        };
        db.save_proposal(&prop)?;
    }

    // 5. Agents at work: sessions, a stream of events (Home activity, Session history)
    let now = Utc::now();
    let work = [
        ("claude", "demo-session-claude", "src/checkout.rs", 14),
        ("cursor", "demo-session-cursor", "src/ui/cart.tsx", 9),
        ("codex", "demo-session-codex", "tests/payments.rs", 5),
    ];
    for (agent, session, file, minutes_ago) in work {
        let started = now - chrono::Duration::minutes(minutes_ago);
        db.conn().execute(
            "INSERT OR IGNORE INTO sessions (id, project_id, agent_adapter, agent_instance_id, created_at, updated_at, status) VALUES (?1, 'demo', ?2, NULL, ?3, ?4, 'active')",
            rusqlite::params![session, agent, started.to_rfc3339(), now.to_rfc3339()],
        )?;
        for (i, (kind, tool, category)) in [
            ("session.start", None, ""),
            ("tool.completed", Some("Read"), "file_read"),
            ("tool.completed", Some("Edit"), "file_write"),
            ("tool.completed", Some("Bash"), "command"),
        ]
        .into_iter()
        .enumerate()
        {
            let ev = NormalizedEvent {
                event_id: format!("demo-ev-{agent}-{i}-{}", Uuid::new_v4().simple()),
                event_type: kind.to_string(),
                timestamp: started + chrono::Duration::minutes(i as i64),
                agent: AgentInfo { adapter_id: agent.to_string(), agent_instance_id: None },
                session_id: session.to_string(),
                task_id: None,
                project: ProjectRef { id: Some("demo".to_string()), slug: Some("demo".to_string()), root_path: "/demo/shop".to_string(), source_git_remote: None },
                tool: tool.map(|name| ToolInfo {
                    name: name.to_string(),
                    category: category.to_string(),
                    input: Some(serde_json::json!({ "file_path": file })),
                    input_hash: None,
                    output: None,
                    output_hash: None,
                    status: "success".to_string(),
                    exit_code: Some(0),
                    duration_ms: Some(120),
                }),
                artifacts: vec![],
                sensitivity: "normal".to_string(),
                schema_version: 1,
                raw_payload: None,
            };
            db.append_event(&ev)?;
        }
    }

    // 6. File locks held right now (Agents & locks)
    for (agent, session, file, goal) in [
        ("claude", "demo-session-claude", "src/checkout.rs", "Add the Stripe checkout flow"),
        ("cursor", "demo-session-cursor", "src/ui/cart.tsx", "Show the cart total after tax"),
    ] {
        let _ = db.acquire_lease(LeaseAcquireArgs {
            project_id: Some("demo"),
            agent,
            session_id: session,
            scope: file,
            scope_type: LeaseScopeType::File,
            goal,
            ttl_seconds: 1800,
        });
    }

    // 7. A hand-off note, what worked, problems and fixes, a learned skill
    db.save_baton_direct(&Baton {
        id: format!("demo-baton-{}", Uuid::new_v4().simple()),
        project_id: Some("demo".to_string()),
        from_session: "demo-session-claude".to_string(),
        to_session: None,
        agent: "claude".to_string(),
        packet: "Stripe checkout is half done: the webhook handler exists, the success page does not.\nNext: add src/pages/success.tsx and a test for a failed payment.\nRepository: branch feat/checkout, 2 file(s) uncommitted.".to_string(),
        created_at: now - chrono::Duration::minutes(3),
    })?;

    db.save_trace(&SuccessTrace {
        id: format!("demo-trace-{}", Uuid::new_v4().simple()),
        project_id: Some("demo".to_string()),
        title: "Run the payments tests against Stripe test mode".to_string(),
        category: "testing".to_string(),
        status: "active".to_string(),
        preconditions: vec!["STRIPE_TEST_KEY is exported".to_string()],
        steps: vec![
            TraceStep { position: 1, action: "Start the local webhook forwarder".to_string(), tool: "Bash".to_string(), command: Some("stripe listen --forward-to localhost:3000/webhook".to_string()), verification: Some("Ready message appears".to_string()) },
            TraceStep { position: 2, action: "Run the payments test suite".to_string(), tool: "Bash".to_string(), command: Some("cargo test payments".to_string()), verification: Some("All tests pass".to_string()) },
        ],
        reproducibility_score: 0.95,
        caveats: vec!["Needs network access".to_string()],
        source_task_id: None,
        evidence_event_ids: vec![],
        created_at: now - chrono::Duration::hours(2),
        updated_at: now - chrono::Duration::hours(2),
    })?;

    db.save_failure(&FailureRecord {
        id: format!("demo-fail-{}", Uuid::new_v4().simple()),
        project_id: Some("demo".to_string()),
        task_title: "Checkout total was off by one cent".to_string(),
        category: Some("logic".to_string()),
        failed_tool: Some("cargo test".to_string()),
        error_message: "assertion failed: left == 1999, right == 2000".to_string(),
        root_cause_hypothesis: Some("The total was rounded before tax was added".to_string()),
        fix_applied: Some("Round once, after tax".to_string()),
        verification_result: Some("cargo test checkout passes".to_string()),
        status: "resolved".to_string(),
        evidence_event_ids: vec![],
        created_at: now - chrono::Duration::hours(5),
        updated_at: now - chrono::Duration::hours(4),
    })?;
    db.save_failure(&FailureRecord {
        id: format!("demo-fail-{}", Uuid::new_v4().simple()),
        project_id: Some("demo".to_string()),
        task_title: "Webhook signature check fails locally".to_string(),
        category: Some("config".to_string()),
        failed_tool: Some("curl".to_string()),
        error_message: "No signatures found matching the expected signature".to_string(),
        root_cause_hypothesis: Some("The forwarder prints a different signing secret each run".to_string()),
        fix_applied: None,
        verification_result: None,
        status: "unresolved".to_string(),
        evidence_event_ids: vec![],
        created_at: now - chrono::Duration::minutes(30),
        updated_at: now - chrono::Duration::minutes(30),
    })?;

    db.save_skill(&ProceduralSkill {
        id: format!("demo-skill-{}", Uuid::new_v4().simple()),
        project_id: Some("demo".to_string()),
        name: "Test Stripe webhooks locally".to_string(),
        description: "Forward Stripe test events to the dev server and run the payment tests".to_string(),
        trigger_patterns: vec!["stripe".to_string(), "webhook".to_string()],
        preconditions: vec!["STRIPE_TEST_KEY is exported".to_string()],
        steps: vec!["stripe listen --forward-to localhost:3000/webhook".to_string(), "cargo test payments".to_string()],
        caveats: vec!["Copy the new signing secret each run".to_string()],
        success_count: 4,
        failure_count: 1,
        source_trace_ids: vec![],
        created_at: now - chrono::Duration::hours(1),
        updated_at: now - chrono::Duration::hours(1),
    })?;

    set_demo_loaded(db, true)?;
    Ok(())
}

/// Removes all demo rows (memories, decisions, proposals, project). Leaves zero demo rows.
pub fn remove_demo(db: &Database) -> Result<()> {
    let conn = db.conn();
    for sql in [
        "DELETE FROM memories WHERE project_id = 'demo' OR json_extract(metadata, '$.demo') = 1 OR json_extract(metadata, '$.demo') = true",
        "DELETE FROM decisions WHERE project_id = 'demo'",
        "DELETE FROM review_proposals WHERE project_id = 'demo'",
        "DELETE FROM tool_events WHERE project_id = 'demo'",
        "DELETE FROM sessions WHERE project_id = 'demo'",
        "DELETE FROM leases WHERE project_id = 'demo'",
        "DELETE FROM batons WHERE project_id = 'demo'",
        "DELETE FROM success_traces WHERE project_id = 'demo'",
        "DELETE FROM failure_records WHERE project_id = 'demo'",
        "DELETE FROM procedural_skills WHERE project_id = 'demo'",
        "DELETE FROM project_paths WHERE project_id = 'demo'",
        "DELETE FROM projects WHERE id = 'demo'",
    ] {
        conn.execute(sql, [])?;
    }
    drop(conn);

    set_demo_loaded(db, false)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_onboarding_lifecycle() {
        let db = Database::open_in_memory().expect("open_in_memory failed");

        // 1. Initial state is default
        let initial = load(&db);
        assert_eq!(initial, OnboardingState::default());
        assert!(!initial.is_done("agent"));
        assert!(!initial.is_dismissed("welcome_card"));

        // 2. Mark step done
        let updated = mark_done(&db, "agent").expect("mark_done failed");
        assert!(updated.is_done("agent"));
        assert!(!updated.is_done("sync"));

        // Loading again retains it
        let loaded = load(&db);
        assert_eq!(loaded, updated);
        assert!(loaded.is_done("agent"));

        // 3. Dismiss specific flag
        let dismissed = dismiss(&db, Some("welcome_card")).expect("dismiss failed");
        assert!(dismissed.dismissed);
        assert!(dismissed.is_dismissed("welcome_card"));

        // 4. Tour step & demo loaded
        let step_updated = set_tour_step(&db, 3).expect("set_tour_step failed");
        assert_eq!(step_updated.tour_step, 3);

        let demo_updated = set_demo_loaded(&db, true).expect("set_demo_loaded failed");
        assert!(demo_updated.demo_loaded);

        // 5. Reset restores default
        let reset_state = reset(&db).expect("reset failed");
        assert_eq!(reset_state, OnboardingState::default());
        let reloaded = load(&db);
        assert_eq!(reloaded, OnboardingState::default());
    }

    #[test]
    fn test_seed_and_remove_demo() {
        let db = Database::open_in_memory().expect("open_in_memory failed");

        // Seed demo
        seed_demo(&db).expect("seed_demo failed");
        let ob = load(&db);
        assert!(ob.demo_loaded);

        // Verify rows exist: 3 decisions (each creates a decision memory) + 8 notes = 11 memories
        let (active_memories, _) = db.count_memories(Some("demo")).unwrap();
        assert_eq!(active_memories, 11);
        let decisions = db.list_decisions(Some("demo"), None, 10).unwrap();
        assert_eq!(decisions.len(), 3);
        let proposals = db.list_pending_proposals(Some("demo")).unwrap();
        assert_eq!(proposals.len(), 3);

        // Every page has something to show
        let count = |table: &str| -> i64 {
            db.conn().query_row(&format!("SELECT COUNT(*) FROM {table} WHERE project_id = 'demo'"), [], |r| r.get(0)).unwrap()
        };
        for (table, min) in [("tool_events", 12), ("sessions", 3), ("leases", 2), ("batons", 1), ("success_traces", 1), ("failure_records", 2), ("procedural_skills", 1)] {
            assert!(count(table) >= min, "{table} should have at least {min} demo rows");
        }

        // Remove demo
        remove_demo(&db).expect("remove_demo failed");
        let ob = load(&db);
        assert!(!ob.demo_loaded);

        // Verify zero demo rows remain
        let (active_memories, _) = db.count_memories(Some("demo")).unwrap();
        assert_eq!(active_memories, 0);
        let decisions = db.list_decisions(Some("demo"), None, 10).unwrap();
        assert_eq!(decisions.len(), 0);
        let proposals = db.list_pending_proposals(Some("demo")).unwrap();
        assert_eq!(proposals.len(), 0);
        for table in ["tool_events", "sessions", "leases", "batons", "success_traces", "failure_records", "procedural_skills"] {
            assert_eq!(count(table), 0, "{table} should be empty after removal");
        }
    }
}
