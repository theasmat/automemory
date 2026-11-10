use crate::protocol::{JsonRpcRequest, JsonRpcResponse, ToolCallResult};
use crate::tools::ToolRegistry;
use automemory_core::db::Database;
use serde_json::json;
use std::sync::Arc;

/// Protocol revisions this server speaks, oldest first. Calls carry no session state, so the same
/// handlers serve all of them; `initialize` only agrees on a version and is optional.
pub const SUPPORTED_VERSIONS: &[&str] = &["2024-11-05", "2025-03-26", "2025-06-18", "2025-11-25", "2026-07-28"];

pub struct McpServer {
    registry: ToolRegistry,
}

impl McpServer {
    pub fn new(db: Arc<Database>) -> Self {
        Self::with_identity(db, None)
    }

    /// `identity`: the agent this server was configured for (`automemory mcp --agent claude`); every record it writes is signed with that name.
    pub fn with_identity(db: Arc<Database>, identity: Option<String>) -> Self {
        Self {
            registry: ToolRegistry::with_identity(db, identity),
        }
    }

    pub fn note_client(&self, name: &str) {
        self.registry.note_client(name);
    }

    /// What the agent called itself when it connected.
    pub fn client_name(&self) -> Option<String> {
        self.registry.client_name()
    }

    fn server_description(&self, version: &str) -> serde_json::Value {
        json!({
            "protocolVersion": version,
            "serverInfo": { "name": "automemory", "version": "0.1.0" },
            "capabilities": { "tools": { "listChanged": false } }
        })
    }

    pub fn handle_request(&self, req: JsonRpcRequest) -> Option<JsonRpcResponse> {
        let id = req.id;
        match req.method.as_str() {
            "initialize" => {
                if let Some(name) = req.params.as_ref().and_then(|p| p.get("clientInfo")).and_then(|c| c.get("name")).and_then(|n| n.as_str()) {
                    self.registry.note_client(name);
                }
                // Agree on the client's version when we speak it, else offer our newest.
                let wanted = req.params.as_ref().and_then(|p| p.get("protocolVersion")).and_then(|v| v.as_str());
                let version = wanted.filter(|v| SUPPORTED_VERSIONS.contains(v)).unwrap_or(SUPPORTED_VERSIONS[SUPPORTED_VERSIONS.len() - 2]);
                Some(JsonRpcResponse::success(id, self.server_description(version)))
            }

            // Stateless clients (spec 2026-07-28) ask what the server is instead of handshaking.
            "server/discover" => Some(JsonRpcResponse::success(id, {
                let mut v = self.server_description(SUPPORTED_VERSIONS[SUPPORTED_VERSIONS.len() - 1]);
                v["supportedVersions"] = json!(SUPPORTED_VERSIONS);
                v
            })),

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
                let arguments = params
                    .get("arguments")
                    .cloned()
                    .unwrap_or_else(|| json!({}));
                let tool_result: ToolCallResult = self.registry.call_tool(name, &arguments);
                Some(JsonRpcResponse::success(
                    id,
                    serde_json::to_value(tool_result).unwrap_or_default(),
                ))
            }

            unknown => Some(JsonRpcResponse::error(
                id,
                -32601,
                &format!("Method not found: {}", unknown),
            )),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn call(server: &McpServer, method: &str, params: serde_json::Value) -> serde_json::Value {
        let req = JsonRpcRequest { jsonrpc: "2.0".into(), id: Some(json!(1)), method: method.into(), params: Some(params) };
        serde_json::to_value(server.handle_request(req).expect("a reply")).unwrap()
    }

    fn server() -> McpServer {
        McpServer::new(Arc::new(Database::open_in_memory().unwrap()))
    }

    #[test]
    fn initialize_agrees_on_a_supported_version_and_falls_back_otherwise() {
        let s = server();
        for v in SUPPORTED_VERSIONS {
            let r = call(&s, "initialize", json!({ "protocolVersion": v, "clientInfo": { "name": "t" } }));
            assert_eq!(r["result"]["protocolVersion"], *v);
        }
        let r = call(&s, "initialize", json!({ "protocolVersion": "1999-01-01" }));
        assert!(SUPPORTED_VERSIONS.contains(&r["result"]["protocolVersion"].as_str().unwrap()));
    }

    #[test]
    fn a_stateless_client_can_discover_and_call_without_initialize() {
        let s = server();
        let d = call(&s, "server/discover", json!({}));
        assert_eq!(d["result"]["serverInfo"]["name"], "automemory");
        assert_eq!(d["result"]["supportedVersions"].as_array().unwrap().len(), SUPPORTED_VERSIONS.len());
        let tools = call(&s, "tools/list", json!({}));
        assert!(!tools["result"]["tools"].as_array().unwrap().is_empty());
    }
}
