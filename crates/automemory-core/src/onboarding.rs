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
}
