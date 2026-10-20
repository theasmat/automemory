mod commands;

use anyhow::Result;
use clap::{Parser, Subcommand};

#[derive(Parser)]
#[command(name = "automemory")]
#[command(
    about = "AutoMemory: Open-source local-first persistent memory engine for coding agents",
    version = "0.1.0",
    after_help = "Command Groups:
  Daily:     note, resume, handoff, locks, overlap, unlock, decision, memory
  Setup:     init, connect, disconnect, doctor, next, tour, learn, demo, completions, export-rules
  Advanced:  sync, mode, guardrail, rule, doc, eval, skill, proposal, embeddings, pin, veto, revoke, baton, stop, hooks, plugin, adapter, link, hook, serve, mcp, project"
)]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Initialize AutoMemory configuration and agent hooks
    #[command(next_help_heading = "Setup")]
    Init {
        /// Agent to configure (e.g. agy, claude, cursor)
        #[arg(short, long, default_value = "agy")]
        agent: String,
    },

    /// Automatically connect and configure AutoMemory into host coding agents
    ///
    /// Examples:
    ///   automemory connect claude
    ///   automemory connect agy
    ///   automemory connect cursor
    #[command(next_help_heading = "Setup")]
    Connect {
        /// Agent host to connect (agy, claude, claude-desktop, cursor, windsurf, continue, cline, roo, kilo, zed, codex, opencode, goose, aider, warp, gemini-cli, devin, copilot, openclaw, hermes, all)
        #[arg(default_value = "agy")]
        agent: String,

        /// Apply configuration globally rather than to current workspace
        #[arg(short, long)]
        global: bool,
    },

    /// Remove exactly what `connect` wrote for one host coding agent
    #[command(next_help_heading = "Setup")]
    Disconnect {
        /// Agent host to disconnect, or `all` (agy, claude, claude-desktop, cursor, windsurf, continue, cline, roo, kilo, zed, codex, opencode, goose, aider, warp, gemini-cli, devin, copilot, openclaw, hermes)
        #[arg(default_value = "agy")]
        agent: String,

        /// Remove global configuration rather than current workspace
        #[arg(short, long)]
        global: bool,

        /// Remove both the current workspace's and the global configuration
        #[arg(long)]
        everywhere: bool,

        /// List what is registered without removing anything
        #[arg(long)]
        dry_run: bool,
    },

    /// Run system diagnostic health checks
    #[command(next_help_heading = "Setup")]
    Doctor,

    /// Print the best next onboarding step and command
    #[command(next_help_heading = "Setup")]
    Next,

    /// Interactive guided tour in an isolated sandbox
    ///
    /// Examples:
    ///   automemory tour
    ///   automemory tour --auto
    ///   automemory tour --resume
    #[command(next_help_heading = "Setup")]
    Tour {
        /// Non-interactive automated tour (prints full transcript)
        #[arg(long)]
        auto: bool,

        /// Resume tour from the last completed step
        #[arg(long)]
        resume: bool,

        /// Reset tour progress and sandbox
        #[arg(long)]
        reset: bool,

        /// Run a specific step (1-8)
        #[arg(long)]
        step: Option<usize>,

        /// Disable ANSI color output
        #[arg(long)]
        no_color: bool,
    },

    /// Learn AutoMemory concepts, tools, and commands
    ///
    /// Topics: memory, decisions, locks, handoff, sync, modes, export-rules, embeddings
    #[command(next_help_heading = "Setup")]
    Learn {
        /// Topic to learn about (leave empty to list topics)
        topic: Option<String>,
    },

    /// Seed or remove sample data for exploring the dashboard and CLI
    ///
    /// Examples:
    ///   automemory demo
    ///   automemory demo --remove
    #[command(next_help_heading = "Setup")]
    Demo {
        /// Remove all sample demo data
        #[arg(long)]
        remove: bool,
    },

    /// Generate shell completion scripts (bash, zsh, fish, powershell, elvish)
    #[command(next_help_heading = "Setup")]
    Completions {
        /// Shell to generate completions for
        shell: clap_complete::Shell,
    },

    /// List registered agent adapters and refresh one's hook/event mappings
    #[command(next_help_heading = "Advanced")]
    Adapter {
        #[command(subcommand)]
        command: AdapterCommands,
    },

    /// Link project to canonical single-source context (~/.automemory/context/projects/<id>) via local .context symlink
    #[command(next_help_heading = "Advanced")]
    Link {
        /// Project ID or slug (inferred from directory if omitted)
        project_id: Option<String>,
    },

    /// Out-of-process lifecycle hook handler for coding agents
    #[command(next_help_heading = "Advanced")]
    Hook {
        /// Agent name (e.g. agy)
        #[arg(long)]
        agent: String,

        /// Event name (e.g. boot, pre-tool, post-tool, post-tool-fail, pre-commit, post-commit, task-done, stop)
        #[arg(long)]
        event: String,

        /// Project identifier ('auto' for auto-detection)
        #[arg(long, default_value = "auto")]
        project: String,
    },

    /// Start the background daemon and embedded web dashboard
    #[command(next_help_heading = "Advanced")]
    Serve {
        /// Port for dashboard web UI and HTTP MCP server
        #[arg(short, long, default_value_t = 4545)]
        port: u16,

        /// Custom path to SQLite database
        #[arg(long)]
        db: Option<String>,
    },

    /// Run the MCP server over stdio for coding agent integration
    #[command(next_help_heading = "Advanced")]
    Mcp {
        /// Custom path to SQLite database
        #[arg(long)]
        db: Option<String>,
    },

    /// Manage project registry
    #[command(next_help_heading = "Advanced")]
    Project {
        #[command(subcommand)]
        command: ProjectCommands,
    },

    /// Manage persistent memory records
    ///
    /// Examples:
    ///   automemory memory list
    ///   automemory memory search "database"
    ///   automemory memory archive <id>
    #[command(next_help_heading = "Daily")]
    Memory {
        #[command(subcommand)]
        command: MemoryCommands,
    },

    /// Tell every agent something about this project, in plain words
    ///
    /// Examples:
    ///   automemory note "Always run pnpm, never npm"
    ///   automemory note "Production API uses port 8080" -t preference
    #[command(next_help_heading = "Daily")]
    Note {
        text: String,
        /// fact (default), lesson, pattern, warning or preference
        #[arg(short = 't', long = "type", default_value = "fact")]
        kind: String,
    },

    /// Write verified project knowledge into the files agents read (AGENTS.md, .cursor/rules, ...)
    ///
    /// Examples:
    ///   automemory export-rules
    ///   automemory export-rules --check
    ///   automemory export-rules --agent claude
    #[command(next_help_heading = "Setup")]
    ExportRules {
        /// One agent (claude, codex, cursor, gemini-cli, copilot, windsurf, cline, roo, kiro)
        #[arg(long)]
        agent: Option<String>,
        /// Every known instruction file, not just those the repository already uses
        #[arg(long)]
        all: bool,
        /// Only report files that are out of date (non-zero exit)
        #[arg(long)]
        check: bool,
    },

    /// Recall mode and embedding provider (keyword or hybrid)
    #[command(next_help_heading = "Advanced")]
    Embeddings {
        #[command(subcommand)]
        command: EmbeddingsCommands,
    },

    /// Optional LLM that only suggests (conflicts, duplicates, candidate notes) into the Inbox
    #[command(next_help_heading = "Advanced")]
    Llm {
        #[command(subcommand)]
        command: LlmCommands,
    },

    /// The brief the next agent is handed: goal, ruled-out options, open questions, next steps
    #[command(next_help_heading = "Daily")]
    Continuity {
        #[command(subcommand)]
        command: ContinuityCommands,
    },

    /// Planning rooms: see what agents agreed and approve, reject or send it back
    #[command(next_help_heading = "Daily")]
    Room {
        #[command(subcommand)]
        command: RoomCommands,
    },

    /// Save, list and restore copies of the database
    #[command(next_help_heading = "Advanced")]
    Backup {
        #[command(subcommand)]
        command: Option<BackupCommands>,
    },

    /// What AutoMemory did for you lately (collisions prevented, notes recalled, hand-offs)
    #[command(next_help_heading = "Daily")]
    Stats {
        /// How many days back
        #[arg(long, default_value_t = 7)]
        days: i64,
        #[arg(long)]
        json: bool,
    },

    /// Install the latest release (checksum-verified; asks first, `--check` only reports)
    #[command(next_help_heading = "Advanced")]
    Update {
        /// Only say whether a newer version exists
        #[arg(long)]
        check: bool,
        /// Do not ask for confirmation
        #[arg(long, short = 'y')]
        yes: bool,
    },

    /// Show where the last session left off (hand-off, checkpoint, decisions, repository state)
    #[command(next_help_heading = "Daily")]
    Resume,

    /// Write a hand-off for the next session or machine and sync it
    #[command(next_help_heading = "Daily")]
    Handoff {
        /// What the next session should do first (flag)
        #[arg(short, long)]
        note: Option<String>,

        /// What the next session should do first (positional)
        message: Option<String>,
    },

    /// Manage text-first vault sync
    ///
    /// Examples:
    ///   automemory sync remote https://github.com/org/vault.git
    ///   automemory sync auto
    ///   automemory sync pair
    #[command(next_help_heading = "Advanced")]
    Sync {
        #[command(subcommand)]
        command: SyncCommands,
    },

    /// Manage autopilot safety modes (vibe, strict, review) and guardrails
    #[command(next_help_heading = "Advanced")]
    Mode {
        #[command(subcommand)]
        command: ModeCommands,
    },

    /// Manage product documentation, PRDs, ADRs, and OpenViking L0/L1 compilation
    #[command(next_help_heading = "Advanced")]
    Doc {
        #[command(subcommand)]
        command: DocCommands,
    },

    /// Manage custom guardrail rules
    #[command(next_help_heading = "Advanced")]
    Rule {
        #[command(subcommand)]
        command: RuleCommands,
    },

    /// Evaluate a shell command against guardrail rules without executing it
    #[command(next_help_heading = "Advanced")]
    Guardrail {
        #[command(subcommand)]
        command: GuardrailCommands,
    },

    /// Run the local eval suites (retrieval, stale flags, leases, hook latency)
    #[command(next_help_heading = "Advanced")]
    Eval {
        #[command(subcommand)]
        command: EvalCommands,
    },

    /// Manage synthesized procedural playbooks and agent skills
    #[command(next_help_heading = "Advanced")]
    Skill {
        #[command(subcommand)]
        command: SkillCommands,
    },

    /// Manage review proposals, deduplication merges, and conflict governance
    #[command(next_help_heading = "Advanced")]
    Proposal {
        #[command(subcommand)]
        command: ProposalCommands,
    },

    /// View and inspect active multi-agent file and symbol locks
    ///
    /// Examples:
    ///   automemory locks
    ///   automemory locks --coverage
    #[command(next_help_heading = "Daily")]
    Locks {
        /// Optional project slug or ID filter
        #[arg(short, long)]
        project: Option<String>,

        /// Instead of listing leases, report how many file changes a lease covered
        #[arg(long)]
        coverage: bool,

        /// Window for --coverage, in days
        #[arg(long, default_value_t = 7)]
        days: i64,
    },

    /// Warn about other branches that change the same files as this one (run before merging)
    #[command(next_help_heading = "Daily")]
    Overlap {
        /// Branch everything is compared against
        #[arg(long, default_value = "main")]
        base: String,

        /// Branch to check (default: the checked-out branch)
        #[arg(long)]
        branch: Option<String>,
    },

    /// Release or override an active resource lock
    #[command(next_help_heading = "Daily")]
    Unlock {
        /// Resource scope (file path, glob pattern, or symbol)
        scope: String,

        /// Optional project slug or ID
        #[arg(short, long)]
        project: Option<String>,

        /// Force unlock regardless of session
        #[arg(long, default_value_t = false)]
        force: bool,
    },

    /// Manage architectural decisions log with dissent and history
    ///
    /// Examples:
    ///   automemory decision list
    ///   automemory decision record --topic "Storage" --decision "SQLite WAL" --reason "Fast local ACID"
    #[command(next_help_heading = "Daily")]
    Decision {
        #[command(subcommand)]
        command: DecisionCommands,
    },

    /// Pin a memory as an immutable Tier 1 invariant (human control)
    #[command(next_help_heading = "Advanced")]
    Pin {
        /// Memory ID to pin
        id: String,
    },

    /// Veto and reject a candidate memory or mark invalid (human control)
    #[command(next_help_heading = "Advanced")]
    Veto {
        /// Memory ID to veto
        id: String,
        /// Reason for veto
        #[arg(short, long)]
        reason: Option<String>,
    },

    /// Revoke and archive memories by ID, session, or agent (human control)
    #[command(next_help_heading = "Advanced")]
    Revoke {
        /// Specific memory ID to revoke
        #[arg(long)]
        id: Option<String>,
        /// Revoke all memories produced by this session ID
        #[arg(long)]
        session: Option<String>,
        /// Revoke all memories produced by this agent name
        #[arg(long)]
        agent: Option<String>,
    },

    /// Agent-to-agent handoff baton management (<400-token ledger-grounded packets)
    #[command(next_help_heading = "Advanced")]
    Baton {
        #[command(subcommand)]
        command: BatonCommands,
    },

    /// Kill switch: release every active lease, halting multi-agent work
    #[command(next_help_heading = "Advanced")]
    Stop {
        /// Limit the halt to one project slug or ID
        #[arg(short, long)]
        project: Option<String>,
    },

    /// Inspect and dry-run the hook pipeline (config: hooks.toml)
    #[command(next_help_heading = "Advanced")]
    Hooks {
        #[command(subcommand)]
        command: HooksCommands,
    },

    /// List versioned ports and installed plugin manifests
    #[command(next_help_heading = "Advanced")]
    Plugin {
        #[command(subcommand)]
        command: PluginCommands,
    },
}

