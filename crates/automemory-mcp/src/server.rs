use crate::protocol::{JsonRpcRequest, JsonRpcResponse, ToolCallResult};
use crate::tools::ToolRegistry;
use automemory_core::db::Database;
use serde_json::json;
use std::sync::Arc;

pub struct McpServer {
    registry: ToolRegistry,
}

impl McpServer {
    pub fn new(db: Arc<Database>) -> Self {
        Self {
            registry: ToolRegistry::new(db),
        }
    }

    pub fn handle_request(&self, req: JsonRpcRequest) -> Option<JsonRpcResponse> {
        let id = req.id;
        match req.method.as_str() {
            "initialize" => {
                let res = json!({
                    "protocolVersion": "2024-11-05",
                    "serverInfo": {
                        "name": "automemory",
                        "version": "0.1.0"
                    },
                    "capabilities": {
                        "tools": {
                            "listChanged": false
                        }
                    }
                });
                Some(JsonRpcResponse::success(id, res))
            }

            "notifications/initialized" => None, // No response for notifications

            "ping" => Some(JsonRpcResponse::success(id, json!({}))),

            "tools/list" => {
                let tools = self.registry.list_tools();
                let tools_val = tools
                    .into_iter()
                    .map(|t| {
                        json!({
                            "name": t.name,
                            "description": t.description,
                            "inputSchema": t.input_schema
                        })
                    })
                    .collect::<Vec<_>>();
                Some(JsonRpcResponse::success(id, json!({ "tools": tools_val })))
            }

            "tools/call" => {
                let params = req.params.unwrap_or_else(|| json!({}));
                let name = match params.get("name").and_then(|v| v.as_str()) {
                    Some(n) => n,
                    None => {
                        return Some(JsonRpcResponse::error(
                            id,
                            -32602,
                            "Missing tool name in tools/call",
                        ));
                    }
                };
                let arguments = params.get("arguments").cloned().unwrap_or_else(|| json!({}));
                let tool_result: ToolCallResult = self.registry.call_tool(name, &arguments);
                Some(JsonRpcResponse::success(id, serde_json::to_value(tool_result).unwrap_or_default()))
            }

            unknown => Some(JsonRpcResponse::error(
                id,
                -32601,
                &format!("Method not found: {}", unknown),
            )),
        }
    }
}
