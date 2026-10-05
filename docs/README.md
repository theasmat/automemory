# AutoMemory Documentation Index

Welcome to the official technical documentation for **AutoMemory** — the local-first, zero-knowledge persistent memory and autonomous intelligence platform for AI coding agents.

---

## 📚 Documentation Chapters

| Document | Description |
|---|---|
| 📦 **[Installation Guide](INSTALLATION.md)** | macOS, Linux, and Windows quick scripts, Cargo installs, prerequisites, and diagnostics. |
| 🤖 **[Coding Agents Integration](AGENTS.md)** | Deep integration guide for 20 coding agents (Claude, Cursor, Antigravity, Windsurf, Roo, etc.). |
| 🧠 **[Core Features & Cognitive Architecture](FEATURES.md)** | Native Rust core, BM25 decay search, 100-to-1 trajectory distillation, and failure catalogs. |
| 🔌 **[Model Context Protocol (MCP) Reference](MCP_TOOLS.md)** | Full reference for all 26 MCP tools with input schemas, parameters, and descriptions. |
| 🛡️ **[Autonomous Guardrails & Safety](GUARDRAILS.md)** | AST command tokenizer, catastrophic interceptor, and `vibe`/`strict`/`review` autopilot modes. |
| 🔒 **[Zero-Knowledge Cloud Sync & Vault](SYNC_AND_VAULT.md)** | OpenViking git-ignored plan sync, private GitHub vaults, and Cloudflare edge gateway. |
| 🖥️ **[Interactive Dashboard & REST API](DASHBOARD.md)** | Embedded web console (`http://localhost:4545`), proposal bulk actions, and WebSocket live pulse. |
| 🛠️ **[CLI Command Reference](CLI_REFERENCE.md)** | Complete command-line reference and cheat sheet for all subcommands. |
| 🧪 **[Human Testing & Edge Cases](human_testing_guide.md)** | QA guide with copy-paste prompts and verification steps for real-world scenarios. |

---

## ⚡ Quick Start Summary

```bash
# 1. Install (macOS / Linux):
curl -fsSL https://raw.githubusercontent.com/theasmat/automemory-bin/main/install.sh | bash

# Or Install (Windows PowerShell):
irm https://raw.githubusercontent.com/theasmat/automemory-bin/main/install.ps1 | iex

# 2. Connect all 20 agents globally:
automemory connect all -g

# 3. Launch embedded Web Dashboard:
automemory serve --port 4545
```