#[derive(Subcommand)]
enum BatonCommands {
    /// Hand off work to another session by generating a ledger-grounded baton packet
    Pass {
        /// Passing session ID
        #[arg(short, long)]
        session: String,

        /// Target receiving session ID (omit to broadcast)
        #[arg(short, long)]
        to: Option<String>,

        /// Passing agent name
        #[arg(short, long, default_value = "agent")]
        agent: String,

        /// Next step guidance or handover note
        #[arg(short, long)]
        note: Option<String>,

        /// Optional project slug or ID
        #[arg(short, long)]
        project: Option<String>,
    },

    /// Pick up the latest handoff baton packet (direct first, then broadcast)
    Pickup {
        /// Receiving session ID
        #[arg(short, long)]
        session: Option<String>,

        /// Optional project slug or ID
        #[arg(short, long)]
        project: Option<String>,
    },
}

#[derive(Subcommand)]
enum DecisionCommands {
    /// Record an architectural decision (supersedes prior decision on same topic)
    Record {
        /// Architectural topic or domain (e.g. auth, cache, db)
        #[arg(short, long)]
        topic: String,

        /// Exact decision made
        #[arg(short, long)]
        decision: String,

        /// Technical rationale and justification
        #[arg(short, long)]
        reason: String,

        /// Considered alternatives that were rejected
        #[arg(short, long)]
        alternatives: Vec<String>,

        /// Dissenting opinions or known trade-offs
        #[arg(long)]
        dissent: Option<String>,

        /// Agent or human recording the decision
        #[arg(long, default_value = "human")]
        agent: String,

        /// Affected symbols or code modules
        #[arg(short, long)]
        symbols: Vec<String>,

        /// Supporting evidence or ADR references
        #[arg(short, long)]
        evidence: Vec<String>,

        /// Optional project slug or ID
        #[arg(short, long)]
        project: Option<String>,
    },

