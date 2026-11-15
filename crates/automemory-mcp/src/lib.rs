pub mod http;
pub mod protocol;
pub mod restart;
pub mod server;
pub mod stdio;
pub mod tools;

pub use http::mcp_router;
pub use protocol::{JsonRpcRequest, JsonRpcResponse, ToolCallResult, ToolDefinition};
pub use server::McpServer;
pub use stdio::StdioTransport;
pub use tools::{canonical_agent, ToolRegistry};

#[cfg(test)]
mod tests {
    use super::*;
    use automemory_core::db::Database;
    use serde_json::json;
    use std::sync::Arc;

    #[test]
    fn test_mcp_initialize_and_tools_list() {
        let db = Arc::new(Database::open_in_memory().unwrap());
        let server = McpServer::new(db);

        // 1. Test initialize
        let init_req = JsonRpcRequest {
            jsonrpc: "2.0".to_string(),
            id: Some(json!(1)),
            method: "initialize".to_string(),
            params: None,
        };
        let init_resp = server.handle_request(init_req).expect("response expected");
        assert_eq!(init_resp.id, Some(json!(1)));
        assert!(init_resp.result.is_some());

        // 2. Test tools/list
        let list_req = JsonRpcRequest {
            jsonrpc: "2.0".to_string(),
            id: Some(json!(2)),
            method: "tools/list".to_string(),
            params: None,
        };
        let list_resp = server.handle_request(list_req).expect("response expected");
        let tools = list_resp.result.unwrap()["tools"]
            .as_array()
            .unwrap()
            .clone();
        assert!(!tools.is_empty());

        // Verify key tools exist
        let names: Vec<_> = tools.iter().map(|t| t["name"].as_str().unwrap()).collect();
        assert!(names.contains(&"memory.recall"));
        assert!(names.contains(&"memory.save"));
        assert!(names.contains(&"memory.search") && names.contains(&"memory.list"));
        assert!(names.contains(&"trace.save"));
        assert!(names.contains(&"failure.save"));
        // Folded tools are not advertised any more...
        for retired in ["memory.ask", "memory.context", "memory.topics", "memory.digest", "memory.constitution", "memory.skills", "chat.search", "library.search", "baton.pass", "baton.pickup", "graph.index", "graph.query", "graph.neighbors", "graph.impact", "lock.acquire", "lock.release", "review.submit", "review.next", "review.post", "doc.list", "doc.get"] {
            assert!(!names.contains(&retired), "{retired} should be folded into another tool");
        }
        assert_eq!(names.len(), 47, "advertised tools (docs/decisions.md)");
    }

    #[test]
    fn the_merged_tools_work_and_the_old_names_say_what_replaced_them() {
        let db = Arc::new(Database::open_in_memory().unwrap());
        let server = McpServer::new(db);
        let call = |name: &str, arguments: serde_json::Value| -> serde_json::Value {
            let r = server
                .handle_request(JsonRpcRequest { jsonrpc: "2.0".to_string(), id: Some(json!(1)), method: "tools/call".to_string(), params: Some(json!({ "name": name, "arguments": arguments })) })
                .unwrap()
                .result
                .unwrap();
            serde_json::from_str(r["content"][0]["text"].as_str().unwrap()).unwrap_or(json!({ "text": r["content"][0]["text"] }))
        };
        // The merged tools reach the handlers.
        assert!(call("memory.list", json!({ "view": "topics" })).is_object());
        let all = call("memory.search", json!({ "query": "x", "scope": "all" }));
        assert!(all.get("notes").is_some() && all.get("chat").is_some() && all.get("library").is_some(), "{all}");
        assert!(call("memory.search", json!({ "query": "x", "scope": "chat" })).get("hits").is_some());
        assert!(call("memory.search", json!({ "query": "x", "scope": "library" })).get("hits").is_some());
        // continuity.brief {handoff} passes the baton and leaves the note as the brief's next step.
        let h = call("continuity.brief", json!({ "action": "handoff", "session_id": "s1", "agent": "claude", "note": "Wire the cart page to the new route" }));
        assert_eq!(h["status"], "passed", "{h}");
        let brief = call("continuity.brief", json!({ "action": "get" }));
        assert!(brief["brief"].as_str().unwrap_or("").contains("Wire the cart page"), "{brief}");
        // An old name is not "unknown": it says what to use.
        for (old, instead) in [("memory.ask", "memory.recall"), ("chat.search", "scope=chat"), ("baton.pickup", "memory.resume"), ("graph.query", "codebase-memory"), ("doc.list", "library.list"), ("review.next", "room.pending")] {
            let r = call(old, json!({}));
            let text = r["text"].as_str().unwrap_or_default();
            assert!(text.contains("was removed") && text.contains(instead), "{old}: {text}");
        }
    }

