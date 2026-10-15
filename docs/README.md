# AutoMemory Documentation Index

Local-first persistent memory, verified recall, and multi-agent coordination for AI coding agents. Pure Rust, single binary, SQLite WAL.

---

## Start here

| Document | What it is |
|---|---|
| [PRD](PRD.md) | Normative product requirements: goals, 16 ports, hook pipeline, trust model, phases |
| [OVERVIEW](OVERVIEW.md) | Vision, pillars, measured benchmarks, roadmap |
| [ARCHITECTURE](ARCHITECTURE.md) | As-built technical architecture: runtime, tools, data model |
| [PRODUCT](PRODUCT.md) | Detailed product spec: registry, extraction, sync, acceptance criteria |

## Use it

| Document | What it is |
|---|---|
| [INSTALLATION](INSTALLATION.md) | Installers, Cargo builds, diagnostics |
| [AGENTS](AGENTS.md) | 20-agent integration matrix, hook events, cross-agent sharing |
| [HOOKS](HOOKS.md) | Hook event catalog, pipeline config (`hooks.toml`), install/remove |
| [CLI_REFERENCE](CLI_REFERENCE.md) | Every CLI command with flags |
| [MCP_TOOLS](MCP_TOOLS.md) | All 59 MCP tools with schemas |
| [human_testing_guide](human_testing_guide.md) | QA walkthrough with copy-paste prompts |

## Operate it

| Document | What it is |
|---|---|
| [GUARDRAILS](GUARDRAILS.md) | Safety modes, rule specs, custom policies |
| [SYNC_AND_VAULT](SYNC_AND_VAULT.md) | Vault sync, BIP-39 pairing, edge gateway |
| [DASHBOARD](DASHBOARD.md) | Embedded console, REST, WebSocket pulse |
| [FEATURES](FEATURES.md) | Retrieval, distillation, failure catalog, constitution |
| [ACHIEVEMENTS](ACHIEVEMENTS.md) | Verified capability ledger + measurement history |

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
