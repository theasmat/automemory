# Coding Agents Integration Guide

AutoMemory provides zero-configuration integration for **20 major AI coding agents**. All connected agents share the exact same persistent memory database, trajectory distillation engine, and AST guardrails.

---

## 🚀 Quick Connect

Connect all supported agents in your workstation with a single command:

```bash
# Workspace Mode (Local Repository):
automemory connect all

# Global Mode (All user-level configurations):
automemory connect all -g
```

Or connect a specific agent individually:
```bash
automemory connect claude -g
automemory connect cursor
automemory connect windsurf -g
```

---

## 📋 Supported Agents & Configuration Matrix

| # | Agent Host | CLI Flag / Aliases | Workspace Mode Path | Global Mode Path (`-g`) |
|---|---|---|---|---|
| 1 | **Antigravity / Gemini** | `agy`, `antigravity`, `gemini` | `.agents/hooks.json` + `.agents/plugins/` | `~/.gemini/config/mcp_config.json` |
| 2 | **Claude Code** | `claude`, `claudecode` | `.mcp.json` | `~/.claude.json` |
| 3 | **Claude Desktop** | `claude-desktop` | *(App-level)* | `~/Library/Application Support/Claude/claude_desktop_config.json` |
| 4 | **Cursor** | `cursor` | `.cursor/mcp.json` + `.cursorrules` | `~/Library/Application Support/Cursor/.../mcp.json` |
| 5 | **Windsurf (Codeium)** | `windsurf`, `codeium` | `.windsurf/mcp.json` | `~/.codeium/windsurf/mcp_config.json` |
| 6 | **Continue.dev** | `continue`, `vscode` | `.continue/config.json` | `~/.continue/config.json` |
| 7 | **Cline** | `cline` | `.cline/mcp.json` | `Code/User/globalStorage/saoudrizwan.claude-dev/...` |
| 8 | **Roo Code** | `roo`, `roocode`, `roo-cline` | `.roo/mcp.json` | `Code/User/globalStorage/rooveterinaryinc.roo-cline/...` |
| 9 | **Kilo Code** | `kilo`, `kilocode` | `.kilo/mcp.json` | `Code/User/globalStorage/kilocode.kilo-code/...` |
| 10 | **Zed Editor** | `zed` | `.zed/settings.json` | `~/.config/zed/settings.json` |
| 11 | **Codex CLI (OpenAI)** | `codex`, `openai` | `.codex/config.json` | `~/.codex/config.json` |
| 12 | **OpenCode** | `opencode` | `.opencode/config.json` | `~/.opencode/config.json` |
| 13 | **Goose (Block)** | `goose`, `block-goose` | `.goose/config.yaml` | `~/.config/goose/config.yaml` |
| 14 | **Aider** | `aider` | `.aider.conf.yml` | `~/.aider.conf.yml` |
| 15 | **Warp Terminal** | `warp` | `.warp/mcp.json` | `~/.warp/mcp.json` |
| 16 | **Gemini CLI (Standalone)** | `gemini-cli` | `.gemini/settings.json` | `~/.gemini/settings.json` |
| 17 | **Devin** | `devin` | `.devin/mcp.json` | `~/.devin/mcp.json` |
| 18 | **GitHub Copilot** | `copilot`, `ghcopilot` | `.vscode/mcp.json` | `Code/User/mcp.json` |
| 19 | **OpenClaw** | `openclaw` | `.openclaw/mcp.json` | `~/.openclaw/mcp.json` |
| 20 | **Hermes (NousResearch)** | `hermes` | `.hermes/mcp.json` | `~/.hermes/mcp.json` |

---

## 🪟 Windows Path Equivalents

On Windows systems, paths are automatically mapped to standard environment folders:
- `~/.config` -> `%APPDATA%`
- `Code/User` -> `%APPDATA%\Code\User`
- Global configs -> `%USERPROFILE%\.<agent>\config.json`

---

## 🪝 Out-of-Process Lifecycle Interception

For agents supporting hook systems (Antigravity, Gemini, OpenCode, Claude Code), AutoMemory installs zero-overhead lifecycle hooks.

When an agent executes tools or shell commands, events are streamed asynchronously via:
```bash
automemory hook --agent agy --event post-tool --project my-project
```

### Registered Hook Events:
- **`session_boot`**: Injects token-capped Hot Memory Constitution and recent project decisions into the agent prompt before the first turn.
- **`pre_tool`**: Validates planned shell commands against AST guardrail rules (`vibe`, `strict`, `review`).
- **`post_tool`**: Logs tool execution evidence and file diffs into the append-only SQLite WAL ledger in <5ms.
- **`post_tool_fail`**: Captures error stack traces, triggers root-cause hypothesis generation, and catalogs failure playbooks.
- **`session_stop`**: Triggers async trajectory distillation, pruning transient noise and synthesizing reusable procedural skills.
- **`pre_commit`**: Checks staged files against active multi-agent leases before the commit lands.
- **`post_commit`**: Records the commit and triggers distillation; symbol-linked staleness marking is planned (memories do not carry linked paths yet).

---

## 🔄 Cross-Agent Memory Sharing

Because AutoMemory uses a unified local SQLite WAL engine (`~/.automemory/automemory.db`), all coding agents operate with complete shared awareness:

1. **Session 1 (Cursor)**: You ask Cursor to refactor your database migrations and enforce strict foreign keys. Cursor records the architectural decision.
2. **Session 2 (Claude Code)**: You start a session in Claude Code. On startup, Claude Code queries `memory.context` and immediately knows your database schema policies without needing re-explanation.
3. **Session 3 (Windsurf / Cline)**: When executing shell commands, both agents pass through the same AST safety interceptor, blocking catastrophic operations like dropping tables or formatting production disks.