    #[test]
    fn an_agent_cannot_change_archive_or_restore_a_note_a_person_wrote() {
        use automemory_core::models::{Memory, MemoryScope, MemorySourceKind, MemoryStatus, MemoryType};
        let db = Arc::new(Database::open_in_memory().unwrap());
        let mem = Memory {
            id: "m-person".into(),
            scope: MemoryScope::Project,
            project_id: None,
            memory_type: MemoryType::Fact,
            content: "the human's rule".into(),
            summary: "rule".into(),
            confidence: 0.9,
            importance: 0.5,
            freshness: 1.0,
            status: MemoryStatus::Active,
            source_kind: MemorySourceKind::User,
            version: Some(1),
            created_at: chrono::Utc::now(),
            updated_at: chrono::Utc::now(),
            last_recalled_at: None,
            tags: vec![],
            entities: vec![],
            metadata: json!({}),
        };
        db.save_memory_direct(&mem).unwrap();
        let server = McpServer::new(db.clone());
        for (tool, args) in [
            ("memory.update", json!({"id": "m-person", "content": "agent rewrite"})),
            ("memory.archive", json!({"id": "m-person"})),
            ("memory.restore", json!({"id": "m-person"})),
        ] {
            let req = JsonRpcRequest { jsonrpc: "2.0".into(), id: Some(json!(1)), method: "tools/call".into(), params: Some(json!({"name": tool, "arguments": args})) };
            let r = serde_json::to_value(server.handle_request(req).unwrap()).unwrap();
            assert_eq!(r["result"]["is_error"], true, "{tool} must be refused: {r}");
        }
        let kept = db.get_memory_by_id("m-person").unwrap().unwrap();
        assert_eq!(kept.content, "the human's rule");
        assert_eq!(kept.status, MemoryStatus::Active);
    }

