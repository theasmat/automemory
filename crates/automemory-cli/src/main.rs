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

    /// Link project to canonical single-source context (~/.automemory_pro/context/projects/<id>) via local .context symlink
    Link {
        /// Project ID or slug (inferred from directory if omitted)
        project_id: Option<String>,
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

    /// Manage autopilot safety modes (vibe, strict, review) and guardrails
    Mode {
        #[command(subcommand)]
        command: ModeCommands,
    },

    /// Manage product documentation, PRDs, ADRs, and OpenViking L0/L1 compilation
    Doc {
        #[command(subcommand)]
        command: DocCommands,
    },

    /// Manage custom guardrail rules
    Rule {
        #[command(subcommand)]
        command: RuleCommands,
    },

    /// Manage synthesized procedural playbooks and agent skills
    Skill {
        #[command(subcommand)]
        command: SkillCommands,
    },
}


#[derive(Subcommand)]
enum DocCommands {
    /// Track a product specification or documentation file
    Track {
        /// Path to the markdown or specification file
        path: String,

        /// Document category (prd, adr, business-rules, api, general)
        #[arg(short, long)]
        category: Option<String>,

        /// Document display title
        #[arg(short, long)]
        title: Option<String>,

        /// Project slug (defaults to current project)
        #[arg(short, long)]
        project: Option<String>,
    },

    /// List all tracked product documents and their sync status
    List {
        /// Project slug (defaults to current project)
        #[arg(short, long)]
        project: Option<String>,
    },

    /// Compile L0/L1 summaries for all tracked documents in project
    Compile {
        /// Project slug (defaults to current project)
        #[arg(short, long)]
        project: Option<String>,
    },

    /// Automatically scan project for PRDs, ADRs, and markdown specifications
    Scan {
        /// Directory to scan (defaults to current directory)
        #[arg(short = 'd', long)]
        path: Option<String>,

        /// Project slug (defaults to current project)
        #[arg(short, long)]
        project: Option<String>,
    },
}

#[derive(Subcommand)]
enum RuleCommands {
    /// List all guardrail rules (built-in and custom)
    List {
        /// Optional project slug filter
        #[arg(short, long)]
        project: Option<String>,
    },
    /// Add a new custom guardrail rule
    Add {
        /// Rule name
        #[arg(short, long)]
        name: String,

        /// Regex pattern to match
        #[arg(short, long)]
        pattern: String,

        /// Critical flag (block in all modes)
        #[arg(short, long)]
        critical: bool,

        /// Action (block or warn)
        #[arg(short, long, default_value = "block")]
        action: String,

        /// Description of risk
        #[arg(short, long, default_value = "Custom guardrail policy")]
        description: String,

        /// Mitigation hint
        #[arg(short, long, default_value = "Command matches custom policy")]
        mitigation: String,

        /// Project slug
        #[arg(long)]
        project: Option<String>,
    },
    /// Remove a custom rule by ID
    Remove {
        /// Rule ID
        id: String,
    },
    /// Enable or disable a custom rule
    Toggle {
        /// Rule ID
        id: String,

        /// Enable flag (true or false)
        enabled: bool,
    },
}

#[derive(Subcommand)]
enum SkillCommands {
    /// List synthesized procedural skills and playbooks
    List {
        /// Project slug (defaults to current project)
        #[arg(short, long)]
        project: Option<String>,
    },
    /// Export synthesized skills into standard agent SKILL.md format (.agents/skills/<name>/SKILL.md)
    Export {
        /// Project slug (defaults to current project)
        #[arg(short, long)]
        project: Option<String>,

        /// Target root directory (defaults to .agents/skills)
        #[arg(short, long)]
        target: Option<String>,

        /// Minimum successful runs required to export (defaults to 1)
        #[arg(short, long, default_value_t = 1)]
        min_runs: usize,
    },
}

#[derive(Subcommand)]
enum ModeCommands {
    /// Get the current autopilot mode for a project
    Get {
        /// Project slug (defaults to current directory or 'default')
        #[arg(short, long)]
        project: Option<String>,
    },
    /// Set the autopilot mode for a project (vibe, strict, review)
    Set {
        /// Mode to apply (vibe, strict, review)
        mode: String,

        /// Project slug (defaults to current directory or 'default')
        #[arg(short, long)]
        project: Option<String>,
    },
}

#[derive(Subcommand)]
enum ProjectCommands {
    /// List all registered projects in global config (~/.automemory/config.toml)
    List,

    /// Register a project in the global registry
    Register {
        /// Project unique name or slug (defaults to directory name)
        #[arg(short, long)]
        name: Option<String>,

        /// Local path to project root (defaults to current directory)
        #[arg(short, long)]
        path: Option<String>,

        /// Comma-separated list of trackable file patterns (e.g. 'plans/**/*.md,docs/**/*.md')
        #[arg(short, long)]
        track: Option<String>,

        /// Dedicated remote GitHub vault repository URL
        #[arg(long)]
        vault_repo: Option<String>,

        /// Automatically restore git-ignored files to workspace on sync pull
        #[arg(long, default_value_t = true)]
        restore_to_workspace: bool,
    },