    /// Get the active decision and preserved history/dissent for a topic
    Get {
        /// Topic to query (e.g. auth)
        topic: String,

        /// Optional project slug or ID
        #[arg(short, long)]
        project: Option<String>,
    },

    /// List recorded architectural decisions
    List {
        /// Optional project slug or ID
        #[arg(short, long)]
        project: Option<String>,

        /// Optional status filter (active, superseded)
        #[arg(short, long)]
        status: Option<String>,

        /// Limit number of decisions returned
        #[arg(short, long, default_value_t = 20)]
        limit: usize,
    },
}

#[derive(Subcommand)]
enum ProposalCommands {
    /// Show aggregated summary of proposals by status and type
    Summary {
        /// Project slug or ID filter
        #[arg(short, long)]
        project: Option<String>,
    },
    /// List review proposals with optional filtering
    List {
        /// Status filter (pending, approved, rejected, all)
        #[arg(short, long, default_value = "pending")]
        status: String,

        /// Proposal type filter (e.g. duplicate_merge, tool_execution_review)
        #[arg(short = 't', long)]
        proposal_type: Option<String>,

        /// Maximum number of proposals to display
        #[arg(short, long, default_value_t = 20)]
        limit: usize,

        /// Pagination offset
        #[arg(short, long, default_value_t = 0)]
        offset: usize,

        /// Project slug or ID filter
        #[arg(short, long)]
        project: Option<String>,
    },
    /// Approve or reject a specific review proposal
    Decide {
        /// Proposal ID
        id: String,

        /// Approve the proposal
        #[arg(long, default_value_t = true)]
        approve: bool,

        /// Reject the proposal
        #[arg(long)]
        reject: bool,
    },
    /// Approve all pending proposals (and automatically archive duplicate memories)
    ApproveAll {
        /// Optional proposal type filter (e.g. duplicate_merge)
        #[arg(short = 't', long)]
        proposal_type: Option<String>,

        /// Project slug or ID filter
        #[arg(short, long)]
        project: Option<String>,
    },
    /// Reject all pending proposals
    RejectAll {
        /// Optional proposal type filter (e.g. duplicate_merge)
        #[arg(short = 't', long)]
        proposal_type: Option<String>,

        /// Project slug or ID filter
        #[arg(short, long)]
        project: Option<String>,
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
        #[arg(action = clap::ArgAction::Set)]
        enabled: bool,
    },
    /// Set the human password guarding safety writes (rule/hooks/mode changes)
    Passwd {
        /// Read the new password from AUTOMEMORY_PASSWORD instead of prompting
        #[arg(long, default_value_t = false)]
        from_env: bool,
    },
}

