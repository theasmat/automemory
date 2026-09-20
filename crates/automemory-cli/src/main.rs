mod commands;

use anyhow::Result;
use clap::{Parser, Subcommand};

#[derive(Parser)]
#[command(name = "automemory")]
#[command(about = "AutoMemory Pro: Open-source local-first persistent memory engine for coding agents", version = "0.1.0")]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Initialize AutoMemory Pro configuration and agent hooks
    Init {
        /// Agent to configure (e.g. agy, claude, cursor)
        #[arg(short, long, default_value = "agy")]
        agent: String,
    },

    /// Automatically connect and configure AutoMemory Pro into host coding agents
    Connect {
        /// Agent host to connect (agy, claude, cursor)
        #[arg(default_value = "agy")]
        agent: String,

        /// Apply configuration globally rather than to current workspace
        #[arg(short, long)]
        global: bool,
    },

    /// Out-of-process lifecycle hook handler for coding agents
    Hook {
        /// Agent name (e.g. agy)
        #[arg(long)]
        agent: String,

        /// Event name (e.g. boot, pre-tool, post-tool, post-tool-fail, task-done, stop)
        #[arg(long)]
        event: String,

        /// Project identifier ('auto' for auto-detection)
        #[arg(long, default_value = "auto")]
        project: String,
    },

    /// Run system diagnostic health checks
    Doctor,

    /// Start the background daemon and embedded web dashboard
    Serve {
        /// Port for dashboard web UI and HTTP MCP server
        #[arg(short, long, default_value_t = 4545)]
        port: u16,

        /// Custom path to SQLite database
        #[arg(long)]
        db: Option<String>,
    },

    /// Run the MCP server over stdio for coding agent integration
    Mcp {
        /// Custom path to SQLite database
        #[arg(long)]
        db: Option<String>,
    },

    /// Manage project registry
    Project {
        #[command(subcommand)]
        command: ProjectCommands,
    },

    /// Manage persistent memory records
    Memory {
        #[command(subcommand)]
        command: MemoryCommands,
    },

    /// Manage text-first vault sync
    Sync {
        #[command(subcommand)]
        command: SyncCommands,
    },
}

#[derive(Subcommand)]
enum ProjectCommands {
    /// List all registered projects
    List,

    /// Register a project directory
    Register {
        /// Project unique slug (e.g. my-app)
        #[arg(short, long)]
        slug: String,

        /// Human-readable display name
        #[arg(short, long)]
        name: String,

        /// Local path to project root
        #[arg(short, long)]
        path: Option<String>,
    },
}

#[derive(Subcommand)]
enum MemoryCommands {
    /// List memories
    List {
        #[arg(short, long)]
        project: Option<String>,

        #[arg(short, long, default_value_t = 20)]
        limit: usize,
    },

    /// Search memories using FTS5 full-text search
    Search {
        /// Search query
        query: String,

        #[arg(short, long)]
        project: Option<String>,

        #[arg(short, long, default_value_t = 10)]
        limit: usize,
    },

    /// Save a memory explicitly
    Save {
        /// Short 1-line summary
        summary: String,

        /// Detailed content
        #[arg(short, long)]
        content: String,

        /// Memory type (fact, decision, lesson, pattern, warning, observation, preference, skill)
        #[arg(short = 't', long)]
        mem_type: Option<String>,

        /// Scope (global or project)
        #[arg(short, long)]
        scope: Option<String>,

        /// Project ID or slug
        #[arg(short, long)]
        project: Option<String>,
    },

    /// Archive a memory
    Archive {
        /// Memory ID
        id: String,
    },
}

#[derive(Subcommand)]
enum SyncCommands {
    /// Export database to text-first JSONL & Markdown vault
    Export {
        #[arg(short, long)]
        path: Option<String>,
    },

    /// Import text-first vault records into local database
    Import {
        #[arg(short, long)]
        path: Option<String>,
    },

    /// Check vault sync status
    Status,
}

#[tokio::main]
async fn main() -> Result<()> {
    // In dev / debug mode (cargo run / debug builds), enable full debug & info logs.
    // In production / release builds, enable only critical error logging by default.
    let filter = if cfg!(debug_assertions) {
        tracing_subscriber::EnvFilter::try_from_default_env()
            .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("automemory=debug,tower_http=info"))
    } else {
        tracing_subscriber::EnvFilter::try_from_default_env()
            .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("automemory=error"))
    };

    let _ = tracing_subscriber::fmt()
        .with_env_filter(filter)
        .with_target(false)
        .compact()
        .try_init();

    let cli = Cli::parse();

    match cli.command {
        Commands::Init { agent } => {
            commands::init::execute(Some(agent))?;
        }
        Commands::Connect { agent, global } => {
            commands::connect::execute(&agent, global)?;
        }
        Commands::Hook {
            agent,
            event,
            project,
        } => {
            commands::hook::execute(&agent, &event, &project)?;
        }
        Commands::Doctor => {
            commands::doctor::execute()?;
        }
        Commands::Serve { port, db } => {
            commands::serve::execute(port, db).await?;
        }
        Commands::Mcp { db } => {
            commands::mcp::execute(db).await?;
        }
        Commands::Project { command } => match command {
            ProjectCommands::List => commands::project::list()?,
            ProjectCommands::Register { slug, name, path } => {
                commands::project::register(&slug, &name, path)?;
            }
        },
        Commands::Memory { command } => match command {
            MemoryCommands::List { project, limit } => {
                commands::memory::list(project, limit)?;
            }
            MemoryCommands::Search {
                query,
                project,
                limit,
            } => {
                commands::memory::search(&query, project, limit)?;
            }
            MemoryCommands::Save {
                summary,
                content,
                mem_type,
                scope,
                project,
            } => {
                commands::memory::save(&content, &summary, mem_type, scope, project)?;
            }
            MemoryCommands::Archive { id } => {
                commands::memory::archive(&id)?;
            }
        },
        Commands::Sync { command } => match command {
            SyncCommands::Export { path } => commands::sync::export_vault(path)?,
            SyncCommands::Import { path } => commands::sync::import_vault(path)?,
            SyncCommands::Status => commands::sync::status()?,
        },
    }

    Ok(())
}
