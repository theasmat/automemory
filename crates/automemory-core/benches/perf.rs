//! PRD-gated performance benches. Run with `cargo bench -p automemory-core`.
//! Numbers quoted in docs/features.md (benchmarks row) must come from here.

use automemory_core::db::Database;
use automemory_core::extraction::TrajectoryDistiller;
use automemory_core::hooks::{
    CanonicalEvent, GuardrailHandler, HooksConfig, LockCheckHandler, PipelineCtx, merge_decision,
    run_pipeline,
};
use automemory_core::models::*;
use automemory_core::ports::{HandlerEnv, HookHandler};
use chrono::Utc;
use criterion::{Criterion, criterion_group, criterion_main};
use uuid::Uuid;

fn seed_memories(db: &Database, n: usize) {
    for i in 0..n {
        let mem = Memory {
            id: Uuid::now_v7().to_string(),
            scope: MemoryScope::Project,
            project_id: Some("bench".to_string()),
            memory_type: MemoryType::Fact,
            content: format!("Bench memory {i} about auth session handling with tokens"),
            summary: format!("bench fact {i}"),
            confidence: 0.9,
            importance: 0.7,
            freshness: 1.0,
            status: MemoryStatus::Active,
            source_kind: MemorySourceKind::Agent,
            version: Some(1),
            created_at: Utc::now(),
            updated_at: Utc::now(),
            last_recalled_at: None,
            tags: vec!["auth".to_string()],
            entities: vec![],
            metadata: serde_json::json!({}),
        };
        db.save_memory(&mem).unwrap();
    }
}

fn bench_hook_pretool_allow(c: &mut Criterion) {
    let db = Database::open_in_memory().unwrap();
    let cfg = HooksConfig::with_defaults();
    let args = serde_json::json!({"file_path": "src/auth/session.ts"});
    let paths = vec!["src/auth/session.ts".to_string()];
    let symbols: Vec<String> = vec![];
    let ctx = PipelineCtx {
        event: CanonicalEvent::PreToolUse,
        agent: "bench",
        session_id: "bench-session",
        tool: "read",
        args: &args,
        paths: &paths,
        symbols: &symbols,
        is_mutation: false,
    };
    let env = HandlerEnv {
        db: &db,
        project_id: Some("bench"),
        mode: AutopilotMode::Vibe,
    };
    let lock_check = LockCheckHandler;
    let guardrail = GuardrailHandler;
    let handlers: Vec<&dyn HookHandler> = vec![&lock_check, &guardrail];
    c.bench_function("hook_pretool_allow", |b| {
        b.iter(|| {
            let results = run_pipeline(&cfg.handler, &ctx, &env, &handlers);
            let _ = merge_decision(&results);
        })
    });
}

fn bench_fts_recall_500(c: &mut Criterion) {
    let db = Database::open_in_memory().unwrap();
    seed_memories(&db, 500);
    c.bench_function("fts_recall_500", |b| {
        b.iter(|| {
            let _ = db.search_memories_fts("auth session tokens", Some("bench"), 5);
        })
    });
}

fn bench_distill_session(c: &mut Criterion) {
    let db = Database::open_in_memory().unwrap();
    for i in 0..40 {
        let ev = NormalizedEvent {
            event_id: Uuid::now_v7().to_string(),
            event_type: "tool.completed".to_string(),
            timestamp: Utc::now(),
            agent: AgentInfo {
                adapter_id: "bench".to_string(),
                agent_instance_id: None,
            },
            session_id: "bench-session".to_string(),
            task_id: None,
            project: ProjectRef {
                id: Some("bench".to_string()),
                slug: None,
                root_path: String::new(),
                source_git_remote: None,
            },
            tool: Some(ToolInfo {
                name: "read".to_string(),
                category: "file_read".to_string(),
                input: Some(serde_json::json!({"file_path": format!("src/file{i}.ts")})),
                input_hash: None,
                output: None,
                output_hash: None,
                status: "success".to_string(),
                exit_code: Some(0),
                duration_ms: Some(5),
            }),
            artifacts: vec![],
            sensitivity: "public".to_string(),
            schema_version: 1,
            raw_payload: None,
        };
        db.append_event(&ev).unwrap();
    }
    let distiller = TrajectoryDistiller::new(&db);
    c.bench_function("distill_session_40_events", |b| {
        b.iter(|| {
            let _ = distiller.distill_session("bench-session");
        })
    });
}

criterion_group!(benches, bench_hook_pretool_allow, bench_fts_recall_500, bench_distill_session);
criterion_main!(benches);
