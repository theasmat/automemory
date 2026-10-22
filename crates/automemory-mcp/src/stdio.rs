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
        let restarter = crate::restart::Restarter::new();
        // After a restart the new process did not see the handshake: take the client's name from the one that did.
        if let Ok(client) = std::env::var(crate::restart::CLIENT_ENV) {
            self.server.note_client(&client);
        }

        // A request the previous version of this server had already read when it restarted.
        let mut pending = std::env::var(crate::restart::PENDING_ENV).ok();
        std::env::remove_var(crate::restart::PENDING_ENV);

        loop {
            let line = match pending.take() {
                Some(l) => l,
                None => {
                    let Some(l) = reader.next_line().await? else { break };
                    // A newer automemory was installed and nothing else is waiting: become it, handing over this request (same process id, same pipes).
                    if reader.get_ref().buffer().is_empty() && restarter.is_stale() {
                        restarter.restart(self.server.client_name().as_deref(), Some(&l));
                    }
                    l
                }
            };
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
