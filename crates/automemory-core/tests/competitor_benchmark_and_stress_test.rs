use automemory_core::db::Database;
use automemory_core::extraction::TrajectoryDistiller;
use automemory_core::graph::{GraphBuilder, GraphEdge, GraphEntity};
use automemory_core::models::*;
use automemory_core::redaction::redact_text;
use automemory_core::retrieval::RetrievalEngine;
use chrono::Utc;
use serde_json::json;
use std::time::Instant;

#[test]
fn test_benchmark_ingestion_and_fts5_retrieval_speed() {
    let db = Database::open_in_memory().expect("in-memory db open failed");
    let project_id = "bench-proj-01";

    // Register project
    let project = Project {
        id: project_id.to_string(),
        slug: "bench-proj".to_string(),
        display_name: "Benchmark Project".to_string(),
        created_at: Utc::now(),
        updated_at: Utc::now(),
        capture_policy: CapturePolicy::default(),
        context_budget: ContextBudget::default(),
        document_config: DocumentConfig::default(),
        sync_config: SyncConfig::default(),
    };
    db.save_project(&project).expect("save project failed");

    // 1. Stress Ingest 500 Memories
    let start_mem_ingest = Instant::now();
    for i in 0..500 {
        let mem = Memory {
            id: format!("mem-bench-{}", i),
            scope: if i % 4 == 0 { MemoryScope::Global } else { MemoryScope::Project },
            project_id: if i % 4 == 0 { None } else { Some(project_id.to_string()) },
            memory_type: match i % 5 {
                0 => MemoryType::Decision,
                1 => MemoryType::Pattern,
                2 => MemoryType::Lesson,
                3 => MemoryType::Fact,
                _ => MemoryType::Warning,
            },
            content: format!("Benchmark item {}: Database connection pool configured with timeout of {} seconds for PostgreSQL node cluster.", i, (i % 30) + 5),
            summary: format!("Pool timeout {}s (idx {})", (i % 30) + 5, i),
            confidence: 0.85 + (i as f64 % 15.0) / 100.0,
            importance: 0.8,
            freshness: 1.0,
            status: MemoryStatus::Active,
            source_kind: MemorySourceKind::Agent,
            version: Some(1),
            created_at: Utc::now(),
            updated_at: Utc::now(),
            last_recalled_at: None,
            tags: vec!["database".to_string(), "postgres".to_string(), format!("node-{}", i % 5)],
            entities: vec!["postgres".to_string()],
            metadata: json!({ "batch_id": i / 50 }),
        };
        db.save_memory(&mem).expect("save_memory failed");
    }
    let mem_ingest_duration = start_mem_ingest.elapsed();
    println!(
        "⏱ [Benchmark] 500 Memories saved & FTS5 indexed in: {:?}",
        mem_ingest_duration
    );
    assert!(
        mem_ingest_duration.as_millis() < 2500,
        "500 memory saves should complete under 2500ms"
    );

    // 2. FTS5 BM25 Sub-Millisecond Search Benchmark
    let start_query = Instant::now();
    let search_iterations = 200;
    for _ in 0..search_iterations {
        let results = db
            .search_memories_fts("PostgreSQL timeout cluster", Some(project_id), 5)
            .expect("fts query failed");
        assert!(!results.is_empty());
    }
    let query_duration = start_query.elapsed();
    let avg_query_us = query_duration.as_micros() / search_iterations;
    println!(
        "⏱ [Benchmark] 200 FTS5 BM25 searches completed in: {:?} (Avg: {} µs/query)",
        query_duration, avg_query_us
    );
    assert!(
        avg_query_us < 20000,
        "Average SQLite FTS5 search query in debug mode must be fast (was {} µs)",
        avg_query_us
    );

    // 3. Stress Ingest 1,000 Raw Tool Events
    let start_event_ingest = Instant::now();
    for i in 0..1000 {
        let ev = NormalizedEvent {
            event_id: format!("ev-bench-{}", i),
            event_type: "tool_execution".to_string(),
            timestamp: Utc::now(),
            agent: AgentInfo {
                adapter_id: "antigravity".to_string(),
                agent_instance_id: Some("worker-1".to_string()),
            },
            session_id: format!("session-{}", i / 50),
            task_id: Some(format!("task-{}", i / 10)),
            project: ProjectRef {
                id: Some(project_id.to_string()),
                slug: Some("bench-proj".to_string()),
                root_path: "/dev/bench".to_string(),
                source_git_remote: None,
            },
            tool: Some(ToolInfo {
                name: if i % 3 == 0 {
                    "run_command".to_string()
                } else if i % 3 == 1 {
                    "replace_file_content".to_string()
                } else {
                    "view_file".to_string()
                },
                category: "code".to_string(),
                input: Some(json!({ "path": format!("src/module_{}.rs", i % 20), "line": i })),
                input_hash: Some(format!("hash_{}", i)),
                output: Some(json!({ "stdout": "Compilation succeeded with 0 warnings" })),
                output_hash: Some(format!("out_hash_{}", i)),
                status: if i % 25 == 0 {
                    "error".to_string()
                } else {
                    "success".to_string()
                },
                exit_code: if i % 25 == 0 { Some(1) } else { Some(0) },
                duration_ms: Some((i % 100) as u64 + 10),
            }),
            artifacts: Vec::new(),
            sensitivity: "internal".to_string(),
            schema_version: 1,
            raw_payload: None,
        };
        db.append_event(&ev).expect("append_event failed");
    }
    let event_ingest_duration = start_event_ingest.elapsed();
    println!(
        "⏱ [Benchmark] 1,000 Tool Events appended to ledger in: {:?}",
        event_ingest_duration
    );
    assert!(
        event_ingest_duration.as_millis() < 3000,
        "1,000 event appends must complete under 3000ms"
    );
}