#[derive(Subcommand)]
enum EvalCommands {
    /// Run all local suites; exit 0 if every suite passes, 1 otherwise
    Run {
        /// Emit the full report as JSON
        #[arg(long, default_value_t = false)]
        json: bool,
    },
}

#[derive(Subcommand)]
enum HooksCommands {
    /// List merged pipeline handlers (builtin defaults + hooks.toml layers)
    List,
    /// Dry-run the pipeline for an event with piped JSON input (no side effects)
    Test {
        /// Canonical or CLI event name (e.g. pre-tool, pre-commit)
        #[arg(long)]
        event: String,
        /// JSON input: {"tool","paths","symbols","mutation","session_id","agent"}
        #[arg(long)]
        input: String,
    },
    /// Enable a handler id in the project hooks.toml
    Enable {
        /// Handler id
        id: String,
    },
    /// Disable a handler id in the project hooks.toml
    Disable {
        /// Handler id
        id: String,
    },
    /// Set a handler's priority in the project hooks.toml (lower runs first)
    Move {
        /// Handler id
        id: String,
        /// New priority
        priority: u8,
    },
    /// Save real hook payloads (secrets removed) to turn into contract fixtures: on, off or status
    Record {
        mode: String,
    },
}

#[derive(Subcommand)]
enum PluginCommands {
    /// List the 16 versioned ports and any installed plugin manifests
    List,
    /// Install a plugin manifest (validates ports + api_version first)
    Install {
        /// Path to the plugin manifest (.toml)
        path: String,
    },
    /// Update an installed plugin from a manifest path
    Update {
        /// Path to the plugin manifest (.toml)
        path: String,
    },
    /// Pin an installed plugin version so updates refuse to replace it
    Pin {
        /// Plugin name
        name: String,
    },
    /// Roll back an installed plugin to its pre-update manifest
    Rollback {
        /// Plugin name
        name: String,
    },
}

#[derive(Subcommand)]
enum AdapterCommands {
    /// List registered agent adapters and their hook coverage
    List,
    /// Re-run an adapter's installer to refresh hook/event mappings
    Update {
        /// Agent id or alias (see `adapter list`)
        agent: String,

        /// Apply configuration globally rather than to current workspace
        #[arg(short, long)]
        global: bool,
    },
}

#[derive(Subcommand)]
enum GuardrailCommands {
    /// Evaluate a shell command against guardrail rules without executing it (exit 0 allow, 1 warn, 2 block)
    Test {
        /// Shell command to evaluate
        #[arg(long)]
        command: String,

        /// Autopilot mode (vibe, strict, review) [default: vibe]
        #[arg(long)]
        mode: Option<String>,

        /// Project slug for project-scoped rules and failure matching
        #[arg(long)]
        project: Option<String>,
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
    /// Read a specific memory record in detail
    Get {
        /// Memory ID
        id: String,
    },

    /// List memories
    List {
        #[arg(short, long)]
        project: Option<String>,

        #[arg(short, long, default_value_t = 20)]
        limit: usize,

        /// Group output by field ('type' or 'scope')
        #[arg(short = 'g', long)]
        group_by: Option<String>,
    },

    /// Generate an activity and progress digest for a specific time span
    Digest {
        /// Time span window (e.g. 15m, 30m, 1h, 2h, 4h, 24h, today) [default: 1h]
        #[arg(short, long)]
        span: Option<String>,

        /// Optional project filter
        #[arg(short, long)]
        project: Option<String>,

        /// Persist synthesized digest as a durable milestone checkpoint memory
        #[arg(long, default_value_t = false)]
        save: bool,
    },

    /// Search memories using FTS5 or zero-knowledge semantic vector search
    Search {
        /// Search query
        query: String,

        #[arg(short, long)]
        project: Option<String>,

        #[arg(short, long, default_value_t = 10)]
        limit: usize,

        /// Enable zero-knowledge semantic vector search (Cloudflare Vectorize ANN)
        #[arg(short = 's', long)]
        semantic: bool,
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

        /// Knowledge domain category (architecture, bugfix, feature, decision, lesson)
        #[arg(short = 'C', long)]
        category: Option<String>,

        /// Specific achievements reached
        #[arg(long = "achieved")]
        achieved: Vec<String>,

        /// Specific failures or blockers resolved
        #[arg(long = "failed")]
        failed: Vec<String>,

        /// Comma-separated list of affected file paths
        #[arg(long = "files")]
        files: Option<String>,
    },

    /// Archive a memory
    Archive {
        /// Memory ID
        id: String,
    },
}

#[derive(Subcommand)]
enum SyncCommands {
    /// One sync pass through the git remote: pull, apply, write, push (hooks start this in the background)
    Auto {
        /// Only pull
        #[arg(long)]
        pull: bool,

        /// Only push
        #[arg(long)]
        push: bool,
    },

    /// Set the private git repository that carries your memory between machines
    Remote {
        /// Git URL (any private repository you can push to; a bare repo path also works)
        #[arg(default_value = "")]
        url: String,

        /// Clear/remove the configured remote URL
        #[arg(long)]
        clear: bool,
    },

