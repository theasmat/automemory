use anyhow::{Context, Result};
use automemory_core::config::Config;
use automemory_core::db::Database;
use automemory_mcp::{McpServer, StdioTransport};
use std::sync::Arc;

pub async fn execute(db_path: Option<String>) -> Result<()> {
    let config = Config::default_config();
    let path = db_path.map(std::path::PathBuf::from).unwrap_or(config.db_path);

    let db = Arc::new(
        Database::open(&path)
            .with_context(|| format!("Failed to open database at {:?}", path))?,
    );

    let server = Arc::new(McpServer::new(db));
    let transport = StdioTransport::new(server);
    transport.run().await?;
    Ok(())
}
