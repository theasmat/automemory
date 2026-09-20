# AutoMemory Pro: Planning Directory (`.plan/`)

This directory serves as the canonical source of truth for engineering plans, architecture blueprints, execution milestones, and future roadmap phases for **AutoMemory Pro**.

---

## Roadmap & Phase Status

| Phase | Title | Status | Primary Focus |
|---|---|---|---|
| **Phase 1** | [Core MVP Engine](phase-1-mvp-core.md) | ✅ **Completed** | Rust workspace, SQLite WAL, FTS5 search, out-of-process hooks (<2ms), 30+ MCP tools, Lamborghini dark web dashboard. |
| **Phase 1.5** | [Daemon Automation & Host Wiring](phase-1.5-polish-and-wiring.md) | ✅ **Completed** | Live document watcher (`notify`), debounced Git auto-sync daemon, pluggable LLM extractor (Ollama/APIs), multi-host `automemory connect`. |
| **Phase 2** | [Advanced Intelligence & Knowledge Graph](phase-2-advanced-intelligence.md) | ✅ **Completed** | Pure local ONNX embeddings (`fastembed-rs`), Reciprocal Rank Fusion (RRF), codebase structural graph, SSE stream & graph MCP tools. |
| **Phase 2.5** | [Next-Gen React Dashboard & WebSocket Engine](phase-2.5-nextgen-web-and-websocket.md) | 🟡 **In Progress** | Modular React 19 + Vite SPA, Axum WebSocket JSON-RPC & live broadcasting (`/ws`), Web Worker offloading (graph layout & event indexing), embedded single binary. |
| **Phase 3** | [Managed Cloudflare SaaS & Teams](phase-3-managed-cloud.md) | ⚪ **Planned (Post-MVP)** | Cloudflare Workers + D1 + R2 sync, zero-knowledge client encryption, team workspaces, RBAC, and hosted web app. |

---

## Architectural Principles

1. **Local-First & Offline-Operational**: The core engine must run 100% locally with zero required external API keys or cloud accounts.
2. **Deterministic & Lightning Fast (<5ms)**: Lifecycle hooks execute out-of-process in `<2ms`, never blocking host coding agent turns.
3. **Canonical Text-First Vault**: Memory artifacts, traces, and documentation sync as diffable Markdown and JSONL, avoiding live binary database git merge conflicts.
4. **Agent-Neutral Core**: Seamless support for Google Antigravity, Claude Code, Cursor, Codex CLI, and any standard MCP client.

---

## Document Index

- [`phase-1-mvp-core.md`](phase-1-mvp-core.md): Architecture, data models, and implementation details for Milestone 1.
- [`phase-1.5-polish-and-wiring.md`](phase-1.5-polish-and-wiring.md): Daemon automation, document watching, and adapter connections.
- [`phase-2-advanced-intelligence.md`](phase-2-advanced-intelligence.md): Local vector embeddings, RRF hybrid retrieval, and knowledge graph specification.
- [`phase-2.5-nextgen-web-and-websocket.md`](phase-2.5-nextgen-web-and-websocket.md): Modular React 19 web dashboard, WebSocket streaming & RPC, and Web Worker offloading.
- [`phase-3-managed-cloud.md`](phase-3-managed-cloud.md): Cloudflare serverless sync, team collaboration, and web app.