    /// Set up this machine from an existing vault: remote, recovery phrase, first sync, agents
    Bootstrap {
        /// The git remote used on your other machines
        #[arg(long)]
        remote: String,

        /// Recovery phrase from `automemory sync pair` on your first machine
        #[arg(long)]
        phrase: Option<String>,

        /// Do not connect Antigravity, Claude Code and Codex
        #[arg(long)]
        no_connect: bool,
    },

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

        /// Custom encryption key passphrase override
        #[arg(short, long)]
        key: Option<String>,
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

        /// Custom encryption key passphrase override
        #[arg(short, long)]
        key: Option<String>,
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

        /// Custom encryption key passphrase override
        #[arg(short, long)]
        key: Option<String>,
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

    /// Secure zero-knowledge multi-device pairing via BIP-39 mnemonic phrase and QR code
    Pair {
        /// Join an existing vault using a 12-word recovery seed phrase
        #[arg(long)]
        join: Option<String>,

        /// Cloud endpoint URL
        #[arg(short, long)]
        endpoint: Option<String>,

        /// Auth token or device token
        #[arg(short, long)]
        token: Option<String>,
    },

    /// Watch trackable files in background and automatically sync on change
    Watch {
        /// Project slug (defaults to current project)
        #[arg(short, long)]
        project: Option<String>,

        /// Polling interval in seconds (default: 5)
        #[arg(short, long, default_value_t = 5)]
        interval: u64,
    },
}

#[tokio::main]
async fn main() -> Result<()> {
    // In dev / debug mode (cargo run / debug builds), enable full debug & info logs.
    // In production / release builds, enable only critical error logging by default.
    let filter = if cfg!(debug_assertions) {
        tracing_subscriber::EnvFilter::try_from_default_env().unwrap_or_else(|_| {
            tracing_subscriber::EnvFilter::new("automemory=debug,tower_http=info")
        })
    } else {
        tracing_subscriber::EnvFilter::try_from_default_env()
            .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("automemory=error"))
    };

    let _ = tracing_subscriber::fmt()
        .with_env_filter(filter)
        .with_writer(std::io::stderr)
        .with_target(false)
        .compact()
        .try_init();

    let cli = Cli::parse();

    maybe_print_tour_hint(&cli.command);

