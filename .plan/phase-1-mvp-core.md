# Phase 1: Core MVP Engine (Completed)

## Goal
Build the foundational persistent memory engine in native Rust, providing a durable SQLite WAL store with FTS5 search, out-of-process lifecycle hooks (<5ms), 30+ MCP tools, an asynchronous extraction worker, and an embedded Lamborghini-inspired dark web dashboard.

---

## Deliverables

### 1. Multi-Crate Rust Workspace
- `crates/automemory-core`: Data layer, schema migrations, models, secret redaction, retrieval, extraction, and vault managers.
- `crates/automemory-mcp`: MCP JSON-RPC 2.0 protocol engine with dual stdio and HTTP transports.
- `crates/automemory-cli`: Unified binary `automemory` with commands `init`, `hook`, `doctor`, `serve`, `mcp`, `project`, `memory`, `sync`.

### 2. Storage & Search
- SQLite with `PRAGMA journal_mode = WAL`, `busy_timeout = 5000`, `foreign_keys = ON`, `synchronous = NORMAL`.
- Virtual table `memories_fts` using SQLite FTS5 unicode61 tokenizer with automatic insert/update/delete triggers.
- Versioned memories with prior version retention (`memory_versions`), supersession tracking, and archive/restore workflows.

### 3. Out-of-Process Hook (<2ms)
- `automemory hook --agent agy --event <event> --project auto`
- Normalizes Antigravity tool steps (`view_file -> read`, `replace_file_content -> edit`, `run_command -> command`).
- Emits `{}` on stdout and exits immediately with code 0 without stalling agent turns.

### 4. Embedded Web Dashboard
- Served via `automemory serve --port 4545`.
- Zero Node.js/web server install needed; assets bundled directly into binary using `rust-embed`.
- Lamborghini dark aesthetic: `#000000` canvas, `#FFC000` gold accents, `#1E1E1E` charcoal panels, sharp 0px geometry.
