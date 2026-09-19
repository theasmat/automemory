# AutoMemory Pro

[![License](https://img.shields.io/badge/license-MIT%20OR%20Apache--2.0-blue.svg)](LICENSE)
[![Rust](https://img.shields.io/badge/rust-1.80%2B-orange.svg)](https://www.rust-lang.org)

**AutoMemory Pro** is an open-source, local-first persistent memory platform for AI coding agents built in Rust. It automatically captures useful task evidence, tool outcomes, decisions, errors, preferences, project documents, and successful procedures without blocking agents. In a new chat, it retrieves only the relevant bounded context for the current project plus user-level preferences, so agents can continue work without re-reading or re-discovering the entire codebase.

---

## Key Features

- 🦀 **Pure Native Rust**: Zero Node.js/Python runtime overhead. Lightning-fast startup and execution.
- ⚡ **Ultra-Fast Out-of-Process Hooks (<5ms)**: Asynchronously appends events to an append-only ledger; never blocks coding agents.
- 🗄️ **Local-First SQLite in WAL Mode**: Robust local store with FTS5 full-text indexing, transactions, and migration support.
- 🎯 **Project-Isolated Namespaces**: Prevents cross-project context contamination while supporting global user preferences.
- 🔁 **First-Class Success Traces & Failure Records**: Automatically tracks repeatable procedures and records root-cause hypotheses with verified fixes.
- 🔒 **Tamper-Evident Redaction Engine**: Strips API keys, JWTs, credentials, private keys, and `.env` secrets before persistence or sync.
- 🌐 **MCP Server (Stdio & HTTP Transport)**: Implements 30+ Model Context Protocol tools for reading, writing, and governance.
- 🔄 **Text-First GitHub Vault Sync**: Clean diffable Markdown and JSONL synchronization for user-owned private repositories.
- 🏎️ **Embedded Dashboard UI**: Integrated dark web dashboard inspired by Lamborghini styling (pure black `#000000`, Lamborghini gold `#FFC000`, 0px border-radius).

---

## Architecture

```
┌─────────────────────────────────────────────────────────────────┐
│                  Coding Agent (Antigravity, Claude)             │
└────────────────┬───────────────────────────────┬────────────────┘
                 │ Out-of-process Hook (<5ms)    │ MCP (stdio / HTTP)
                 ▼                               ▼
┌──────────────────────────────────┐   ┌──────────────────────────┐
│        automemory hook           │   │      MCP Server          │
│   (Normalizes & appends event)   │   │     (30+ MCP Tools)      │
└────────────────┬─────────────────┘   └─────────▲──────┬─────────┘
                 ▼                               │      │
┌──────────────────────────────────┐             │      │
│  Append-Only SQLite WAL Ledger   │             │      │
└────────────────┬─────────────────┘             │      │
                 ▼                               │      │
┌──────────────────────────────────┐             │      │
│     Async Extraction Worker      │             │      │
│ (Heuristics & Pattern Detection) │             │      │
└────────────────┬─────────────────┘             │      │
                 ▼                               │      │
┌──────────────────────────────────┐             │      │
│ SQLite WAL + FTS5 Database Store ◄─────────────┘      │
│ (Projects, Memories, Traces,     │                    │
│  Failures, Preferences, Docs)    ├────────────────────┤
└────────────────┬─────────────────┘                    │
                 │                                      │
                 ▼                                      ▼
┌──────────────────────────────────┐   ┌──────────────────────────┐
│  Text-First GitHub Vault Sync    │   │ Embedded Web Dashboard   │
│  (Markdown & JSONL Artifacts)    │   │ (Port 4545, Dark Theme)  │
└──────────────────────────────────┘   └──────────────────────────┘
```

---

## Quick Start

### 1. Build from Source

```bash
cargo build --release
```

The resulting binary will be located at `target/release/automemory`.

### 2. Initialize Configuration & Agent Hooks

```bash
automemory init --agent agy
```

This command:
- Creates `~/.automemory_pro/` and `~/.automemory_pro/hooks/`
- Generates `~/.automemory_pro/hooks/agy.json` for Antigravity
- Initializes the local SQLite database in WAL mode
- Registers the current workspace as a project

### 3. Verify Health with Diagnostic Doctor

```bash
automemory doctor
```

```text
=== AutoMemory Pro System Diagnostic (doctor) ===

Checking directories (~/.automemory_pro)... ✓ OK
Checking SQLite database connectivity & WAL mode... ✓ OK (opened in 2.61ms)
Benchmarking FTS5 search & memory operations... ✓ OK (FTS5 search latency: 1.81ms)
  Active memories: 2, Archived: 0
  Pending event queue: 0 unprocessed events
Checking Antigravity hook configuration (agy.json)... ✓ OK

✓ All health checks passed! AutoMemory Pro is fully operational.
```

### 4. Start Background Daemon & Web Dashboard

```bash
automemory serve --port 4545
```

Open `http://127.0.0.1:4545` in your browser to view the real-time Lamborghini-styled memory dashboard!

---

## CLI Reference

| Command | Description |
|---|---|
| `automemory init [--agent agy]` | Initialize configuration, database, and hook files |
| `automemory hook --agent <agent> --event <event> --project auto` | Fast out-of-process lifecycle hook handler (<5ms) |
| `automemory doctor` | Run comprehensive system health check |
| `automemory serve [--port 4545] [--db <path>]` | Start background daemon, HTTP MCP, and Web Dashboard |
| `automemory mcp` | Launch MCP server over stdio for coding agent integration |
| `automemory project list` | List registered projects and local path mappings |
| `automemory project register --slug <slug> --name <name> [--path <path>]` | Register a new project directory |
| `automemory memory list [--project <slug>]` | List active memories |
| `automemory memory search <query> [--project <slug>]` | Search memories using FTS5 full-text search |
| `automemory memory save <summary> --content <content> [-t <type>]` | Save a durable memory explicitly |
| `automemory memory archive <id>` | Archive a memory record |
| `automemory sync export [--path <path>]` | Export database to text-first JSONL & Markdown vault |
| `automemory sync import [--path <path>]` | Import vault records into local database |
| `automemory sync status` | Check vault status |

---

## MCP Server Tools

AutoMemory Pro provides 30+ MCP tools conforming to the Model Context Protocol:

### Read Tools
- `memory.recall`: Retrieve relevant memories for a natural language query
- `memory.context`: Assemble bounded context package for session boot (default 2000 tokens)
- `memory.get`: Read a specific memory record with version history
- `memory.search`: Hybrid search across memories, success traces, and failure records
- `memory.list`: Filter memories by project, type, or status
- `memory.stats`: Get health metrics and memory counts
- `trace.list` & `trace.get`: Inspect repeatable procedures with ordered verification steps
- `failure.list` & `failure.get`: Review past failure records, root-causes, and fixes
- `project.list` & `project.get`: Inspect registered projects
- `doc.list` & `doc.get`: Inspect tracked project documents
- `preference.get`: Read global or project preferences

### Write Tools
- `memory.save`: Explicitly save a memory
- `memory.update`: Update a memory, creating a new version
- `memory.archive` & `memory.restore`: Archive or restore memories
- `trace.save`: Save a reusable procedure
- `failure.save`: Save an error with root-cause hypothesis
- `failure.resolve`: Mark a failure resolved with applied fix and verification
- `preference.set` & `preference.lock`: Set or lock user preferences
- `project.register`: Register a project and path mapping

### Governance Tools
- `memory.proposals.list` & `memory.proposals.decide`: Manage conflict and archive review proposals
- `memory.redact`: Test or apply secret redaction

---

## License

Dual-licensed under either of:
- Apache License, Version 2.0 ([LICENSE-APACHE](LICENSE-APACHE) or http://www.apache.org/licenses/LICENSE-2.0)
- MIT license ([LICENSE-MIT](LICENSE-MIT) or http://opensource.org/licenses/MIT)