    match cli.command {
        Commands::Init { agent } => {
            commands::init::execute(Some(agent))?;
        }
        Commands::Connect { agent, global } => {
            commands::connect::execute(&agent, global)?;
        }
        Commands::Disconnect {
            agent,
            global,
            everywhere,
            dry_run,
        } => {
            commands::disconnect::execute(&agent, global, everywhere, dry_run)?;
        }
        Commands::Link { project_id } => {
            commands::link::execute(project_id)?;
        }
        Commands::Hook {
            agent,
            event,
            project,
        } => {
            commands::hook::run(&agent, &event, &project);
        }
        Commands::Doctor => {
            commands::doctor::execute()?;
        }
        Commands::Next => {
            commands::learn::next()?;
        }
        Commands::Tour {
            auto,
            resume,
            reset,
            step,
            no_color,
        } => {
            commands::learn::tour(auto, resume, reset, step, no_color)?;
        }
        Commands::Learn { topic } => {
            commands::learn::learn(topic.as_deref())?;
        }
        Commands::Demo { remove } => {
            commands::learn::demo(remove)?;
        }
        Commands::Completions { shell } => {
            use clap::CommandFactory;
            let mut cmd = Cli::command();
            let bin_name = cmd.get_name().to_string();
            clap_complete::generate(shell, &mut cmd, bin_name, &mut std::io::stdout());
        }
        Commands::Serve { port, db } => {
            commands::serve::execute(port, db).await?;
        }
        Commands::Mcp { db } => {
            commands::mcp::execute(db).await?;
        }
        Commands::Project { command } => match command {
            ProjectCommands::List => commands::project::list()?,
            ProjectCommands::Register {
                name,
                path,
                track,
                vault_repo,
                restore_to_workspace,
            } => {
                commands::project::register(name, path, track, vault_repo, restore_to_workspace)?;
            }
            ProjectCommands::Remove { name } => {
                commands::project::remove(&name)?;
            }
        },
        Commands::Memory { command } => match command {
            MemoryCommands::Get { id } => {
                commands::memory::get(&id)?;
            }
            MemoryCommands::List {
                project,
                limit,
                group_by,
            } => {
                commands::memory::list(project, limit, group_by)?;
            }
            MemoryCommands::Digest {
                span,
                project,
                save,
            } => {
                commands::memory::digest(span, project, save)?;
            }
            MemoryCommands::Search {
                query,
                project,
                limit,
                semantic,
            } => {
                commands::memory::search(&query, project, limit, semantic).await?;
            }
            MemoryCommands::Save {
                summary,
                content,
                mem_type,
                scope,
                project,
                category,
                achieved,
                failed,
                files,
            } => {
                commands::memory::save(commands::memory::MemorySaveArgs {
                    content,
                    summary,
                    mem_type,
                    scope,
                    project_id: project,
                    category,
                    achieved,
                    failed,
                    files,
                })?;
            }
            MemoryCommands::Archive { id } => {
                commands::memory::archive(&id)?;
            }
        },
        Commands::Sync { command } => match command {
            SyncCommands::Auto { pull, push } => commands::sync::auto(pull, push)?,
            SyncCommands::Remote { url, clear } => {
                let target = if clear { "--clear" } else { &url };
                commands::sync::remote(target)?;
            }
            SyncCommands::Bootstrap { remote, phrase, no_connect } => {
                commands::sync::bootstrap(&remote, phrase, !no_connect)?
            }
            SyncCommands::Init {
                project,
                endpoint,
                token,
            } => {
                commands::sync::init(project, endpoint, token)?;
            }
            SyncCommands::Status { project } => {
                commands::sync::status(project)?;
            }
            SyncCommands::Push {
                project,
                endpoint,
                token,
                key,
            } => {
                commands::sync::push(project, endpoint, token, key).await?;
            }
            SyncCommands::Pull {
                project,
                endpoint,
                token,
                key,
            } => {
                commands::sync::pull(project, endpoint, token, key).await?;
            }
            SyncCommands::Restore {
                project,
                from_file,
                endpoint,
                token,
                key,
            } => {
                commands::sync::restore(project, from_file, endpoint, token, key).await?;
            }
            SyncCommands::Export { path } => commands::sync::export_vault(path)?,
            SyncCommands::Import { path } => commands::sync::import_vault(path)?,
            SyncCommands::Pair {
                join,
                endpoint,
                token,
            } => {
                commands::sync::pair(join, endpoint, token).await?;
            }
            SyncCommands::Watch { project, interval } => {
                commands::sync::watch(project, interval).await?;
            }
        },
        Commands::Mode { command } => match command {
            ModeCommands::Get { project } => commands::mode::get(project)?,
            ModeCommands::Set { mode, project } => commands::mode::set(&mode, project)?,
        },
        Commands::Guardrail { command } => match command {
            GuardrailCommands::Test {
                command,
                mode,
                project,
            } => commands::guardrail::test_command(&command, mode, project)?,
        },
        Commands::Eval { command } => match command {
            EvalCommands::Run { json } => commands::eval::run(json)?,
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
            } => commands::rule::add(
                name,
                pattern,
                critical,
                action,
                description,
                mitigation,
                project,
            )?,
            RuleCommands::Remove { id } => commands::rule::remove(&id)?,
            RuleCommands::Toggle { id, enabled } => commands::rule::toggle(&id, enabled)?,
            RuleCommands::Passwd { from_env } => commands::rule::passwd(from_env)?,
        },
        Commands::Skill { command } => match command {
            SkillCommands::List { project } => commands::skill::list(project)?,
            SkillCommands::Export {
                project,
                target,
                min_runs,
            } => {
                commands::skill::export(project, target, min_runs)?;
            }
        },
        Commands::Proposal { command } => match command {
            ProposalCommands::Summary { project } => commands::proposal::summary(project)?,
            ProposalCommands::List {
                status,
                proposal_type,
                limit,
                offset,
                project,
            } => commands::proposal::list(status, proposal_type, limit, offset, project)?,
            ProposalCommands::Decide {
                id,
                approve,
                reject,
            } => {
                let final_approval = if reject { false } else { approve };
                commands::proposal::decide(id, final_approval)?;
            }
            ProposalCommands::ApproveAll {
                proposal_type,
                project,
            } => {
                commands::proposal::approve_all(proposal_type, project)?;
            }
            ProposalCommands::RejectAll {
                proposal_type,
                project,
            } => {
                commands::proposal::reject_all(proposal_type, project)?;
            }
        },
        Commands::Note { text, kind } => commands::continuity::note(&text, &kind)?,
        Commands::ExportRules { agent, all, check } => commands::rules_export::execute(agent, all, check)?,
        Commands::Embeddings { command } => match command {
            EmbeddingsCommands::Status => commands::embeddings::status()?,
            EmbeddingsCommands::Test => commands::embeddings::test()?,
            EmbeddingsCommands::Reindex { all } => commands::embeddings::reindex(all)?,
            EmbeddingsCommands::Use { provider, model, endpoint, api_key_env, allow_network } => {
                commands::embeddings::use_provider(provider, model, endpoint, api_key_env, allow_network)?
            }
            EmbeddingsCommands::Off => commands::embeddings::off()?,
            EmbeddingsCommands::Key { command } => match command {
                EmbeddingsKeyCommands::Set { profile, stdin, from_env } => commands::embeddings::key_set(profile, stdin, from_env)?,
                EmbeddingsKeyCommands::Status { profile } => commands::embeddings::key_status(profile)?,
                EmbeddingsKeyCommands::Remove { profile } => commands::embeddings::key_remove(profile)?,
            },
            EmbeddingsCommands::Profile { command } => match command {
                EmbeddingsProfileCommands::List => commands::embeddings::profile_list()?,
                EmbeddingsProfileCommands::Add { name, preset, provider, model, endpoint, dimensions, allow_network } => {
                    commands::embeddings::profile_add(name, preset, provider, model, endpoint, dimensions, allow_network)?
                }
                EmbeddingsProfileCommands::Use { name } => commands::embeddings::profile_use(name)?,
                EmbeddingsProfileCommands::Remove { name } => commands::embeddings::profile_remove(name)?,
            },
            EmbeddingsCommands::Models => commands::embeddings::models()?,
            EmbeddingsCommands::Clean => commands::embeddings::clean()?,
            EmbeddingsCommands::Eval { file } => commands::embeddings::eval(file)?,
        },
        Commands::Llm { command } => match command {
            LlmCommands::Status => commands::llm::status()?,
            LlmCommands::Use { model, endpoint, api_key_env, allow_network } => commands::llm::use_model(model, endpoint, api_key_env, allow_network)?,
            LlmCommands::Off => commands::llm::off()?,
            LlmCommands::KeySet { profile, stdin, from_env } => commands::llm::key_set(profile, stdin, from_env)?,
            LlmCommands::KeyRemove { profile } => commands::llm::key_remove(profile)?,
            LlmCommands::Profile { command } => match command {
                LlmProfileCommands::Add { name, model, endpoint, api_key_env, allow_network } => commands::llm::profile_add(name, model, endpoint, api_key_env, allow_network)?,
                LlmProfileCommands::Use { name } => commands::llm::profile_use(name)?,
                LlmProfileCommands::List => commands::llm::profile_list()?,
                LlmProfileCommands::Remove { name } => commands::llm::profile_remove(name)?,
            },
            LlmCommands::Run { what } => commands::llm::run(&what)?,
        },
        Commands::Continuity { command } => match command {
            ContinuityCommands::Show { all } => commands::brief::show(all)?,
            ContinuityCommands::Add { kind, text, reason } => commands::brief::add(&kind, &text, reason)?,
            ContinuityCommands::Edit { id, text, reason } => commands::brief::edit(&id, &text, reason)?,
            ContinuityCommands::Resolve { id } => commands::brief::set_status(&id, automemory_core::continuity::ItemStatus::Resolved)?,
            ContinuityCommands::Remove { id } => commands::brief::set_status(&id, automemory_core::continuity::ItemStatus::Removed)?,
            ContinuityCommands::Confirm { id } => commands::brief::confirm(&id)?,
            ContinuityCommands::Forget { all } => commands::brief::forget(all)?,
        },
        Commands::Room { command } => match command {
            RoomCommands::List => commands::room::list()?,
            RoomCommands::Show { id } => commands::room::show(&id)?,
            RoomCommands::Approve { id } => commands::room::approve(&id)?,
            RoomCommands::Reject { id } => commands::room::reject(&id)?,
            RoomCommands::Again { id } => commands::room::again(&id)?,
        },
        Commands::Backup { command } => match command.unwrap_or(BackupCommands::Create) {
            BackupCommands::Create => commands::backup::create()?,
            BackupCommands::List => commands::backup::list()?,
            BackupCommands::Restore { file, yes } => commands::backup::restore(&file, yes)?,
        },
        Commands::Stats { days, json } => commands::stats::run(days, json)?,
        Commands::Update { check, yes } => commands::update::run(check, yes)?,
        Commands::Resume => commands::continuity::resume()?,
        Commands::Handoff { note, message } => commands::continuity::handoff(note.or(message)).await?,
        Commands::Locks { project, coverage: true, days } => {
            let _ = project;
            commands::lock::coverage(days)?
        }
        Commands::Locks { project, .. } => commands::lock::list_locks(project.as_deref())?,
        Commands::Overlap { base, branch } => commands::lock::overlap(&base, branch.as_deref())?,
        Commands::Unlock {
            scope,
            project,
            force,
        } => commands::lock::unlock(&scope, project.as_deref(), force)?,
        Commands::Decision { command } => match command {
            DecisionCommands::Record {
                topic,
                decision,
                reason,
                alternatives,
                dissent,
                agent,
                symbols,
                evidence,
                project,
            } => {
                commands::decision::record_decision(commands::decision::DecisionRecordCliArgs {
                    topic: &topic,
                    decision: &decision,
                    reason: &reason,
                    alternatives: &alternatives,
                    dissent: dissent.as_deref(),
                    agent: &agent,
                    symbols: &symbols,
                    evidence: &evidence,
                    project_arg: project.as_deref(),
                })?;
            }
            DecisionCommands::Get { topic, project } => {
                commands::decision::get_decision(&topic, project.as_deref())?;
            }
            DecisionCommands::List {
                project,
                status,
                limit,
            } => {
                commands::decision::list_decisions(project.as_deref(), status.as_deref(), limit)?;
            }
        },
        Commands::Pin { id } => {
            commands::governance::pin_memory(&id)?;
        }
        Commands::Veto { id, reason } => {
            commands::governance::veto_memory(&id, reason.as_deref())?;
        }
        Commands::Revoke { id, session, agent } => {
            commands::governance::revoke_memories(
                id.as_deref(),
                session.as_deref(),
                agent.as_deref(),
            )?;
        }
        Commands::Baton { command } => match command {
            BatonCommands::Pass {
                session,
                to,
                agent,
                note,
                project,
            } => {
                commands::baton::pass(
                    &session,
                    to.as_deref(),
                    &agent,
                    note.as_deref(),
                    project.as_deref(),
                )?;
            }
            BatonCommands::Pickup { session, project } => {
                commands::baton::pickup(session.as_deref(), project.as_deref())?;
            }
        },
        Commands::Stop { project } => {
            commands::lock::stop_all(project.as_deref())?;
        }
        Commands::Hooks { command } => match command {
            HooksCommands::List => commands::hooks_admin::list()?,
            HooksCommands::Test { event, input } => commands::hooks_admin::test(&event, &input)?,
            HooksCommands::Enable { id } => {
                commands::hooks_admin::set_field(&id, |h| h.enabled = true)?;
            }
            HooksCommands::Disable { id } => {
                commands::hooks_admin::set_field(&id, |h| h.enabled = false)?;
            }
            HooksCommands::Record { mode } => commands::hooks_admin::record(&mode)?,
            HooksCommands::Move { id, priority } => {
                commands::hooks_admin::set_field(&id, |h| h.priority = priority)?;
            }
        },
        Commands::Plugin { command } => match command {
            PluginCommands::List => commands::plugin::list()?,
            PluginCommands::Install { path } => commands::plugin::install(&path)?,
            PluginCommands::Update { path } => commands::plugin::update(&path)?,
            PluginCommands::Pin { name } => commands::plugin::pin(&name)?,
            PluginCommands::Rollback { name } => commands::plugin::rollback(&name)?,
        },
        Commands::Adapter { command } => match command {
            AdapterCommands::List => commands::adapter::list()?,
            AdapterCommands::Update { agent, global } => {
                commands::adapter::update(&agent, global)?;
            }
        },
    }

    Ok(())
}

