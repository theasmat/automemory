# AutoMemory

<p align="center">
  <strong>Persistent long-term memory, 100-to-1 trajectory distillation, and autonomous AST guardrails for 20 AI coding agents.</strong><br/>
  <em>Pure native Rust. Sub-millisecond latency. Zero-knowledge local & edge synchronization.</em>
</p>

<p align="center">
  <a href="LICENSE-MIT"><img src="https://img.shields.io/badge/license-MIT%20OR%20Apache--2.0-blue.svg" alt="License" /></a>
  <a href="https://www.rust-lang.org"><img src="https://img.shields.io/badge/rust-1.80%2B-orange.svg" alt="Rust" /></a>
  <a href="#supported-agents"><img src="https://img.shields.io/badge/agents-20%20connected-success.svg" alt="20 Agents" /></a>
  <a href="docs/GUARDRAILS.md"><img src="https://img.shields.io/badge/governance-Zero--SQL%20Rule-gold.svg" alt="Zero-SQL" /></a>
  <a href="docs/SYNC_AND_VAULT.md"><img src="https://img.shields.io/badge/security-AES--256--GCM%20Zero--Knowledge-brightgreen.svg" alt="Security" /></a>
  <a href="docs/MCP_TOOLS.md"><img src="https://img.shields.io/badge/mcp-26%20tools-purple.svg" alt="MCP Tools" /></a>
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