    #[test]
    fn test_mcp_memory_save_and_recall() {
        let db = Arc::new(Database::open_in_memory().unwrap());
        let server = McpServer::new(db);

        // Save memory
        let save_req = JsonRpcRequest {
            jsonrpc: "2.0".to_string(),
            id: Some(json!(10)),
            method: "tools/call".to_string(),
            params: Some(json!({
                "name": "memory.save",
                "arguments": {
                    "content": "PostgreSQL connection pool max size set to 25.",
                    "summary": "DB pool size 25",
                    "type": "decision",
                    "tags": ["database", "postgres"]
                }
            })),
        };
        let save_resp = server.handle_request(save_req).unwrap();
        assert!(save_resp.result.is_some());

        // Recall memory: default save lands at Tier 4 (agent claim, 0.9),
        // so default recall shows it only in a capped, labelled slot (the strict policy withholds it).
        let recall_req = JsonRpcRequest {
            jsonrpc: "2.0".to_string(),
            id: Some(json!(11)),
            method: "tools/call".to_string(),
            params: Some(json!({
                "name": "memory.recall",
                "arguments": {
                    "query": "PostgreSQL pool"
                }
            })),
        };
        let recall_resp = server.handle_request(recall_req).unwrap();
        let content_text = &recall_resp.result.unwrap()["content"][0]["text"];
        // Another agent's unchecked note is shown by default, in a capped slot and labelled; `trust_policy = "strict"` withholds it.
        assert!(content_text.as_str().unwrap().contains("DB pool size 25"));
        assert!(content_text.as_str().unwrap().contains("unverified"));

        // Explicit opt-in returns Tier 4.
        let open_req = JsonRpcRequest {
            jsonrpc: "2.0".to_string(),
            id: Some(json!(12)),
            method: "tools/call".to_string(),
            params: Some(json!({
                "name": "memory.recall",
                "arguments": {
                    "query": "PostgreSQL pool",
                    "include_untrusted": true
                }
            })),
        };
        let open_resp = server.handle_request(open_req).unwrap();
        let open_text = &open_resp.result.unwrap()["content"][0]["text"];
        assert!(open_text.as_str().unwrap().contains("DB pool size 25"));

        // Verified memory (Tier 2) is returned by default.
        let save_verified = JsonRpcRequest {
            jsonrpc: "2.0".to_string(),
            id: Some(json!(13)),
            method: "tools/call".to_string(),
            params: Some(json!({
                "name": "memory.save",
                "arguments": {
                    "content": "PostgreSQL pool verified by test suite.",
                    "summary": "DB pool verified",
                    "type": "decision",
                    "confidence": 0.97,
                    "tags": ["database", "postgres"]
                }
            })),
        };
        assert!(server
            .handle_request(save_verified)
            .unwrap()
            .result
            .is_some());
        let recall_verified = JsonRpcRequest {
            jsonrpc: "2.0".to_string(),
            id: Some(json!(14)),
            method: "tools/call".to_string(),
            params: Some(json!({
                "name": "memory.recall",
                "arguments": {
                    "query": "PostgreSQL verified"
                }
            })),
        };
        let verified_resp = server.handle_request(recall_verified).unwrap();
        let verified_text = &verified_resp.result.unwrap()["content"][0]["text"];
        // An agent's own confidence (even 0.97) is not evidence: its note stays a Tier 4 claim, shown labelled and never as a fact.
        let vt = verified_text.as_str().unwrap();
        assert!(vt.contains("DB pool verified") && vt.contains("\"trust_tier\": 4") && vt.contains("unverified"), "{vt}");
        let with_claims = JsonRpcRequest {
            jsonrpc: "2.0".to_string(),
            id: Some(json!(15)),
            method: "tools/call".to_string(),
            params: Some(json!({
                "name": "memory.recall",
                "arguments": { "query": "PostgreSQL verified", "include_untrusted": true }
            })),
        };
        let text = server.handle_request(with_claims).unwrap().result.unwrap()["content"][0]["text"].as_str().unwrap().to_string();
        assert!(text.contains("DB pool verified") && text.contains("\"trust_tier\": 4"), "{text}");
    }

    #[test]
    fn test_strict_policy_quarantines_claims_end_to_end() {
        use crate::tools::recall_via;
        use automemory_core::ports::{
            BuiltinTrustPolicy, FtsRetriever, MemoryStore, StrictTrustPolicy,
        };
        use automemory_core::verify::DecayVerifier;

        let db = Arc::new(Database::open_in_memory().unwrap());
        let server = McpServer::new(db.clone());
        let save_req = JsonRpcRequest {
            jsonrpc: "2.0".to_string(),
            id: Some(json!(1)),
            method: "tools/call".to_string(),
            params: Some(json!({
                "name": "memory.save",
                "arguments": {
                    "content": "Unverified agent claim content.",
                    "summary": "agent claim summary"
                }
            })),
        };
        assert!(server.handle_request(save_req).unwrap().result.is_some());

        let store: &dyn MemoryStore = &*db;
        let retriever = FtsRetriever::new(store);
        let verifier = DecayVerifier;
        let text_of = |policy: &dyn automemory_core::ports::TrustPolicy| {
            let result = recall_via(&retriever, &verifier, policy, "agent claim", None, 5, true);
            result.content[0].text.clone()
        };

        // Builtin labels the claim Tier 4; strict reports Tier 5 + unverifiable.
        let builtin_text = text_of(&BuiltinTrustPolicy);
        assert!(builtin_text.contains("\"trust_tier\": 4"));
        let strict_text = text_of(&StrictTrustPolicy);
        assert!(strict_text.contains("\"trust_tier\": 5"));
        assert!(strict_text.contains("\"verify_status\": \"unverifiable\""));
        assert!(!strict_text.contains("\"verify_status\": \"verified\""));
    }

