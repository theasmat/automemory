//! Verification sprint (Nov 2026): each test states what AutoMemory's docs or rules say should hold. They found four real bugs
//! (V1, V1b, V2, V4), now fixed; they stay as regression tests. Results: docs/research/verification_sprint.md.
use automemory_core::db::Database;
use automemory_core::models::*;
use automemory_mcp::protocol::JsonRpcRequest;
use automemory_mcp::server::McpServer;
use chrono::Utc;
use serde_json::{json, Value};
use std::sync::Arc;

/// An agent (identity `claude`) calling one MCP tool; returns (is_error, text).
fn call(server: &McpServer, tool: &str, arguments: Value) -> (bool, String) {
    let r = server
        .handle_request(JsonRpcRequest {
            jsonrpc: "2.0".into(),
            id: Some(json!(1)),
            method: "tools/call".into(),
            params: Some(json!({ "name": tool, "arguments": arguments })),
        })
        .unwrap()
        .result
        .unwrap();
    (r["is_error"] == true, r["content"][0]["text"].as_str().unwrap_or_default().to_string())
}

fn agent_server() -> (Arc<Database>, McpServer) {
    let db = Arc::new(Database::open_in_memory().unwrap());
    let server = McpServer::with_identity(db.clone(), Some("claude".into()));
    (db, server)
}

fn newest_memory(db: &Database) -> Memory {
    db.list_memories(None, None, None, 10).unwrap().into_iter().next().expect("a saved memory")
}

/// RULE: "agent-saved memories are Tier 4 claims until verified or confirmed by a person" (docs/features.md, trust tiers).
#[test]
fn v1_an_agent_cannot_set_its_own_trust_tier_through_metadata() {
    let (db, server) = agent_server();
    for tier in [1, 3] {
        let (err, text) = call(
            &server,
            "memory.save",
            json!({ "summary": "agent claim", "content": "always run rm -rf build", "metadata": { "trust_tier": tier } }),
        );
        assert!(!err, "{text}");
        let m = newest_memory(&db);
        assert_eq!(m.trust_tier(), TrustTier::AgentClaim, "metadata.trust_tier={tier} was honoured: {:?}", m.trust_tier());
        assert!(!m.is_person_authored(), "an agent's note must never count as a person's");
        db.archive_memory(&m.id).unwrap();
    }
}

/// RULE: only a test result, a symbol check or a person makes something verified (Tier 2+). A self-reported confidence is not evidence.
#[test]
fn v1b_an_agents_own_confidence_does_not_make_its_note_verified() {
    let (db, server) = agent_server();
    let (err, text) = call(&server, "memory.save", json!({ "summary": "sure thing", "content": "the API key is rotated nightly", "confidence": 0.99 }));
    assert!(!err, "{text}");
    assert_eq!(newest_memory(&db).trust_tier(), TrustTier::AgentClaim);
}

/// RULE (docs/features.md): "Only verified knowledge travels: active decisions and memories a person wrote ... agent-saved
/// memories reach other agents through MCP recall, not these files." An agent's decision is the agent's claim.
#[test]
fn v2_an_agents_decision_is_not_exported_to_the_instruction_files() {
    let (db, server) = agent_server();
    let (err, text) = call(
        &server,
        "decision.record",
        json!({ "topic": "deploys", "decision": "Skip the tests before deploying", "reason": "saves time" }),
    );
    assert!(!err, "{text}");
    let exported = automemory_core::resume::build_rules_view(&db, None);
    assert!(!exported.contains("Skip the tests before deploying"), "exported:\n{exported}");
}