#[derive(Subcommand)]
enum ContinuityCommands {
    /// What the next session will be told, with item ids
    Show {
        /// Include resolved and removed items
        #[arg(long)]
        all: bool,
    },
    /// Add an item: goal, status, constraint, decision, rejected, open, next, file, glossary, preference
    Add {
        kind: String,
        text: String,
        /// Why (for ruled-out options and decisions)
        #[arg(long)]
        reason: Option<String>,
    },
    /// Change an item's wording (agents can no longer change it)
    Edit {
        id: String,
        text: String,
        #[arg(long)]
        reason: Option<String>,
    },
    /// An open question answered or a step done
    Resolve { id: String },
    /// Delete an item
    Remove { id: String },
    /// Accept a draft item
    Confirm { id: String },
    /// Delete the whole brief of this project
    Forget {
        /// Every project, not only this folder's
        #[arg(long)]
        all: bool,
    },
}

#[derive(Subcommand)]
enum RoomCommands {
    /// Every room and what it waits for
    List,
    /// The discussion and, for a decided room, what approving would store
    Show { id: String },
    /// Approve the plan: records the decision and one task per step
    Approve { id: String },
    /// Reject the plan: nothing is stored
    Reject { id: String },
    /// Send a decided room back for another critique and revision
    Again { id: String },
}