    #[test]
    fn a_handoff_is_written_with_the_brief_and_read_back_with_resume() {
        let db = Arc::new(Database::open_in_memory().unwrap());
        let server = McpServer::with_identity(db, Some("claude".into()));
        let sent = run_on(&server, "continuity.brief", json!({ "action": "handoff", "session_id": "sess-a", "to_session": "sess-b", "note": "finish the auth refactor", "agent": "claude-code" }));
        assert!(sent.contains("finish the auth refactor") && sent.contains("next_action"), "{sent}");
        let resumed = run_on(&server, "memory.resume", json!({ "session_id": "sess-b" }));
        assert!(resumed.contains("sess-a") || resumed.contains("finish the auth refactor"), "{resumed}");
    }

    #[test]
    fn task_next_and_submit_work_without_a_session_id() {
        let db = Arc::new(Database::open_in_memory().unwrap());
        db.create_task("lead", None, "build numbering", "spec", &[]).unwrap();
        let server = McpServer::with_identity(db.clone(), Some("claude".into()));
        let call = |name: &str, arguments: serde_json::Value| {
            let r = server
                .handle_request(JsonRpcRequest { jsonrpc: "2.0".into(), id: Some(json!(1)), method: "tools/call".into(), params: Some(json!({ "name": name, "arguments": arguments })) })
                .unwrap()
                .result
                .unwrap();
            (r["is_error"] == true, r["content"][0]["text"].as_str().unwrap().to_string())
        };
        // No session id: allowed, and the id is the same from claim to submit.
        let (err, text) = call("task.next", json!({}));
        assert!(!err, "{text}");
        let id = serde_json::from_str::<serde_json::Value>(&text).unwrap()["task"]["id"].as_str().unwrap().to_string();
        let (err, text) = call("task.submit", json!({ "id": id, "result": "done" }));
        assert!(!err, "{text}");
    }

    #[test]
    fn test_mcp_task_create_next_submit() {
        let db = Arc::new(Database::open_in_memory().unwrap());
        let server = McpServer::new(db);

        let call = |id: i64, name: &str, arguments: serde_json::Value| {
            server
                .handle_request(JsonRpcRequest {
                    jsonrpc: "2.0".to_string(),
                    id: Some(json!(id)),
                    method: "tools/call".to_string(),
                    params: Some(json!({ "name": name, "arguments": arguments })),
                })
                .unwrap()
                .result
                .unwrap()["content"][0]["text"]
                .as_str()
                .unwrap()
                .to_string()
        };

        let created = call(
            30,
            "task.create",
            json!({ "session_id": "lead", "title": "migrate auth", "symbols": ["auth::login"] }),
        );
        assert!(created.contains("created"));

        let claimed = call(31, "task.next", json!({ "session_id": "worker-1" }));
        assert!(claimed.contains("migrate auth"));
        assert!(claimed.contains("task.submit"));

        let refused_text = server
            .handle_request(JsonRpcRequest {
                jsonrpc: "2.0".to_string(),
                id: Some(json!(32)),
                method: "tools/call".to_string(),
                params: Some(json!({
                    "name": "task.submit",
                    "arguments": { "id": "task-nope", "session_id": "worker-1" }
                })),
            })
            .unwrap()
            .result
            .unwrap()["content"][0]["text"]
            .as_str()
            .unwrap()
            .to_string();
        assert!(refused_text.contains("refused"));

        let task_id: String = serde_json::from_str::<serde_json::Value>(&claimed).unwrap()["task"]
            ["id"]
            .as_str()
            .unwrap()
            .to_string();
        let submitted = call(
            33,
            "task.submit",
            json!({ "id": task_id, "session_id": "worker-1", "result": "done" }),
        );
        assert!(submitted.contains("submitted"));
        assert!(submitted.contains("baton.pass"));
    }