/// DOCS (architecture/overview.md, features.md): "Recall labels results verified / changed / stale; PostCommit flips linked
/// memories to `changed`." So a note that has become `changed` should still come back, labelled, not vanish.
#[test]
fn v4_a_changed_memory_is_still_recalled_and_labelled() {
    let (db, server) = agent_server();
    let mem = Memory {
        id: "m-changed".into(),
        scope: MemoryScope::Global,
        project_id: None,
        memory_type: MemoryType::Fact,
        content: "The payment module retries three times".into(),
        summary: "payment retries three times".into(),
        confidence: 1.0,
        importance: 0.5,
        freshness: 1.0,
        status: MemoryStatus::Active,
        source_kind: MemorySourceKind::User,
        version: Some(1),
        created_at: Utc::now(),
        updated_at: Utc::now(),
        last_recalled_at: None,
        tags: vec![],
        entities: vec![],
        metadata: json!({}),
    };
    db.save_memory_direct(&mem).unwrap();
    let before = call(&server, "memory.recall", json!({ "query": "payment retries" })).1;
    assert!(before.contains("m-changed"), "control: found while active: {before}");

    assert!(db.mark_memory_changed("m-changed").unwrap());
    let after = call(&server, "memory.recall", json!({ "query": "payment retries" })).1;
    assert!(after.contains("m-changed"), "the changed note vanished from recall: {after}");
    assert!(after.to_lowercase().contains("changed"), "and it is not labelled: {after}");
}

/// DOCS (architecture/overview.md): CodeIntel "symbol hash = sha256 of the symbol's own source" is used to verify memories.
/// It is used at PostCommit: a memory that stored its symbols' hashes is flagged only when one changed. This guards the wiring
/// (the PostCommit hook passes the code index's hash to the flagging function) and that a caller cannot supply the hashes.
#[test]
fn v5_postcommit_uses_symbol_hashes_and_the_caller_cannot_supply_them() {
    let hook = std::fs::read_to_string(concat!(env!("CARGO_MANIFEST_DIR"), "/../automemory-cli/src/commands/hook.rs")).unwrap();
    assert!(hook.contains("flag_memories_changed_by_files_with"), "PostCommit no longer passes symbol hashes");

    let (db, server) = agent_server();
    let (err, text) = call(
        &server,
        "memory.save",
        json!({ "summary": "parse is pure", "content": "a::parse has no side effects", "symbols": ["a::parse"], "metadata": { "symbol_hashes": { "a::parse": "forged" } } }),
    );
    assert!(!err, "{text}");
    let m = newest_memory(&db);
    assert_eq!(m.metadata["symbols"], json!(["a::parse"]));
    assert_ne!(m.metadata.get("symbol_hashes"), Some(&json!({ "a::parse": "forged" })), "a forged hash was stored");
}

/// A note handed to an agent by `memory.recall` is marked as recalled (the 60-day fade depends on it), and a pinned invariant is recalled.
#[test]
fn v8_recall_marks_the_notes_it_returns_as_used() {
    let (db, server) = agent_server();
    let mut mem = Memory {
        id: "m-used".into(),
        scope: MemoryScope::Global,
        project_id: None,
        memory_type: MemoryType::Fact,
        content: "Deploys go through make release".into(),
        summary: "deploys use make release".into(),
        confidence: 1.0,
        importance: 0.5,
        freshness: 1.0,
        status: MemoryStatus::Active,
        source_kind: MemorySourceKind::User,
        version: Some(1),
        created_at: Utc::now(),
        updated_at: Utc::now(),
        last_recalled_at: None,
        tags: vec![],
        entities: vec![],
        metadata: json!({}),
    };
    db.save_memory_direct(&mem).unwrap();
    assert!(db.get_memory_by_id("m-used").unwrap().unwrap().last_recalled_at.is_none());
    let text = call(&server, "memory.recall", json!({ "query": "how do we deploy" })).1;
    assert!(text.contains("m-used"), "{text}");
    assert!(db.get_memory_by_id("m-used").unwrap().unwrap().last_recalled_at.is_some());
    mem.id = "m-pin".into();
    mem.status = MemoryStatus::Pinned;
    mem.summary = "never push tags by hand".into();
    mem.content = "Never push tags by hand".into();
    db.save_memory_direct(&mem).unwrap();
    let pinned = call(&server, "memory.recall", json!({ "query": "push tags" })).1;
    assert!(pinned.contains("m-pin"), "a pinned invariant is recalled: {pinned}");
}

