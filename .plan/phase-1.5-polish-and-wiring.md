# Phase 1.5: Daemon Automation & Host Wiring (Completed)

## Goal
Transform AutoMemory Pro into a fully autonomous, hands-free background system with automated document tracking, debounced Git vault sync, pluggable LLM extraction, and one-command host configuration.

---

## Deliverables

### 1. Document Scanner & Live File Watcher (`notify`)
- **Location**: `crates/automemory-core/src/docs/`
- **Features**:
  - Automatically scans and tracks `PRD.md`, `docs/architecture.md`, `docs/decisions/**`, `.cursor/rules/**`, `AGENTS.md`, `CLAUDE.md`.
  - Computes SHA256 hashes on save, creating versioned `document_revisions` in SQLite.
  - Detects credentials with `contains_secrets` before vault export to prevent secret leakage.
  - Watches project roots recursively using `notify::RecommendedWatcher` with debounce.
  - Automatically mirrors tracked docs into the vault (`projects/<slug>/docs/`).

### 2. Automated Git Vault Sync Daemon (`AutoSyncDaemon`)
- **Location**: `crates/automemory-core/src/sync/daemon.rs`
- **Features**:
  - Background loop in `automemory serve` with a configurable 30-second debounce timer.
  - Triggers on tool activity, `task.completed`, and session `stop`.
  - Commits diffable Markdown and JSONL text artifacts to local git repo and pushes to GitHub remote.

### 3. Pluggable LLM Extraction & Consolidation Engine
- **Location**: `crates/automemory-core/src/extraction/`
- **Features**:
  - `LlmExtractor`: Supports **Local Ollama** (e.g. `llama3.2`, `qwen2.5-coder` - 100% offline, zero cloud cost), OpenAI, and Anthropic APIs.
  - `ConsolidationWorker`: Periodically merges duplicate memories, resolves contradictions via supersession, and queues review proposals (`review_proposals`).

### 4. Multi-Host Connect Command (`automemory connect`)
- **Location**: `crates/automemory-cli/src/commands/connect.rs`
- **Features**:
  - `automemory connect agy`: Auto-wires `.agents/mcp_config.json`, `.agents/hooks.json`, `.mcp.json`, and `.agents/plugins/automemory/`.
  - `automemory connect claude`: Configures Claude Code in `~/.claude.json`.
  - `automemory connect cursor`: Configures Cursor in `.cursor/mcp.json`.

### 5. Antigravity Hook Contract Strict Compliance
- `PreToolUse` returns `{"decision": "allow"}` in `<2ms`.
- Lifecycle hooks (`PreInvocation`, `PostToolUse`, `Stop`) return `{}`.
- Proper grouped format with `matcher: "*"` in `hooks.json`.