    #[test]
    fn test_mcp_room_full_round() {
        let db = Arc::new(Database::open_in_memory().unwrap());
        // Two agents are two servers (two processes in real use), sharing one database.
        let server = McpServer::with_identity(db.clone(), Some("claude".into()));
        let server_b = McpServer::with_identity(db, Some("agy".into()));

        let call_on = |srv: &McpServer, id: i64, name: &str, arguments: serde_json::Value| {
            srv.handle_request(JsonRpcRequest {
                jsonrpc: "2.0".to_string(),
                id: Some(json!(id)),
                method: "tools/call".to_string(),
                params: Some(json!({ "name": name, "arguments": arguments })),
            })
            .unwrap()
            .result
            .unwrap()["content"][0]["text"]
            .as_str()
            .unwrap()
            .to_string()
        };
        let call = |id: i64, name: &str, arguments: serde_json::Value| call_on(&server, id, name, arguments);

        let saved = call(
            50,
            "memory.save",
            json!({
                "content": "JWT in cookie", "summary": "auth fact"
            }),
        );
        let ev: String = serde_json::from_str::<serde_json::Value>(&saved).unwrap()["id"]
            .as_str()
            .unwrap()
            .to_string();

        let joined = call(
            51,
            "room.join",
            json!({ "session_id": "a", "topic": "auth" }),
        );
        let room_id: String = serde_json::from_str::<serde_json::Value>(&joined).unwrap()["room"]
            ["id"]
            .as_str()
            .unwrap()
            .to_string();
        call(
            52,
            "room.join",
            json!({ "session_id": "b", "room_id": room_id }),
        );

        let post = |id: i64, sess: &str, kind: &str, body: &str| {
            call_on(
                if sess == "b" { &server_b } else { &server },
                id,
                "room.post",
                json!({
                    "room_id": room_id, "session_id": sess,
                    "kind": kind, "body": body, "evidence": [ev]
                }),
            )
        };
        // Out-of-turn post is refused.
        let refused = call_on(
            &server_b,
            53,
            "room.post",
            json!({
                "room_id": room_id, "session_id": "b",
                "kind": "propose", "body": "nope", "evidence": [ev]
            }),
        );
        assert!(refused.contains("not your turn"));

        post(54, "a", "propose", "JWT plan: sign sessions in an httpOnly cookie");
        // Changing the session id does not make a second agent: the same server cannot critique its own proposal.
        let self_critique = call(58, "room.post", json!({ "room_id": room_id, "session_id": "b", "kind": "critique", "body": "I disagree with myself, strongly", "evidence": [ev] }));
        assert!(self_critique.contains("another agent must critique"), "{self_critique}");
        post(55, "b", "critique", "Rotation of signing keys is not covered");
        let last = post(56, "a", "revise", "JWT plus key rotation every 24 hours");
        assert!(last.contains("decided"));
        assert!(last.contains("decision.record"));

        let status = call(57, "room.status", json!({ "room_id": room_id }));
        let status: serde_json::Value = serde_json::from_str(&status).unwrap();
        assert_eq!(status["room"]["state"], "decided");
    }

    #[test]
    fn test_mcp_prd_alias_tools() {
        let db = Arc::new(Database::open_in_memory().unwrap());
        let server = McpServer::new(db);

        let call = |id: i64, name: &str, arguments: serde_json::Value| {
            server
                .handle_request(JsonRpcRequest {
                    jsonrpc: "2.0".to_string(),
                    id: Some(json!(id)),
                    method: "tools/call".to_string(),
                    params: Some(json!({ "name": name, "arguments": arguments })),
                })
                .unwrap()
                .result
                .unwrap()["content"][0]["text"]
                .as_str()
                .unwrap()
                .to_string()
        };

        // recall shows Tier 4 in its capped slot, labelled; Tier 5 needs the opt-in.
        let saved = call(
            60,
            "memory.save",
            json!({
                "content": "JWT in cookie", "summary": "auth fact", "tags": ["auth"]
            }),
        );
        assert!(saved.contains("saved"));
        let asked = call(61, "memory.recall", json!({ "query": "JWT" }));
        assert!(asked.contains("auth fact") && asked.contains("unverified"), "the agent claim shows, labelled: {asked}");
        let asked_open = call(
            66,
            "memory.recall",
            json!({ "query": "JWT", "include_untrusted": true }),
        );
        assert!(asked_open.contains("auth fact"));

        // topics lists the tag.
        let topics = call(62, "memory.list", json!({ "view": "topics" }));
        assert!(topics.contains("auth"));

        // about answers for the tag target (opt-in surfaces the Tier 4 hit).
        let about = call(
            63,
            "memory.about",
            json!({ "target": "auth", "include_untrusted": true }),
        );
        assert!(about.contains("auth fact"));

        // propose without provenance is rejected; with it, queued as candidate.
        let refused = call(
            64,
            "memory.propose",
            json!({
                "content": "x", "summary": "y", "evidence": []
            }),
        );
        assert!(refused.contains("provenance"));
        let queued = call(
            65,
            "memory.propose",
            json!({
                "content": "Use rotation", "summary": "rotate it", "evidence": ["manual"]
            }),
        );
        assert!(queued.contains("proposed"));
    }