#[test]
fn test_trajectory_distillation_100_event_noise_pruning() {
    let db = Database::open_in_memory().expect("in-memory db open failed");
    let distiller = TrajectoryDistiller::new(&db);
    let session_id = "session-massive-trajectory-100";

    // Simulate 100 agent tool calls:
    // 30 view_files (reads), 20 directory scans, 10 ripgreps, 5 failed compiler runs, 5 fixed edits, 10 tests, 20 git status checks
    for i in 0..100 {
        let (tool_name, status, is_error, output_msg) = if i < 30 {
            ("view_file", "success", false, "file content lines 1-100")
        } else if i < 50 {
            (
                "list_dir",
                "success",
                false,
                "directory listing [a.rs, b.rs, c.rs]",
            )
        } else if i < 60 {
            ("grep_search", "success", false, "3 matches found")
        } else if i < 65 {
            (
                "run_command",
                "error",
                true,
                "error[E0382]: use of moved value: `buffer`",
            )
        } else if i < 70 {
            (
                "replace_file_content",
                "success",
                false,
                "replaced buffer clone with Arc::clone",
            )
        } else if i < 80 {
            (
                "run_command",
                "success",
                false,
                "test result: ok. 15 passed; 0 failed",
            )
        } else {
            (
                "run_command",
                "success",
                false,
                "On branch main, working tree clean",
            )
        };

        let ev = NormalizedEvent {
            event_id: format!("ev-distill-{}", i),
            event_type: "tool_execution".to_string(),
            timestamp: Utc::now() + chrono::Duration::milliseconds(i as i64 * 100),
            agent: AgentInfo {
                adapter_id: "antigravity".to_string(),
                agent_instance_id: None,
            },
            session_id: session_id.to_string(),
            task_id: Some("task-fix-buffer-leak".to_string()),
            project: ProjectRef {
                id: Some("proj-core".to_string()),
                slug: Some("proj-core".to_string()),
                root_path: "/dev/proj".to_string(),
                source_git_remote: None,
            },
            tool: Some(ToolInfo {
                name: tool_name.to_string(),
                category: if tool_name == "view_file"
                    || tool_name == "list_dir"
                    || tool_name == "grep_search"
                {
                    "read".to_string()
                } else {
                    "exec".to_string()
                },
                input: Some(json!({ "command": format!("cargo test buffer --step-{}", i) })),
                input_hash: None,
                output: Some(json!(output_msg)),
                output_hash: None,
                status: status.to_string(),
                exit_code: if is_error { Some(1) } else { Some(0) },
                duration_ms: Some(120),
            }),
            artifacts: Vec::new(),
            sensitivity: "internal".to_string(),
            schema_version: 1,
            raw_payload: None,
        };
        db.append_event(&ev).expect("failed to append test event");
    }

    // Execute Trajectory Distillation
    let start_distill = Instant::now();
    let report = distiller
        .distill_session(session_id)
        .expect("distill_session failed");
    let distill_time = start_distill.elapsed();

    println!(
        "⏱ [Distillation] 100 raw events distilled in: {:?}",
        distill_time
    );
    println!(
        "📊 Distillation Report: Total={}, Pruned={}, Ratio={:.1}%",
        report.total_raw_events,
        report.pruned_events,
        (report.pruned_events as f64 / report.total_raw_events as f64) * 100.0
    );

    assert_eq!(report.total_raw_events, 100);
    assert!(
        report.pruned_events >= 75,
        "Distillation must prune at least 75% redundant reads/scans"
    );
    assert!(
        report.synthesized_trace.is_some(),
        "Must produce a synthesized golden trace"
    );
    assert!(
        report.synthesized_memory.is_some(),
        "Must produce a high-signal crystallized memory"
    );
    assert!(
        !report.resolved_failures.is_empty(),
        "Must identify and resolve the error events"
    );

    let trace = report.synthesized_trace.unwrap();
    assert!(
        !trace.steps.is_empty() && trace.steps.len() <= 10,
        "Golden trace must be concise (<=10 steps)"
    );
}