> **Requirements**: Zero runtime dependencies. No Node.js, Python, or external database required. Single standalone binary compiled in native Rust.

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
Open **[http://127.0.0.1:4545](http://127.0.0.1:4545)** to inspect active memories, manage review proposals with 1-click bulk approvals, and explore your codebase AST graph.

---

## 🤖 Supported Coding Agents (20 Hosts)

AutoMemory provides universal configuration for **20 major AI coding agents**. All connected agents share the exact same persistent memory database, trajectory distillation engine, and AST guardrails:

| Category | Agents Supported |
|---|---|
| **Autonomous CLI Agents** | **Claude Code**, **Codex CLI**, **Gemini CLI**, **OpenCode**, **Aider**, **Goose (Block)**, **Devin**, **OpenClaw**, **Hermes** |
| **IDE & Editor Agents** | **Antigravity / Gemini**, **Cursor**, **Windsurf (Codeium)**, **Continue.dev**, **Zed Editor**, **GitHub Copilot**, **Claude Desktop** |
| **VS Code Extensions** | **Cline**, **Roo Code**, **Kilo Code** |
| **Terminal Environments** | **Warp Terminal** |
| **Universal Batch** | `automemory connect all [-g]` configures all 20 agents in one command. |

👉 *For detailed configuration paths, workspace vs. global options, and hook specifications, see the **[Coding Agents Integration Guide](docs/AGENTS.md)**.*

---

## ⚡ Key Architectural Capabilities

- 🦀 **Pure Native Rust Core (<5ms Latency)**: Zero interpreter overhead. Instant sub-millisecond startup, minimal RAM footprint (<15 MB), and multi-threaded WAL concurrency.
- 🧹 **Zero-SQL Rule & High-Throughput Proposals**: Strictly eliminates direct database tampering. Atomic bulk proposal approvals archive redundant trajectories at over 12,000 operations/sec without raw SQL risks.
- 🗜️ **100-to-1 Trajectory Distillation**: Strips noisy multi-step exploratory transcripts, extracts verified code edits, and synthesizes compact procedural skill playbooks.
- 🛡️ **Autonomous Catastrophe Interceptor**: Multi-tier AST tokenizer actively halts destructive actions (`rm -rf /`, force-pushes to production, raw disk formats, unverified drops) across *Vibe*, *Strict*, and *Review* modes.
- 📐 **OpenViking Git-Ignored Plan Vault**: Automatically preserves git-ignored specifications (`plans/**`, `.gemini/**`, `docs/specs/**`) into a private GitHub vault without cluttering public target repositories.
- 🔒 **Zero-Knowledge Encryption**: Client-side AES-256-GCM authenticated encryption with keys derived via PBKDF2 (600,000 rounds). Edge servers only ever see pseudorandom blind HMAC digests.
- 🖥️ **Embedded Dark Web Dashboard**: Built-in Axum REST API and WebSocket event gateway serving pre-compiled Nuxt single-page application assets directly from memory.
- 🔌 **Full Model Context Protocol (MCP)**: Native stdio and HTTP transports implementing 26 tools for context assembly, skill acquisition, graph impact, and proposal decisions.

---

## 📊 Comparison Matrix

| Feature | AutoMemory | agentmemory | mem0 | Letta / MemGPT | Static (CLAUDE.md) |
|---|:---:|:---:|:---:|:---:|:---:|
| **Engine Runtime** | **Native Rust** | TypeScript (Node.js) | Python / API | Python runtime | Static Markdown |
| **Startup Overhead** | **< 1 ms** | ~200–500 ms | Network latency | High (VM/Runtime) | 0 ms |
| **External Database** | **0 (Embedded SQLite)** | Embedded iii | Vector DB required | PostgreSQL required | None |
| **Supported Agents** | **20 Agents** | 20 Agents | ~5 Agents | Custom runtime | Agent-specific |
| **AST Catastrophe Interceptor** | **Built-in (Tree-Sitter)** | None | None | None | None |
| **100:1 Trajectory Distillation** | **Autonomous** | Manual / LLM | Manual / LLM | Manual / LLM | Manual |
| **Zero-SQL Rule Governance** | **Enforced** | None | None | None | None |
| **Web Dashboard** | **Embedded (4545)** | Separate viewer | Cloud app | Web console | None |
| **Zero-Knowledge Encryption** | **AES-256-GCM + PBKDF2** | None | Cloud / Server | Server | None |

---

## 📚 Modular Documentation

AutoMemory features comprehensive documentation chapters covering every aspect of the platform:

- 📦 **[Installation Guide](docs/INSTALLATION.md)**: Shell installers for macOS, Linux, and Windows, Cargo builds, and diagnostics.
- 🤖 **[Coding Agents Integration](docs/AGENTS.md)**: Configuration matrix, paths, and lifecycle hooks for all 20 agents.
- 🧠 **[Core Features & Cognitive Architecture](docs/FEATURES.md)**: Detailed dive into BM25 decay search, distillation, and the AST code graph.
- 🔌 **[Model Context Protocol (MCP) Reference](docs/MCP_TOOLS.md)**: Complete parameter and schema reference for all 26 MCP tools.
- 🛡️ **[Autonomous Guardrails & Safety](docs/GUARDRAILS.md)**: Safety modes (`vibe`, `strict`, `review`), rule specifications, and custom regex policies.
- 🔒 **[Zero-Knowledge Cloud Sync & Vault](docs/SYNC_AND_VAULT.md)**: OpenViking plan synchronization, BIP-39 mnemonic pairing, and Cloudflare edge gateway.
- 🖥️ **[Interactive Dashboard & REST API](docs/DASHBOARD.md)**: Embedded web console, REST endpoints, and live WebSocket pulse.
- 🛠️ **[CLI Command Reference](docs/CLI_REFERENCE.md)**: Comprehensive command-line cheat sheet for all subcommands and flags.
- 🧪 **[Human Testing & Edge Cases](docs/human_testing_guide.md)**: Step-by-step QA verification guide with copy-paste prompts.

---

## 🧪 Testing & Validation

AutoMemory is verified by an extensive automated test suite covering all 108 IDE agent use cases, cryptography, and benchmarks:

```bash
# Run all workspace unit and integration tests (56 tests)
cargo test --workspace

# Run tests with full release optimizations
cargo test --release --workspace

# Verify system diagnostics
automemory doctor
```

---

## 📄 License

Dual-licensed under either of:
- Apache License, Version 2.0 ([LICENSE-APACHE](LICENSE-APACHE) or http://www.apache.org/licenses/LICENSE-2.0)
- MIT license ([LICENSE-MIT](LICENSE-MIT) or http://opensource.org/licenses/MIT)
