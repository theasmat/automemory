# AutoMemory

<p align="center">
  <strong>Persistent long-term memory, verified recall, and multi-agent coordination for 20 AI coding agents.</strong><br/>
  <em>Pure native Rust. Single binary. Local SQLite WAL.</em>
</p>

<p align="center">
  <a href="LICENSE-MIT"><img src="https://img.shields.io/badge/license-MIT%20OR%20Apache--2.0-blue.svg" alt="License" /></a>
  <a href="https://www.rust-lang.org"><img src="https://img.shields.io/badge/rust-1.80%2B-orange.svg" alt="Rust" /></a>
  <a href="#supported-agents"><img src="https://img.shields.io/badge/agents-20%20connected-success.svg" alt="20 Agents" /></a>
  <a href="docs/GUARDRAILS.md"><img src="https://img.shields.io/badge/governance-Zero--SQL%20Rule-gold.svg" alt="Zero-SQL" /></a>
  <a href="docs/SYNC_AND_VAULT.md"><img src="https://img.shields.io/badge/security-AES--256--GCM%20Zero--Knowledge-brightgreen.svg" alt="Security" /></a>
  <a href="docs/MCP_TOOLS.md"><img src="https://img.shields.io/badge/mcp-59%20tools-purple.svg" alt="MCP Tools" /></a>
</p>

---

## ⚡ Quick Install

### macOS & Linux
```bash
curl -fsSL https://raw.githubusercontent.com/theasmat/automemory-bin/main/install.sh | bash
```

### Windows (PowerShell)
```powershell
irm https://raw.githubusercontent.com/theasmat/automemory-bin/main/install.ps1 | iex
```

### Via Cargo
```bash
cargo install --git https://github.com/theasmat/automemory.git --bin automemory
```

> **Requirements**: Single standalone binary. No Python or external database required. (The optional CodeIntel companion path uses `npx`, which needs Node.js.)

---

## 🚀 3-Step Setup

### 1. Connect Your Coding Agents
Configure all supported agents in your workstation in one shot:
```bash
# Workspace Mode (Current Project):
automemory connect all

# Global Mode (All user-level configurations):
automemory connect all -g
```

### 2. Verify System Health
```bash
automemory doctor
```

### 3. Launch Local Dashboard Daemon
```bash
automemory serve --port 4545
```
Open **[http://127.0.0.1:4545](http://127.0.0.1:4545)** to inspect memories, review proposals, and explore the file-level code graph.

---

## 🤖 Supported Coding Agents (20 Hosts)

AutoMemory provides universal configuration for **20 major AI coding agents**. Only Antigravity gets native lifecycle hooks; all others join over MCP:

| Category | Agents Supported |
|---|---|
| **Autonomous CLI Agents** | **Claude Code**, **Codex CLI**, **Gemini CLI**, **OpenCode**, **Aider**, **Goose (Block)**, **Devin**, **OpenClaw**, **Hermes** |
| **IDE & Editor Agents** | **Antigravity / Gemini** (hooks), **Cursor**, **Windsurf (Codeium)**, **Continue.dev**, **Zed Editor**, **GitHub Copilot**, **Claude Desktop** |
| **VS Code Extensions** | **Cline**, **Roo Code**, **Kilo Code** |
| **Terminal Environments** | **Warp Terminal** |
| **Universal Batch** | `automemory connect all [-g]` configures all 20 agents in one command. |

👉 *For detailed configuration paths, workspace vs. global options, and hook specifications, see the **[Coding Agents Integration Guide](docs/AGENTS.md)**.*

---

## ⚡ Key Architectural Capabilities

- 🦀 **Pure Native Rust Core (hook pipeline ~1.26 ms mean)**: Zero interpreter overhead, multi-threaded WAL concurrency. Measured via `cargo bench -p automemory-core`.
- 🧹 **Zero-SQL Rule & Proposal Governance**: No direct database tampering; deduplication and bulk proposal decisions go through reviewed MCP/CLI paths.
- 🗜️ **Trajectory Distillation**: Strips noisy exploratory transcripts at session stop and synthesizes compact procedural skill playbooks (~0.11 ms per 40-event session, measured).
- 🛡️ **Catastrophe Interceptor**: Shell lexer halts destructive actions (`rm -rf /`, force-pushes to production, raw disk formats, secret leaks) across *Vibe*, *Strict*, and *Review* modes.
- 🧠 **Verified Recall**: Trust tiers 1–5; Tier 4–5 claims are labeled and withheld from default recall; PostCommit flips linked memories to `changed`.
- 🔒 **Multi-Agent Locks & Handoff**: File/glob/symbol leases with collision detection, sub-400-token ledger-grounded batons, task queue, cross-review with tests as judge, turn-gated Planning Rooms.
- 📐 **Git-Ignored Plan Vault**: Preserves git-ignored specifications into a private vault without cluttering target repositories.
- 🔒 **Zero-Knowledge Encryption**: Client-side AES-256-GCM with keys derived via PBKDF2. Edge servers only ever see blind digests.
- 🖥️ **Embedded Dashboard**: Built-in Axum REST API and WebSocket gateway serving the pre-compiled Nuxt 3 app.
- 🔌 **Full Model Context Protocol (MCP)**: Native stdio and HTTP transports implementing 59 tools.

---

## 📚 Modular Documentation

All documentation lives in [`docs/`](docs/README.md), categorized by use:

- 📜 **[PRD](docs/PRD.md)**: Normative requirements — goals, 16 ports, pipeline, trust model, phases.
- 🗺️ **[Overview](docs/OVERVIEW.md)**: Vision, pillars, measured benchmarks, roadmap.
- 🏗️ **[Architecture](docs/ARCHITECTURE.md)**: As-built runtime, full tool catalog, data model.
- 📦 **[Installation](docs/INSTALLATION.md)**, 🤖 **[Agents](docs/AGENTS.md)**, 🪝 **[Hooks](docs/HOOKS.md)**, 🛠️ **[CLI](docs/CLI_REFERENCE.md)**, 🔌 **[MCP](docs/MCP_TOOLS.md)**, 🧪 **[Testing guide](docs/human_testing_guide.md)**.
- 🛡️ **[Guardrails](docs/GUARDRAILS.md)**, 🔒 **[Sync & Vault](docs/SYNC_AND_VAULT.md)**, 🖥️ **[Dashboard](docs/DASHBOARD.md)**, 🧠 **[Features](docs/FEATURES.md)**, 🏆 **[Achievements](docs/ACHIEVEMENTS.md)**.

---

## 🧪 Testing & Validation

```bash
# Run all workspace unit and integration tests (120 tests)
cargo test --workspace

# Measured perf benches (hook pipeline, FTS recall, distillation)
cargo bench -p automemory-core --bench perf

# Verify system diagnostics
automemory doctor
```

---

## 📄 License

Dual-licensed under either of:
- Apache License, Version 2.0 ([LICENSE-APACHE](LICENSE-APACHE) or http://www.apache.org/licenses/LICENSE-2.0)
- MIT license ([LICENSE-MIT](LICENSE-MIT) or http://opensource.org/licenses/MIT)