#[test]
fn test_strict_project_isolation_and_global_preference_resolution() {
    let db = Database::open_in_memory().expect("in-memory db open failed");

    let proj_a = "proj-alpha";
    let proj_b = "proj-beta";

    // 1. Save Project Alpha Memory
    let mem_a = Memory {
        id: "mem-alpha-01".to_string(),
        scope: MemoryScope::Project,
        project_id: Some(proj_a.to_string()),
        memory_type: MemoryType::Decision,
        content: "Project Alpha strictly uses gRPC port 50051 for internal telemetry.".to_string(),
        summary: "Alpha uses gRPC port 50051".to_string(),
        confidence: 0.95,
        importance: 0.9,
        freshness: 1.0,
        status: MemoryStatus::Active,
        source_kind: MemorySourceKind::Agent,
        version: Some(1),
        created_at: Utc::now(),
        updated_at: Utc::now(),
        last_recalled_at: None,
        tags: vec!["grpc".to_string(), "telemetry".to_string()],
        entities: vec!["grpc".to_string()],
        metadata: json!({}),
    };
    db.save_memory(&mem_a).expect("save mem_a failed");

    // 2. Save Project Beta Memory
    let mem_b = Memory {
        id: "mem-beta-01".to_string(),
        scope: MemoryScope::Project,
        project_id: Some(proj_b.to_string()),
        memory_type: MemoryType::Decision,
        content: "Project Beta uses REST API on port 8080 with OpenAPI spec v3.".to_string(),
        summary: "Beta uses REST port 8080".to_string(),
        confidence: 0.95,
        importance: 0.9,
        freshness: 1.0,
        status: MemoryStatus::Active,
        source_kind: MemorySourceKind::Agent,
        version: Some(1),
        created_at: Utc::now(),
        updated_at: Utc::now(),
        last_recalled_at: None,
        tags: vec!["rest".to_string(), "openapi".to_string()],
        entities: vec!["rest".to_string()],
        metadata: json!({}),
    };
    db.save_memory(&mem_b).expect("save mem_b failed");

    // 3. Save Global Developer Preference
    let global_mem = Memory {
        id: "mem-global-pref-01".to_string(),
        scope: MemoryScope::Global,
        project_id: None,
        memory_type: MemoryType::Preference,
        content: "Always format code with standard 4-space indentation and deny unsafe blocks."
            .to_string(),
        summary: "Global rule: 4 spaces, no unsafe".to_string(),
        confidence: 0.99,
        importance: 1.0,
        freshness: 1.0,
        status: MemoryStatus::Active,
        source_kind: MemorySourceKind::User,
        version: Some(1),
        created_at: Utc::now(),
        updated_at: Utc::now(),
        last_recalled_at: None,
        tags: vec!["formatting".to_string(), "style".to_string()],
        entities: vec!["style".to_string()],
        metadata: json!({}),
    };
    db.save_memory(&global_mem).expect("save global_mem failed");

    // 4. Verify Project Isolation in FTS5 Search
    let results_a = db
        .search_memories_fts("gRPC telemetry port", Some(proj_a), 5)
        .unwrap();
    assert_eq!(results_a.len(), 1);
    assert_eq!(results_a[0].0.id, "mem-alpha-01");

    // Beta query for gRPC should return 0 (no leakage from Alpha)
    let results_b_leak_check = db
        .search_memories_fts("gRPC telemetry port", Some(proj_b), 5)
        .unwrap();
    assert_eq!(
        results_b_leak_check.len(),
        0,
        "Project Beta must not see Project Alpha's memories"
    );

    // 5. Verify Context Assembly incorporates project + global preferences
    let engine = RetrievalEngine::new(&db);
    let ctx_a = engine.assemble_context(Some(proj_a), None, 2000).unwrap();
    assert!(
        ctx_a.contains("Alpha uses gRPC port 50051"),
        "Context A must contain Alpha memory"
    );
    assert!(
        !ctx_a.contains("Beta uses REST port 8080"),
        "Context A must NOT contain Beta memory"
    );
}