    /// Remove a project from the global registry
    Remove {
        /// Project name or local directory path
        name: String,
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
    /// Initialize cloud delta sync configuration (.automemory/config.json)
    Init {
        /// Project ID or slug (defaults to folder name or config)
        #[arg(short, long)]
        project: Option<String>,

        /// Cloud endpoint URL (Firebase Cloud Functions / Cloudflare Worker)
        #[arg(short, long)]
        endpoint: Option<String>,

        /// Authentication bearer token or API key
        #[arg(short, long)]
        token: Option<String>,
    },

    /// Check cloud sync status and pending deltas
    Status {
        /// Project slug (defaults to current project)
        #[arg(short, long)]
        project: Option<String>,
    },

    /// Push local document deltas and memories to cloud
    Push {
        /// Project slug (defaults to current project)
        #[arg(short, long)]
        project: Option<String>,

        /// Custom endpoint override
        #[arg(short, long)]
        endpoint: Option<String>,

        /// Auth token override
        #[arg(short, long)]
        token: Option<String>,
    },

    /// Pull remote updates from cloud endpoint
    Pull {
        /// Project slug (defaults to current project)
        #[arg(short, long)]
        project: Option<String>,

        /// Custom endpoint override
        #[arg(short, long)]
        endpoint: Option<String>,

        /// Auth token override
        #[arg(short, long)]
        token: Option<String>,
    },

    /// Disaster recovery: restore memories, skills, and product docs onto machine
    Restore {
        /// Project ID to restore
        #[arg(short, long)]
        project: Option<String>,

        /// Path to local backup or delta JSON archive
        #[arg(short, long)]
        from_file: Option<String>,

        /// Cloud endpoint to fetch manifest from
        #[arg(short, long)]
        endpoint: Option<String>,

        /// Auth token for cloud endpoint
        #[arg(short, long)]
        token: Option<String>,
    },

    /// Export database to text-first JSONL & Markdown vault (legacy)
    Export {
        #[arg(short, long)]
        path: Option<String>,
    },

    /// Import text-first vault records into local database (legacy)
    Import {
        #[arg(short, long)]
        path: Option<String>,
    },
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
        Commands::Link { project_id } => {
            commands::link::execute(project_id)?;
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
            ProjectCommands::Register { name, path, track, vault_repo, restore_to_workspace } => {
                commands::project::register(name, path, track, vault_repo, restore_to_workspace)?;
            }
            ProjectCommands::Remove { name } => {
                commands::project::remove(&name)?;
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
            SyncCommands::Init { project, endpoint, token } => {
                commands::sync::init(project, endpoint, token)?;
            }
            SyncCommands::Status { project } => {
                commands::sync::status(project)?;
            }
            SyncCommands::Push { project, endpoint, token } => {
                commands::sync::push(project, endpoint, token).await?;
            }
            SyncCommands::Pull { project, endpoint, token } => {
                commands::sync::pull(project, endpoint, token).await?;
            }
            SyncCommands::Restore { project, from_file, endpoint, token } => {
                commands::sync::restore(project, from_file, endpoint, token).await?;
            }
            SyncCommands::Export { path } => commands::sync::export_vault(path)?,
            SyncCommands::Import { path } => commands::sync::import_vault(path)?,
        },
        Commands::Mode { command } => match command {
            ModeCommands::Get { project } => commands::mode::get(project)?,
            ModeCommands::Set { mode, project } => commands::mode::set(&mode, project)?,
        },
        Commands::Doc { command } => match command {
            DocCommands::Track {
                path,
                category,
                title,
                project,
            } => commands::doc::track(path, category, title, project)?,
            DocCommands::List { project } => commands::doc::list(project)?,
            DocCommands::Compile { project } => commands::doc::compile(project)?,
            DocCommands::Scan { path, project } => commands::doc::scan(path, project)?,
        },
        Commands::Rule { command } => match command {
            RuleCommands::List { project } => commands::rule::list(project)?,
            RuleCommands::Add {
                name,
                pattern,
                critical,
                action,
                description,
                mitigation,
                project,
            } => commands::rule::add(name, pattern, critical, action, description, mitigation, project)?,
            RuleCommands::Remove { id } => commands::rule::remove(&id)?,
            RuleCommands::Toggle { id, enabled } => commands::rule::toggle(&id, enabled)?,
        },
        Commands::Skill { command } => match command {
            SkillCommands::List { project } => commands::skill::list(project)?,
            SkillCommands::Export { project, target, min_runs } => {
                commands::skill::export(project, target, min_runs)?;
            }
        },
    }


    Ok(())
}