#[derive(Subcommand)]
enum BackupCommands {
    /// Save a copy now (the default)
    Create,
    /// Show saved copies and whether each one opens
    List,
    /// Replace the database with a saved copy (the current one is kept)
    Restore {
        file: String,
        /// Do not ask
        #[arg(long, short = 'y')]
        yes: bool,
    },
}

#[derive(Subcommand)]
enum LlmProfileCommands {
    /// Save a setup under a name
    Add {
        name: String,
        #[arg(long)]
        model: String,
        #[arg(long)]
        endpoint: Option<String>,
        #[arg(long)]
        api_key_env: Option<String>,
        #[arg(long)]
        allow_network: bool,
    },
    /// Use a saved setup (`default` is the original one)
    Use { name: String },
    /// Show saved setups; * marks the one in use
    List,
    /// Delete a setup and its key
    Remove { name: String },
}

#[derive(Subcommand)]
enum LlmCommands {
    /// Whether it is on, where it sends text, today's tokens
    Status,
    /// Turn it on with a model (any OpenAI-compatible chat endpoint, Ollama's /v1 included)
    Use {
        model: String,
        /// Base URL, for example http://127.0.0.1:11434/v1
        #[arg(long)]
        endpoint: Option<String>,
        /// Name of the environment variable holding the API key
        #[arg(long)]
        api_key_env: Option<String>,
        /// Allow note text to be sent to a non-local endpoint
        #[arg(long)]
        allow_network: bool,
    },
    /// Turn it off again
    Off,
    /// Store the API key in the OS keychain (hidden prompt, --stdin or --from-env VAR)
    KeySet {
        /// Profile the key belongs to (default: the active one)
        #[arg(long)]
        profile: Option<String>,
        #[arg(long)]
        stdin: bool,
        #[arg(long)]
        from_env: Option<String>,
    },
    /// Remove the stored key
    KeyRemove {
        #[arg(long)]
        profile: Option<String>,
    },
    /// Saved setups (for example a free local model and a cloud one) to switch between
    Profile {
        #[command(subcommand)]
        command: LlmProfileCommands,
    },
    /// Run a job now: pairs (conflicts and duplicates) or distill (candidate notes from sessions)
    Run { what: String },
}

#[derive(Subcommand)]
enum EmbeddingsCommands {
    /// Mode, provider and how many memories are indexed
    Status,
    /// Embed one sentence with the configured provider
    Test,
    /// Embed memories that have no vector yet (--all: recompute every one)
    Reindex {
        #[arg(long)]
        all: bool,
    },
    /// Switch to hybrid recall with a provider: ngram, ollama, openai, exec
    Use {
        provider: String,
        #[arg(long)]
        model: Option<String>,
        /// Base URL, or the command for `exec`
        #[arg(long)]
        endpoint: Option<String>,
        /// Name of the environment variable holding the API key
        #[arg(long)]
        api_key_env: Option<String>,
        /// Allow memory text to be sent to a non-local endpoint
        #[arg(long)]
        allow_network: bool,
    },
    /// Back to keyword recall
    Off,
    /// API key for a cloud profile, kept in the OS keychain
    Key {
        #[command(subcommand)]
        command: EmbeddingsKeyCommands,
    },
    /// Saved provider setups (switch between local and cloud)
    Profile {
        #[command(subcommand)]
        command: EmbeddingsProfileCommands,
    },
    /// Ready-made models, and what your local Ollama has installed
    Models,
    /// Remove vectors of models that are no longer in use
    Clean,
    /// Compare keyword and hybrid on labelled queries (JSON file)
    Eval { file: std::path::PathBuf },
}

fn maybe_print_tour_hint(command: &Commands) {
    match command {
        Commands::Tour { .. }
        | Commands::Hook { .. }
        | Commands::Mcp { .. }
        | Commands::Completions { .. }
        | Commands::Serve { .. } => return,
        _ => {}
    }
    let config = automemory_core::config::Config::default_config();
    if let Ok(db) = automemory_core::db::Database::open(&config.db_path) {
        if db.sync_kv_get("tour_hint_shown").is_none() {
            let _ = db.sync_kv_set("tour_hint_shown", "true");
            eprintln!("New here? Run `automemory tour` (5 min)");
        }
    }
}

#[derive(Subcommand)]
enum EmbeddingsKeyCommands {
    /// Store the key (hidden prompt; or --stdin; or --from-env VAR)
    Set {
        /// Profile name (default: the active one)
        profile: Option<String>,
        #[arg(long)]
        stdin: bool,
        #[arg(long)]
        from_env: Option<String>,
    },
    /// Where the key comes from (never prints it)
    Status { profile: Option<String> },
    /// Delete the stored key
    Remove { profile: Option<String> },
}

#[derive(Subcommand)]
enum EmbeddingsProfileCommands {
    /// List profiles and key status
    List,
    /// Save a profile from a preset (`embeddings models`) and/or your own values
    Add {
        name: String,
        #[arg(long)]
        preset: Option<String>,
        #[arg(long)]
        provider: Option<String>,
        #[arg(long)]
        model: Option<String>,
        #[arg(long)]
        endpoint: Option<String>,
        #[arg(long)]
        dimensions: Option<usize>,
        /// Allow memory text to be sent to a non-local endpoint
        #[arg(long)]
        allow_network: bool,
    },
    /// Switch to a profile and turn hybrid recall on
    Use { name: String },
    /// Delete a profile and its stored key
    Remove { name: String },
}
