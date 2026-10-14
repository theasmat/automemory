use crate::db::Database;
use crate::hooks::{merge_decision, run_pipeline, CanonicalEvent, HooksConfig, PipelineCtx};
use crate::models::{
    LeaseAcquireArgs, LeaseAcquireResult, LeaseScopeType, Memory, MemoryScope, MemorySourceKind,
    MemoryStatus, MemoryType,
};
use crate::ports::HandlerEnv;
use crate::retrieval::MemoryDecayScorer;
use crate::verify::{DecayVerifier, Verifier, VerifyStatus};
use chrono::{Duration, Utc};
use serde::{Deserialize, Serialize};
use std::time::Instant;
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EvalCheck {
    pub name: String,
    pub passed: bool,
    pub detail: String,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct EvalReport {
    pub checks: Vec<EvalCheck>,
}

impl EvalReport {
    pub fn passed(&self) -> bool {
        !self.checks.is_empty() && self.checks.iter().all(|c| c.passed)
    }

    pub fn summary(&self) -> String {
        let mut out = vec![format!(
            "eval: {}/{} suites passed",
            self.checks.iter().filter(|c| c.passed).count(),
            self.checks.len()
        )];
        for c in &self.checks {
            out.push(format!(
                "  {} {:22} {}",
                if c.passed { "ok  " } else { "FAIL" },
                c.name,
                c.detail
            ));
        }
        out.join("\n")
    }
}

/// Local deterministic eval suites on an isolated in-memory DB.
/// ponytail: task-execution A/B (memory on/off with headless agents) deferred
/// until the Runner exists; this measures what is measurable locally and
/// gates the Planning Room per PRD §13/§14.
pub fn run_local_suites() -> EvalReport {
    let db = Database::open_in_memory().expect("eval db");
    let mut checks = vec![
        stale_flag_precision(&db),
        recall_precision_at_3(&db),
        contradiction_detection(),
        lease_collision_race(&db),
        hook_pipeline_latency(),
    ];
    // Keep output stable for `--json` diffing across runs.
    checks.sort_by(|a, b| a.name.cmp(&b.name));
    EvalReport { checks }
}

fn seed_memory(
    db: &Database,
    summary: &str,
    content: &str,
    updated_days_ago: i64,
    confidence: f64,
) {
    let at = Utc::now() - Duration::days(updated_days_ago);
    let mem = Memory {
        id: Uuid::now_v7().to_string(),
        scope: MemoryScope::Project,
        project_id: None,
        memory_type: MemoryType::Fact,
        content: content.to_string(),
        summary: summary.to_string(),
        confidence,
        importance: 0.8,
        freshness: 1.0,
        status: MemoryStatus::Active,
        source_kind: MemorySourceKind::Agent,
        version: Some(1),
        created_at: at,
        updated_at: at,
        last_recalled_at: None,
        tags: vec![],
        entities: vec![],
        metadata: serde_json::json!({}),
    };
    db.save_memory_direct(&mem).expect("seed");
}

/// PRD §3 gate: stale-flag precision ≥ 90%.
fn stale_flag_precision(db: &Database) -> EvalCheck {
    for i in 0..8 {
        seed_memory(db, &format!("fresh fact {i}"), "current stack fact", 0, 0.9);
    }
    for i in 0..4 {
        seed_memory(db, &format!("old fact {i}"), "legacy stack fact", 120, 0.5);
    }
    let verifier = DecayVerifier::default();
    let all = db
        .list_memories(None, Some(MemoryStatus::Active), None, 50)
        .unwrap_or_default();
    let (mut flagged, mut true_stale) = (0, 0);
    for m in &all {
        if verifier.verify(m) == VerifyStatus::Stale {
            flagged += 1;
            true_stale += ((Utc::now() - m.updated_at).num_days() > 60) as usize;
        }
    }
    let precision = if flagged == 0 {
        0.0
    } else {
        true_stale as f64 / flagged as f64
    };
    EvalCheck {
        name: "stale-flag precision".to_string(),
        passed: precision >= 0.9 && flagged == 4,
        detail: format!("precision {true_stale}/{flagged}"),
    }
}

fn recall_precision_at_3(db: &Database) -> EvalCheck {
    seed_memory(
        db,
        "auth uses JWT",
        "Authentication uses JWT in cookies",
        0,
        0.9,
    );
    seed_memory(db, "auth expiry", "JWT expiry is 30 minutes", 0, 0.9);
    seed_memory(db, "pool size", "DB pool max is 25", 0, 0.9);
    seed_memory(db, "retry policy", "Backoff is exponential", 0, 0.9);
    let hits = db
        .search_memories_fts("JWT auth", None, 3)
        .unwrap_or_default();
    let relevant = hits
        .iter()
        .filter(|(m, _)| m.summary.contains("auth") || m.summary.contains("JWT"))
        .count();
    let precision = if hits.is_empty() {
        0.0
    } else {
        relevant as f64 / hits.len() as f64
    };
    let top_ok = hits
        .first()
        .is_some_and(|(m, _)| m.summary.contains("auth") || m.summary.contains("JWT"));
    EvalCheck {
        name: "recall precision@3".to_string(),
        passed: top_ok && precision >= 0.66,
        detail: format!("precision {relevant}/{}", hits.len()),
    }
}

fn contradiction_detection() -> EvalCheck {
    let scorer = MemoryDecayScorer::default();
    let (hit, _) = scorer.detect_contradiction(
        "We use Vue 2 Options API",
        &["package.json: \"vue\": \"^3.5\""],
    );
    let (miss, _) = scorer.detect_contradiction(
        "We use JWT in cookies",
        &["package.json: \"vue\": \"^3.5\""],
    );
    EvalCheck {
        name: "contradiction detect".to_string(),
        passed: hit && !miss,
        detail: format!("vue2-vs-vue3 flagged={hit}, unrelated clean={}", !miss),
    }
}

/// PRD G2: zero silent overwrites — holder must win, waiter must conflict.
fn lease_collision_race(db: &Database) -> EvalCheck {
    let acquire = |agent: &str, session: &str| {
        db.acquire_lease(LeaseAcquireArgs {
            project_id: None,
            agent,
            session_id: session,
            scope: "src/auth/session.ts",
            scope_type: LeaseScopeType::File,
            goal: "refactor",
            ttl_seconds: 300,
        })
    };
    let first = matches!(acquire("a", "s-a"), Ok(LeaseAcquireResult::Acquired { .. }));
    let conflict = matches!(acquire("b", "s-b"), Ok(LeaseAcquireResult::Conflict { .. }));
    let _ = db.release_leases("s-a", None);
    let after = matches!(acquire("b", "s-b"), Ok(LeaseAcquireResult::Acquired { .. }));
    EvalCheck {
        name: "lease collision race".to_string(),
        passed: first && conflict && after,
        detail: format!("acquire={first} conflict={conflict} reacquire={after}"),
    }
}

/// PRD §3 gate: hook p95 latency under 5 ms.
fn hook_pipeline_latency() -> EvalCheck {
    let specs = HooksConfig::with_defaults();
    let empty_args = serde_json::json!({});
    let empty_paths: [String; 0] = [];
    let empty_symbols: [String; 0] = [];
    let ctx = PipelineCtx {
        event: CanonicalEvent::PreToolUse,
        agent: "eval",
        session_id: "eval",
        tool: "read",
        args: &empty_args,
        paths: &empty_paths,
        symbols: &empty_symbols,
        is_mutation: false,
    };
    let db = Database::open_in_memory().expect("eval db");
    let env = HandlerEnv {
        db: &db,
        project_id: None,
        mode: crate::models::AutopilotMode::Vibe,
    };
    let mut elapsed: Vec<u128> = (0..200)
        .map(|_| {
            let start = Instant::now();
            let results = run_pipeline(&specs.handler, &ctx, &env, &[]);
            let _ = merge_decision(&results);
            start.elapsed().as_micros()
        })
        .collect();
    elapsed.sort_unstable();
    let p95_ms = elapsed[190] as f64 / 1000.0;
    EvalCheck {
        name: "hook p95 latency".to_string(),
        passed: p95_ms < 5.0,
        detail: format!("p95 {p95_ms:.3}ms over 200 runs"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn local_suites_all_pass() {
        let report = run_local_suites();
        assert!(report.passed(), "{}", report.summary());
        assert_eq!(report.checks.len(), 5);
    }
}
