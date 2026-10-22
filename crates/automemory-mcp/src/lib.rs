pub mod http;
pub mod protocol;
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
        for retired in ["memory.ask", "memory.context", "memory.topics", "memory.digest", "memory.constitution", "memory.skills", "chat.search", "library.search", "baton.pass", "baton.pickup", "graph.index", "graph.query", "graph.neighbors", "graph.impact", "review.submit", "review.next", "review.post", "doc.list", "doc.get"] {
            assert!(!names.contains(&retired), "{retired} should be folded into another tool");
        }
        assert_eq!(names.len(), 49, "advertised tools (PLAN section 18)");
    }

    #[test]
    fn folded_tools_still_answer_and_their_replacements_reach_the_same_code() {
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
        // memory.list {view} reaches the same handler.
        assert_eq!(call("memory.list", json!({ "view": "topics" })), call("memory.topics", json!({})));
        assert_eq!(call("memory.list", json!({ "view": "constitution" })), call("memory.constitution", json!({})));
        // memory.search {scope}: chat and library go to their handlers; all returns the three.
        assert_eq!(call("memory.search", json!({ "query": "x", "scope": "chat" })), call("chat.search", json!({ "query": "x" })));
        assert_eq!(call("memory.search", json!({ "query": "x", "scope": "library" })), call("library.search", json!({ "query": "x" })));
        let all = call("memory.search", json!({ "query": "x", "scope": "all" }));
        assert!(all.get("notes").is_some() && all.get("chat").is_some() && all.get("library").is_some(), "{all}");
        // continuity.brief {handoff} passes the baton and leaves the note as the brief's next step.
        let h = call("continuity.brief", json!({ "action": "handoff", "session_id": "s1", "agent": "claude", "note": "Wire the cart page to the new route" }));
        assert_eq!(h["status"], "passed", "{h}");
        let brief = call("continuity.brief", json!({ "action": "get" }));
        assert!(brief["brief"].as_str().unwrap_or("").contains("Wire the cart page"), "{brief}");
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
        // so default recall withholds it per the PRD trust table.
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
        assert!(!content_text.as_str().unwrap().contains("DB pool size 25"));

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
        assert!(verified_text.as_str().unwrap().contains("DB pool verified"));
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
        let verifier = DecayVerifier::default();
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
    fn test_mcp_baton_pass_and_pickup() {
        let db = Arc::new(Database::open_in_memory().unwrap());
        let server = McpServer::new(db);

        let pass_req = JsonRpcRequest {
            jsonrpc: "2.0".to_string(),
            id: Some(json!(20)),
            method: "tools/call".to_string(),
            params: Some(json!({
                "name": "baton.pass",
                "arguments": {
                    "session_id": "sess-a",
                    "to_session": "sess-b",
                    "note": "finish the auth refactor",
                    "agent": "claude-code"
                }
            })),
        };
        let pass_resp = server.handle_request(pass_req).unwrap();
        let pass_text = pass_resp.result.unwrap()["content"][0]["text"]
            .as_str()
            .unwrap()
            .to_string();
        assert!(pass_text.contains("finish the auth refactor"));
        assert!(pass_text.contains("next_action"));

        let pickup_req = JsonRpcRequest {
            jsonrpc: "2.0".to_string(),
            id: Some(json!(21)),
            method: "tools/call".to_string(),
            params: Some(json!({
                "name": "baton.pickup",
                "arguments": { "session_id": "sess-b" }
            })),
        };
        let pickup_resp = server.handle_request(pickup_req).unwrap();
        let pickup_text = &pickup_resp.result.unwrap()["content"][0]["text"];
        assert!(pickup_text.as_str().unwrap().contains("sess-a"));
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
        assert!(claimed.contains("lock.acquire"));

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
    fn test_mcp_review_submit_next_post() {
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

        // Author queues; author cannot review their own diff.
        call(
            40,
            "review.submit",
            json!({
                "session_id": "author-1", "diff": "diff --git a/f", "title": "fix leak",
                "test_command": "cargo test"
            }),
        );
        let own_empty = call(41, "review.next", json!({ "session_id": "author-1" }));
        assert!(own_empty.contains("No pending reviews"));

        // Reviewer approves with green tests → approved.
        let job = call(42, "review.next", json!({ "session_id": "reviewer-1" }));
        assert!(job.contains("fix leak"));
        let rev_id: String = serde_json::from_str::<serde_json::Value>(&job).unwrap()["review"]
            ["id"]
            .as_str()
            .unwrap()
            .to_string();
        let verdict = call(
            43,
            "review.post",
            json!({
                "id": rev_id, "session_id": "reviewer-1",
                "approved": true, "tests_passed": true
            }),
        );
        assert!(verdict.contains("approved"));

        // Approval with red tests → rejected: tests judge.
        call(
            44,
            "review.submit",
            json!({ "session_id": "author-2", "diff": "diff2" }),
        );
        let job2 = call(45, "review.next", json!({ "session_id": "reviewer-1" }));
        let rev2: String = serde_json::from_str::<serde_json::Value>(&job2).unwrap()["review"]
            ["id"]
            .as_str()
            .unwrap()
            .to_string();
        let verdict2 = call(
            46,
            "review.post",
            json!({
                "id": rev2, "session_id": "reviewer-1",
                "approved": true, "tests_passed": false
            }),
        );
        assert!(verdict2.contains("rejected"));
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

        // ask aliases recall (Tier 4 withheld by default, returned on opt-in).
        let saved = call(
            60,
            "memory.save",
            json!({
                "content": "JWT in cookie", "summary": "auth fact", "tags": ["auth"]
            }),
        );
        assert!(saved.contains("saved"));
        let asked = call(61, "memory.ask", json!({ "query": "JWT" }));
        assert!(!asked.contains("auth fact"));
        let asked_open = call(
            66,
            "memory.ask",
            json!({ "query": "JWT", "include_untrusted": true }),
        );
        assert!(asked_open.contains("auth fact"));

        // topics lists the tag.
        let topics = call(62, "memory.topics", json!({}));
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
