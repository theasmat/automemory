pub mod http;
pub mod protocol;
pub mod server;
pub mod stdio;
pub mod tools;

pub use http::mcp_router;
pub use protocol::{JsonRpcRequest, JsonRpcResponse, ToolCallResult, ToolDefinition};
pub use server::McpServer;
pub use stdio::StdioTransport;
pub use tools::ToolRegistry;

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
        assert!(names.contains(&"memory.context"));
        assert!(names.contains(&"trace.save"));
        assert!(names.contains(&"failure.save"));
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
            call(
                id,
                "room.post",
                json!({
                    "room_id": room_id, "session_id": sess,
                    "kind": kind, "body": body, "evidence": [ev]
                }),
            )
        };
        // Out-of-turn post is refused.
        let refused = call(
            53,
            "room.post",
            json!({
                "room_id": room_id, "session_id": "b",
                "kind": "propose", "body": "nope", "evidence": [ev]
            }),
        );
        assert!(refused.contains("not your turn"));

        post(54, "a", "propose", "JWT plan: sign sessions in an httpOnly cookie");
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
}
