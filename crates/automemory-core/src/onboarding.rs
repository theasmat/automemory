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

impl Default for OnboardingState {
    fn default() -> Self {
        Self {
            completed_steps: Vec::new(),
            dismissed: false,
            dismissed_flags: HashMap::new(),
            demo_loaded: false,
            tour_step: 0,
        }
    }
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

/// Seeds a sample project (3 decisions, 5 notes, 1 pending suggestion) tagged with `metadata.demo = true`.
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

    // 3. Five notes
    let notes = [
        (
            MemoryType::Fact,
            "Frontend runs Nuxt 3 with Tailwind CSS and Vite",
            "Nuxt 3 stack with Tailwind and Vite",
        ),
        (
            MemoryType::Preference,
            "Code style prefers functional Rust and descriptive errors with Try: hints",
            "Functional Rust and Try: error hints",
        ),
        (
            MemoryType::Lesson,
            "Always run git status before editing files in clients/web",
            "Git status check before web edits",
        ),
        (
            MemoryType::Pattern,
            "Database operations use transaction-isolated queries",
            "Transaction-isolated database queries",
        ),
        (
            MemoryType::Warning,
            "Never commit plaintext API tokens to source control",
            "Secret redaction and no plaintext tokens",
        ),
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

    // 4. One pending suggestion in review_proposals
    let prop = ReviewProposal {
        id: format!("demo-prop-{}", Uuid::new_v4().simple()),
        project_id: Some("demo".to_string()),
        proposal_type: "pattern_promotion".to_string(),
        target_id: "demo-target-1".to_string(),
        description: "Promote frequent build command pattern to shared project skill".to_string(),
        suggested_action: "approve".to_string(),
        status: "pending".to_string(),
        created_at: Utc::now(),
        decided_at: None,
    };
    db.save_proposal(&prop)?;

    set_demo_loaded(db, true)?;
    Ok(())
}

/// Removes all demo rows (memories, decisions, proposals, project). Leaves zero demo rows.
pub fn remove_demo(db: &Database) -> Result<()> {
    let conn = db.conn();
    conn.execute(
        "DELETE FROM memories WHERE project_id = 'demo' OR json_extract(metadata, '$.demo') = 1 OR json_extract(metadata, '$.demo') = true",
        [],
    )?;
    conn.execute("DELETE FROM decisions WHERE project_id = 'demo'", [])?;
    conn.execute("DELETE FROM review_proposals WHERE project_id = 'demo'", [])?;
    conn.execute("DELETE FROM project_paths WHERE project_id = 'demo'", [])?;
    conn.execute("DELETE FROM projects WHERE id = 'demo'", [])?;
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

        // Verify rows exist: 3 decisions (each creates a decision memory) + 5 notes = 8 memories
        let (active_memories, _) = db.count_memories(Some("demo")).unwrap();
        assert_eq!(active_memories, 8);
        let decisions = db.list_decisions(Some("demo"), None, 10).unwrap();
        assert_eq!(decisions.len(), 3);
        let proposals = db.list_pending_proposals(Some("demo")).unwrap();
        assert_eq!(proposals.len(), 1);

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
    }
}
