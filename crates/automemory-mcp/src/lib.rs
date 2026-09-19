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
        let tools = list_resp.result.unwrap()["tools"].as_array().unwrap().clone();
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

        // Recall memory
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
        assert!(content_text.as_str().unwrap().contains("DB pool size 25"));
    }
}
