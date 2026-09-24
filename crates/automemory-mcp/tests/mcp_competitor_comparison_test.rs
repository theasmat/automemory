use automemory_core::db::Database;
use automemory_mcp::protocol::JsonRpcRequest;
use automemory_mcp::server::McpServer;
use serde_json::json;
use std::sync::Arc;

#[test]
fn test_mcp_comprehensive_tool_suite_execution() {
    let db = Arc::new(Database::open_in_memory().unwrap());
    let server = McpServer::new(db.clone());

    // 1. Register Project via MCP
    let req_reg = JsonRpcRequest {
        jsonrpc: "2.0".to_string(),
        id: Some(json!(101)),
        method: "tools/call".to_string(),
        params: Some(json!({
            "name": "project.register",
            "arguments": {
                "slug": "mcp-test-app",
                "name": "MCP Test Application",
                "root_path": "/workspace/mcp-test"
            }
        })),
    };
    let resp_reg = server.handle_request(req_reg).unwrap();
    assert!(resp_reg.result.is_some(), "project.register must succeed");

    // 2. Save Memory via MCP
    let req_save = JsonRpcRequest {
        jsonrpc: "2.0".to_string(),
        id: Some(json!(102)),
        method: "tools/call".to_string(),
        params: Some(json!({
            "name": "memory.save",
            "arguments": {
                "project_id": "mcp-test-app",
                "content": "Redis cache TTL configured to 300 seconds for hot session keys.",
                "summary": "Redis TTL 300s",
                "type": "decision",
                "tags": ["redis", "cache", "sessions"]
            }
        })),
    };
    let resp_save = server.handle_request(req_save).unwrap();
    assert!(resp_save.result.is_some());

    // 3. Recall Memory via MCP
    let req_recall = JsonRpcRequest {
        jsonrpc: "2.0".to_string(),
        id: Some(json!(103)),
        method: "tools/call".to_string(),
        params: Some(json!({
            "name": "memory.recall",
            "arguments": {
                "query": "Redis session TTL",
                "project_id": "mcp-test-app"
            }
        })),
    };
    let resp_recall = server.handle_request(req_recall).unwrap();
    let text = resp_recall.result.unwrap()["content"][0]["text"].as_str().unwrap().to_string();
    assert!(text.contains("Redis TTL 300s"));

    // 4. Save & Query Repeatable SuccessTrace via MCP
    let req_trace = JsonRpcRequest {
        jsonrpc: "2.0".to_string(),
        id: Some(json!(104)),
        method: "tools/call".to_string(),
        params: Some(json!({
            "name": "trace.save",
            "arguments": {
                "project_id": "mcp-test-app",
                "title": "Migrate SQLite to WAL mode",
                "category": "database",
                "steps": [
                    { "position": 1, "action": "Run PRAGMA journal_mode=WAL", "command": "sqlite3 db.sqlite 'PRAGMA journal_mode=WAL;'" },
                    { "position": 2, "action": "Verify busy_timeout is set", "command": "PRAGMA busy_timeout=5000;" }
                ]
            }
        })),
    };
    let resp_trace = server.handle_request(req_trace).unwrap();
    assert!(resp_trace.result.is_some(), "trace.save must succeed");

    // 5. Save & Resolve Failure Record via MCP
    let req_fail = JsonRpcRequest {
        jsonrpc: "2.0".to_string(),
        id: Some(json!(105)),
        method: "tools/call".to_string(),
        params: Some(json!({
            "name": "failure.save",
            "arguments": {
                "project_id": "mcp-test-app",
                "task_title": "Database connection error",
                "failed_tool": "sql_query",
                "error_message": "database is locked (5)",
                "root_cause": "Concurrent writes without WAL mode enabled",
                "fix_applied": "Enabled WAL mode and set busy_timeout to 5000ms"
            }
        })),
    };
    let resp_fail = server.handle_request(req_fail).unwrap();
    assert!(resp_fail.result.is_some(), "failure.save must succeed");

    // 6. Set & Lock Developer Preference via MCP
    let req_pref = JsonRpcRequest {
        jsonrpc: "2.0".to_string(),
        id: Some(json!(106)),
        method: "tools/call".to_string(),
        params: Some(json!({
            "name": "preference.set",
            "arguments": {
                "key": "compiler_flags",
                "value": "-D warnings",
                "scope": "global",
                "locked": true
            }
        })),
    };
    let resp_pref = server.handle_request(req_pref).unwrap();
    assert!(resp_pref.result.is_some(), "preference.set must succeed");

    // 7. Assemble Bounded Context Package via MCP
    let req_ctx = JsonRpcRequest {
        jsonrpc: "2.0".to_string(),
        id: Some(json!(107)),
        method: "tools/call".to_string(),
        params: Some(json!({
            "name": "memory.context",
            "arguments": {
                "project_id": "mcp-test-app",
                "task_query": "How do we configure database and cache?",
                "max_tokens": 1500
            }
        })),
    };
    let resp_ctx = server.handle_request(req_ctx).unwrap();
    let ctx_text = resp_ctx.result.unwrap()["content"][0]["text"].as_str().unwrap().to_string();
    assert!(ctx_text.contains("Redis TTL 300s"));
    println!("📦 Context Assembly Output:\n{}", ctx_text);
}