#[test]
fn test_ast_knowledge_graph_multi_hop_impact_analysis() {
    let db = Database::open_in_memory().expect("in-memory db open failed");
    let builder = GraphBuilder::new(&db);
    let project_id = "proj-ast-test";

    // 1. Create File Entities
    let files = vec![
        ("f-auth", "crates/auth/src/token.rs", "file"),
        ("f-handler", "crates/api/src/auth_handler.rs", "file"),
        ("f-router", "crates/api/src/router.rs", "file"),
        ("f-server", "crates/server/src/main.rs", "file"),
    ];

    let entities: Vec<GraphEntity> = files
        .into_iter()
        .map(|(id, path, etype)| GraphEntity {
            id: id.to_string(),
            project_id: Some(project_id.to_string()),
            entity_type: etype.to_string(),
            name: path.split('/').last().unwrap().to_string(),
            path: path.to_string(),
            line_start: Some(1),
            line_end: Some(100),
            metadata: json!({}),
            created_at: Utc::now(),
            updated_at: Utc::now(),
        })
        .collect();

    db.batch_insert_graph_entities(&entities)
        .expect("batch_insert_graph_entities failed");

    // 2. Create Dependency Edges: f-server -> f-router -> f-handler -> f-auth
    let edges_raw = vec![
        ("f-server", "f-router", "imports"),
        ("f-router", "f-handler", "calls"),
        ("f-handler", "f-auth", "imports"),
    ];

    let edges: Vec<GraphEdge> = edges_raw
        .into_iter()
        .map(|(source, target, rel)| GraphEdge {
            source_id: source.to_string(),
            target_id: target.to_string(),
            relation_type: rel.to_string(),
            confidence: 1.0,
            created_at: Utc::now(),
        })
        .collect();

    db.batch_insert_graph_edges(&edges)
        .expect("batch_insert_graph_edges failed");

    // 3. Perform 3-Hop Blast Radius Impact Analysis when `crates/auth/src/token.rs` is modified
    let report = builder
        .trace_impact(Some(project_id), "crates/auth/src/token.rs", 3)
        .expect("trace_impact failed")
        .expect("report expected");

    println!(
        "🌳 [AST Graph] Blast radius for token.rs: {} affected files",
        report.affected_files.len()
    );
    for f in &report.affected_files {
        println!("   -> Impacted: {}", f);
    }

    assert_eq!(report.target_entity.id, "f-auth");
    assert!(report
        .affected_files
        .contains(&"crates/api/src/auth_handler.rs".to_string()));
    assert!(report
        .affected_files
        .contains(&"crates/api/src/router.rs".to_string()));
    assert!(report
        .affected_files
        .contains(&"crates/server/src/main.rs".to_string()));
    assert_eq!(
        report.affected_files.len(),
        3,
        "Should trace all 3 dependent ancestors up the graph"
    );
}

#[test]
fn test_security_scrubbing_and_sensitive_redaction() {
    let raw_prompt = "Connected with API_KEY=sk-proj-9843098230984209384092834098234 and Authorization: Bearer eyJhbGciOiJIUzI1NiIsInR5cCI6IkpXVCJ9.eyJzdWIiOiIxMjM0NTY3ODkwIn0.do_not_leak and GitHub ghp_1234567890abcdef1234567890abcdef123456";
    let redacted = redact_text(raw_prompt);

    println!("🔒 Redacted prompt: {}", redacted);
    assert!(!redacted.contains("sk-proj-"), "Must strip OpenAI keys");
    assert!(
        !redacted.contains("eyJhbGciOiJIUzI1NiIsInR5cCI6IkpXVCJ9"),
        "Must strip JWT tokens"
    );
    assert!(
        !redacted.contains("ghp_1234567890abcdef"),
        "Must strip GitHub tokens"
    );
    assert!(
        redacted.contains("[REDACTED_API_KEY]")
            || redacted.contains("[REDACTED_SECRET]")
            || redacted.contains("[REDACTED_JWT]")
    );
}
