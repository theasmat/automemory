# AutoMemory Pro

[![License](https://img.shields.io/badge/license-MIT%20OR%20Apache--2.0-blue.svg)](LICENSE)
[![Rust](https://img.shields.io/badge/rust-1.80%2B-orange.svg)](https://www.rust-lang.org)
[![Zero-Knowledge](https://img.shields.io/badge/security-Zero--Knowledge%20AES--256--GCM-brightgreen.svg)](#zero-knowledge-blind-architecture)
[![Cloudflare Edge](https://img.shields.io/badge/edge-Cloudflare%20Workers%20%2B%20D1%20%2B%20Vectorize-F38020.svg)](#cloudflare-edge-gateway--developer-dashboard)
[![CI](https://github.com/theasmat/automemory/actions/workflows/ci.yml/badge.svg)](https://github.com/theasmat/automemory/actions/workflows/ci.yml)

**AutoMemory Pro** is an open-source, local-first, zero-knowledge persistent memory and autonomous intelligence platform for AI coding agents (Antigravity, Claude Code, Cursor, Codex). Built natively in Rust for blistering performance, AutoMemory Pro passively captures task evidence, tool outcomes, architectural decisions, failure post-mortems, user preferences, and repeatable procedures without ever blocking your coding workflow.

AutoMemory Pro features a **Zero-Knowledge Blind Cloud Architecture** powered by Cloudflare Workers, D1, R2, and Vectorize, paired with **OpenViking-style Plan Synchronization** that backs up git-ignored project plans and specifications into private GitHub vaults without cluttering target repositories.

---

## ⚡ Key Highlights & Capabilities

- 🦀 **Pure Native Rust**: Zero Node.js or Python runtime overhead. Sub-millisecond startup, minimal RAM footprint, and pure multi-threaded concurrency.
- ⚡ **Ultra-Fast Out-of-Process Hooks (<5ms)**: Asynchronously logs agent tool executions, file diffs, and test outcomes to an append-only WAL ledger without blocking agents.
- 🔒 **Zero-Knowledge Blind Cloud Architecture**: All memories, skills, and plans are encrypted client-side using **AES-256-GCM** with keys derived via **PBKDF2 (600,000 iterations)**. Server and edge gateways store only blind HMAC hashes and ciphertext; no decryption keys ever touch the cloud.
- 🔑 **Multi-Device BIP-39 Pairing & OS Keychain**: Pair new workstations instantly using standard 12-word or 24-word BIP-39 mnemonic recovery phrases or terminal-rendered ASCII QR codes. Keys are securely stored in the native macOS Apple Keychain (`security`) or POSIX `0600` vaults.
- 📐 **OpenViking-Style Plan & Spec Vault**: Track git-ignored architectural plans, roadmaps, and project specifications in a private user GitHub vault and seamlessly restore them across machines without modifying target repositories.
- 🧠 **Blind Semantic Vector Search (Cloudflare Vectorize)**: Computes 384-dimensional vector embeddings on the client, executes high-speed cosine similarity searches anonymously on Cloudflare Vectorize, and decrypts payloads locally.
- 🌐 **Cloudflare Edge Sync Gateway & Developer Console**: Global low-latency replication across Cloudflare Workers, D1 SQLite, R2 object storage, and an integrated real-time Developer Web Console (`/dashboard`) with device fleet revocation and Slack/Discord webhook alerts.
- 👁️ **Real-Time Background Auto-Sync Watcher**: Runs as a background daemon (`automemory sync watch`) polling trackable plans and memory deltas, pushing updates instantaneously across your devices.
- 🛡️ **Autonomous Guardrails & Catastrophe Interceptor**: Multi-tier AST tokenizer that intercepts catastrophic commands (`rm -rf /`, force-push to `main`, unverified production migrations) with configurable *Vibe* and *Review* modes.
- 🔌 **Standard Model Context Protocol (MCP)**: Native stdio and HTTP transports implementing 30+ MCP tools for memory recall, skill acquisition, failure resolution, and context injection.
- 🏎️ **Lamborghini Dark UI**: High-contrast, sharp-edge web console (pure `#000000` with Lamborghini gold `#FFC000`) for visual memory inspection and fleet oversight.

---

## 🏛️ System Architecture

### Zero-Knowledge Local & Edge Pipeline

```
┌────────────────────────────────────────────────────────────────────────┐
│                   Coding Agent (Antigravity, Claude)                   │
└──────────────────┬─────────────────────────────────┬───────────────────┘
                   │ Out-of-process Hook (<5ms)      │ MCP (stdio / HTTP)
                   ▼                                 ▼
┌──────────────────────────────────────┐   ┌─────────────────────────────┐
│           automemory hook            │   │      AutoMemory MCP         │
│     (Async normalization & WAL)      │   │       (30+ Tools)           │
└──────────────────┬───────────────────┘   └───────────▲──────┬──────────┘
                   ▼                                   │      │
┌──────────────────────────────────────┐               │      │
│     Append-Only SQLite WAL Ledger    │               │      │
└──────────────────┬───────────────────┘               │      │
                   ▼                                   │      │
┌──────────────────────────────────────┐               │      │
│       Async Distillation Worker      │               │      │
│ (Skill Synthesis & Trajectory Prune) │               │      │
└──────────────────┬───────────────────┘               │      │
                   ▼                                   │      │
┌──────────────────────────────────────┐               │      │
│  Local SQLite WAL + FTS5 Database    ◄───────────────┘      │
│   (Encrypted Master Store on Disk)   ├──────────────────────┘
└──────────────────┬───────────────────┘
                   │
    Client-Side AES-256-GCM + PBKDF2 (600k rounds)
                   │
                   ▼
┌────────────────────────────────────────────────────────────────────────┐
│                     Cloudflare Edge Sync Gateway                       │
│                                                                        │
│   ┌────────────────────┐   ┌───────────────────┐   ┌───────────────┐   │
│   │    SQLite D1       │   │     R2 Buckets    │   │   Vectorize   │   │
│   │ (Blind Delta Sync) │   │ (Encrypted Blobs) │   │  (384d Blind) │   │
│   └────────────────────┘   └───────────────────┘   └───────────────┘   │
│                                                                        │
│   ┌────────────────────────────────────────────────────────────────┐   │
│   │             Edge Web Console (/dashboard)                      │   │
│   │   Fleet Management, Device Revocation, Webhook Dispatcher      │   │
│   └────────────────────────────────────────────────────────────────┘   │
└────────────────────────────────────────────────────────────────────────┘
```

---

## 🔒 Zero-Knowledge Security Model

AutoMemory Pro enforces absolute end-to-end zero-knowledge encryption:

1. **Client-Side Key Derivation**: Your Master Encryption Key never leaves your local workstation. It is derived from your passkey or BIP-39 mnemonic seed using PBKDF2 with HMAC-SHA256 and 600,000 iterations.
2. **Deterministic Blind Hashes**: Projects, memories, and skill identifiers are hashed with HMAC-SHA256 (`HMAC-SHA256(MasterKey, Salt || PlaintextID)`). Cloudflare edge servers see only random 64-character hex strings and cannot correlate your project names or content.
3. **Authenticated Ciphertext (AES-256-GCM)**: All memory contents, skill descriptions, instructions, and file specifications are encrypted with a fresh random 96-bit nonce before network dispatch. Tampering causes immediate authentication failure on decryption.
4. **Blind Vector Indexing**: 384-dimensional dense vectors represent geometric relationships in semantic space. They are indexed on Cloudflare Vectorize under blind IDs. Payloads remain fully encrypted on edge storage.

---

## 🚀 Quick Start Guide

### 1. Build & Install from Source

```bash
cargo build --release
sudo cp target/release/automemory /usr/local/bin/
```

### 2. Initialize AutoMemory & Register Agent Hooks

```bash
automemory init --agent agy
```

This creates `~/.automemory_pro/`, initializes the local encrypted SQLite WAL store, writes the Antigravity lifecycle hook (`agy.json`), and registers your current workspace.

### 3. Verify System Health with Doctor

```bash
automemory doctor
```

```text
=== AutoMemory Pro System Diagnostic (doctor) ===

Checking directories (~/.automemory_pro)... ✓ OK
Checking SQLite database connectivity & WAL mode... ✓ OK (opened in 1.42ms)
Benchmarking FTS5 search & memory operations... ✓ OK (FTS5 search latency: 0.98ms)
  Active memories: 14, Archived: 0, Trackable plans: 3
Checking Antigravity hook configuration (agy.json)... ✓ OK
Checking Zero-Knowledge Encryption Master Key... ✓ OK (AES-256-GCM authenticated)

✓ All health checks passed! AutoMemory Pro is fully operational.
```

### 4. Pair Secondary Machines (BIP-39 & QR Code)

On your primary workstation, generate your 12-word BIP-39 recovery seed and ASCII QR matrix:

```bash
automemory sync pair
```

On your secondary machine (e.g., laptop), run:

```bash
automemory sync pair --join "abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon about"
```

The master key is securely imported directly into your OS Keychain (macOS Keychain via `/usr/bin/security` or Linux secret service), completely eliminating manual key handling.

### 5. Start Background Auto-Sync Watcher

Keep your local project plans and remote memory bank perpetually synchronized:

```bash
automemory sync watch --interval 5
```

AutoMemory will monitor your registered project paths and push git-ignored architectural plans and encrypted deltas continuously.

---

## 📄 OpenViking-Style Git-Ignored Plan Synchronization

AutoMemory Pro solves the challenge of preserving AI agent plans, roadmaps, and scratchpads across multiple machines without polluting public repositories:

1. **Zero Workspace Clutter**: Keep `plans/`, `docs/specs/`, and `.gemini/` git-ignored in your target codebase.
2. **Global Config Mapping**: Configure tracked files in `~/.automemory_pro/config.json`:
   ```json
   {
     "vault_repo": "git@github.com:youruser/my-agent-vault.git",
     "projects": [
       {
         "name": "my-project",
         "path": "/Users/user/Dev/my-project",
         "track_files": ["plans/**", "docs/specs/**", "ARCHITECTURE.md"]
       }
     ]
   }
   ```
3. **Seamless Push & Pull**:
   - `automemory sync push`: Collects all matched files, commits them to your private vault repository under `projects/<project-slug>/`, and pushes encrypted memory deltas to the edge.
   - `automemory sync pull`: Pulls the latest vault changes and restores git-ignored specification files directly into your local workspace.

---

## 🛠️ CLI Reference

### Synchronization & Cloud Operations
| Command | Description |
|---|---|
| `automemory sync pair [--join <seed>]` | Generate or import 12-word BIP-39 mnemonic phrase and QR code |
| `automemory sync watch [--project <slug>] [--interval <sec>]` | Run real-time background file watcher and auto-sync daemon |
| `automemory sync push [--project <slug>]` | Push local plans to GitHub vault and encrypted deltas to Cloudflare Edge |
| `automemory sync pull [--project <slug>]` | Pull vault specifications and decrypt edge updates to local workspace |
| `automemory sync status` | Inspect sync ledger status, uncommitted plans, and edge connectivity |
| `automemory sync export [--path <path>]` | Export memories to diffable Markdown & JSONL format |
| `automemory sync import [--path <path>]` | Import Markdown/JSONL vault into local SQLite store |

### Core Memory & Semantic Search
| Command | Description |
|---|---|
| `automemory memory search <query> [--semantic]` | Hybrid search; add `--semantic` for blind Cloudflare Vectorize retrieval |
| `automemory memory list [--project <slug>]` | List all active project memories |
| `automemory memory save <summary> --content <text> [-t <type>]` | Explicitly persist durable architectural decision or gotcha |
| `automemory memory archive <id>` | Soft-delete a memory record |

### Guardrails, Mode & System
| Command | Description |
|---|---|
| `automemory guardrail test --command <cmd>` | Evaluate a shell command against AST safety rules |
| `automemory mode get [--project <slug>]` | Inspect current project operating mode (`vibe`, `review`, `strict`) |
| `automemory mode set <mode> [--project <slug>]` | Configure safety mode and review approval behavior |
| `automemory serve [--port 4545]` | Start local daemon, HTTP MCP, and Lamborghini Dark Web UI |
| `automemory mcp` | Launch Model Context Protocol server over stdio |
| `automemory doctor` | Comprehensive diagnostic check of keys, database, and hooks |

---

## 🔌 Model Context Protocol (MCP) Tool Suite

AutoMemory Pro exposes 30+ native MCP tools for direct integration with AI coding agents:

### Context & Memory Retrieval
- `memory.recall`: Query memories relevant to user task or prompt.
- `memory.context`: Assemble bounded token-capped context package for agent boot.
- `memory.search`: Full-text and semantic vector search across all tables.
- `memory.get` / `memory.list`: Inspect specific memory revisions and metadata.
- `memory.stats`: Ingest metrics, health counts, and storage utilization.

### Procedural Traces & Failure Post-Mortems
- `trace.save`: Save a verified step-by-step reproducible procedure.
- `trace.list` / `trace.get`: Retrieve successful multi-step trajectories.
- `failure.save`: Record an error pattern with root-cause hypothesis.
- `failure.resolve`: Associate a verified fix with an existing failure record.

### Guardrails & Governance
- `guardrail.eval`: Real-time safety validation before executing shell commands.
- `memory.proposals.list`: Inspect proposed memory migrations or consolidations.
- `memory.proposals.decide`: Approve or reject memory conflict resolutions.
- `memory.redact`: Verify sensitive credential and secret scrubbers.

---

## 🌐 Cloudflare Edge Gateway & Developer Dashboard

AutoMemory Pro includes a production-ready Cloudflare Worker sync gateway (`workers/sync-gateway`):

- **Zero-Knowledge D1 SQLite Engine**: Stores device registrations, encrypted deltas, sync events, and blind vector metadata.
- **R2 Blob Storage**: Stores large encrypted plan bundles and full memory backups.
- **Cloudflare Vectorize**: Real-time 384-dimensional approximate nearest-neighbor vector search.
- **Developer Web Console (`/dashboard`)**:
  - Live device fleet inventory with one-click revocation.
  - Real-time sync timeline with granular event filtering.
  - Storage consumption and plan tier monitoring.
  - Outgoing webhook notifications for Slack and Discord.

---

## 🧪 Testing & Verification

The AutoMemory Pro test suite validates the entire workspace across Rust and TypeScript:

```bash
# Run all native Rust workspace tests (42 unit, integration, and benchmark tests)
cargo test --workspace

# Run Cloudflare Worker local gateway integration suite (30 edge endpoint tests)
cd workers/sync-gateway
./test_local_gateway.sh
```

---

## 📄 License

Dual-licensed under either of:
- Apache License, Version 2.0 ([LICENSE-APACHE](LICENSE-APACHE) or http://www.apache.org/licenses/LICENSE-2.0)
- MIT license ([LICENSE-MIT](LICENSE-MIT) or http://opensource.org/licenses/MIT)
