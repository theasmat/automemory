use crate::protocol::{JsonRpcRequest, JsonRpcResponse};
use crate::server::McpServer;
use anyhow::Result;
use std::sync::Arc;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};

pub struct StdioTransport {
    server: Arc<McpServer>,
}

impl StdioTransport {
    pub fn new(server: Arc<McpServer>) -> Self {
        Self { server }
    }

    pub async fn run(&self) -> Result<()> {
        let stdin = tokio::io::stdin();
        let mut reader = BufReader::new(stdin).lines();
        let mut stdout = tokio::io::stdout();

        while let Some(line) = reader.next_line().await? {
            let line = line.trim();
            if line.is_empty() {
                continue;
            }

            match serde_json::from_str::<JsonRpcRequest>(line) {
                Ok(req) => {
                    if let Some(resp) = self.server.handle_request(req) {
                        let mut serialized = serde_json::to_string(&resp)?;
                        serialized.push('\n');
                        stdout.write_all(serialized.as_bytes()).await?;
                        stdout.flush().await?;
                    }
                }
                Err(e) => {
                    let err_resp =
                        JsonRpcResponse::error(None, -32700, &format!("Parse error: {}", e));
                    let mut serialized = serde_json::to_string(&err_resp)?;
                    serialized.push('\n');
                    stdout.write_all(serialized.as_bytes()).await?;
                    stdout.flush().await?;
                }
            }
        }

        Ok(())
    }
}