    fn run_on(srv: &McpServer, name: &str, arguments: serde_json::Value) -> String {
        srv.handle_request(JsonRpcRequest { jsonrpc: "2.0".to_string(), id: Some(json!(1)), method: "tools/call".to_string(), params: Some(json!({ "name": name, "arguments": arguments })) })
            .unwrap()
            .result
            .unwrap()["content"][0]["text"]
            .as_str()
            .unwrap()
            .to_string()
    }

    #[test]
    fn a_server_signs_what_it_writes_with_its_own_name_whatever_the_caller_says() {
        let db = Arc::new(Database::open_in_memory().unwrap());
        let claude = McpServer::with_identity(db.clone(), Some("Claude-Code".into()));
        run_on(&claude, "decision.record", json!({ "topic": "auth", "decision": "Use JWT", "reason": "stateless", "agent": "codex" }));
        let d = db.list_decisions(None, None, 10).unwrap();
        assert_eq!(d[0].agent, "claude", "the caller said codex; the server said claude");
        let audit: i64 = db.conn().query_row("SELECT COUNT(*) FROM audit_log WHERE action = 'mcp.agent_mismatch'", [], |r| r.get(0)).unwrap();
        assert_eq!(audit, 1, "the attempt is on record");
        // Saying your own name (any spelling) is not a mismatch.
        run_on(&claude, "decision.record", json!({ "topic": "db", "decision": "SQLite", "reason": "local", "agent": "claude-code" }));
        let audit: i64 = db.conn().query_row("SELECT COUNT(*) FROM audit_log WHERE action = 'mcp.agent_mismatch'", [], |r| r.get(0)).unwrap();
        assert_eq!(audit, 1);
    }

    #[test]
    fn search_filters_keep_their_agent_argument() {
        let db = Arc::new(Database::open_in_memory().unwrap());
        db.chat_append("agy", "s1", Some("p"), None, "c", &[("user".into(), None, "sticky routing talk".into())]).unwrap();
        let claude = McpServer::with_identity(db, Some("claude".into()));
        let r = run_on(&claude, "memory.search", json!({ "query": "sticky", "scope": "chat", "agent": "agy", "project_id": "p" }));
        assert!(r.contains("agy:s1"), "the agent argument of a search still selects whose chats to search: {r}");
    }

    #[test]
    fn without_a_configured_name_the_client_name_is_used_and_recorded() {
        let db = Arc::new(Database::open_in_memory().unwrap());
        let server = McpServer::new(db.clone());
        server
            .handle_request(JsonRpcRequest { jsonrpc: "2.0".to_string(), id: Some(json!(1)), method: "initialize".to_string(), params: Some(json!({ "clientInfo": { "name": "claude-code", "version": "2" } })) })
            .unwrap();
        run_on(&server, "decision.record", json!({ "topic": "x", "decision": "y", "reason": "z" }));
        assert_eq!(db.list_decisions(None, None, 10).unwrap()[0].agent, "claude", "no more records signed 'agent'");
        let rec = db.sync_kv_get("mcp_client:claude").unwrap();
        assert!(rec.contains("claude-code") && rec.contains("\"configured\":false"), "{rec}");
    }
}
