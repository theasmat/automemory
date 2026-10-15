# AutoMemory — Hooks Configuration Guide

**Audience:** Coding agents, users, maintainers
**Purpose:** Describe how hooks are installed and configured. Status: accurate as of v0.1.0 + hook pipeline (Oct 2026).

---

## 1. Overview

AutoMemory uses out-of-process hooks to capture agent events. Two locations matter:

- **Reference templates** in `~/.automemory/hooks/{agent}.json`, written by `automemory init`. `automemory doctor` checks these exist.
- **Live installation** in `<workspace>/.agents/hooks.json` (plus `.mcp.json` and `.agents/plugins/`), written by `automemory connect マージ`. Only the Antigravity adapter installs native hooks today; every other agent joins over MCP (see `automemory adapter list`).

Hook pipeline behavior (gate, order, budgets, shadow mode) is configured separately in `hooks.toml` — global `~/.automemory/hooks.toml` layered under project `.automemory/hooks.toml`. See `automemory hooks list` and `automemory hooks test --event <e> --input sample.json`.

---

## 2. Canonical Hook Events

`automemory hook --event` accepts the CLI spellings below, mapped to canonical events in `CanonicalEvent::from_cli`. Unmapped names are recorded verbatim and return `{}`.

| CLI `--event` | Canonical | What the handler does |
|---|---|---|
| `boot` | `SessionStart` | Silent by default (pull principle). With `[defaults] boot_injection = true`, prints token-capped context once and returns. |
| `pre-tool` | `PreToolUse` | Runs the gate pipeline (lease check → guardrail → custom exec, strictest wins). Prints `{"decision": "allow"\|"warn"\|"ask"\|"deny"}`. |
| `post-tool`, `post-tool-fail` | `PostToolUse` | Appends the tool outcome (including exit code and stderr on failure) to the ledger. Prints `{}`. |
| `task-done` | `Stop` | Releases the session's leases, runs distillation. Prints `{}`. |
| `stop` | `Stop` | Same as `task-done`, plus blocks with `{"decision": "block"}` when the session still owes work (claimed task, unpicked baton, owed review). |
| `pre-commit` | `PreCommit` | Gate pipeline over staged files (`git diff --cached`); lease collisions deny in Strict mode. |
| `post-commit` | `PostCommit` | Flips memories linked to committed files to `changed`; triggers distillation. |
| `user-prompt-submit`, `prompt` | `UserPromptSubmit` | Prints a one-line pending-work nudge only if something is assigned, else `{}`. |
| `subagent-start`, `subagent-stop` | `SubagentStart`, `SubagentStop` | Recorded to the ledger for attribution; no gate. Prints `{}`. |
| `pre-compact`, `post-compact` | `PreCompact`, `PostCompact` | Recorded to the ledger; no gate. Prints `{}`. |
| `notify` | `Notification` | Recorded to the ledger for replay. Prints `{}`. |

---

## 3. Antigravity Hook Configuration

`automemory connect agy` (workspace scope) writes `.agents/hooks.json` with exactly these registrations (5s timeout each):

```json
{
  "automemory": {
    "enabled": true,
    "PreInvocation": [
      {
        "type": "command",
        "command": "<automemory-bin> hook --agent agy --event boot --project auto",
        "timeout": 5
      }
    ],
    "PreToolUse": [
      {
        "matcher": "*",
        "hooks": [
          {
            "type": "command",
            "command": "<automemory-bin> hook --agent agy --event pre-tool --project auto",
            "timeout": 5
          }
        ]
      }
    ],
    "PostToolUse": [
      {
        "matcher": "*",
        "hooks": [
          {
            "type": "command",
            "command": "<automemory-bin> hook --agent agy --event post-tool --project auto",
            "timeout": 5
          }
        ]
      }
    ],
    "Stop": [
      {
        "type": "command",
        "command": "<automemory-bin> hook --agent agy --event stop --project auto",
        "timeout": 5
      }
    ]
  }
}
```

### 3.1 How It Works

1. User runs `automemory connect agy` — CLI writes `.agents/hooks.json`, root `.mcp.json`, and `.agents/plugins/automemory/`.
2. `automemory init --agent agy` writes a reference copy to `~/.automemory/hooks/agy.json`.
3. Hooks fire as separate OS processes running the same Rust binary — a hook crash cannot take the agent down.
4. `Stop` triggers lease release plus trajectory distillation, pruning transient noise into reusable traces.

---

## 4. Claude Code Hook Configuration

`automemory connect claude` installs **MCP configuration only** (`.mcp.json` in workspace scope, `~/.claude.json` globally). It does not install native lifecycle hooks — Claude Code joins over MCP plus `UserPromptSubmit`-style nudges where supported. There is no `claude.json` hook file to create; do not invent one.

---

## 5. Hook Command Options

```bash
automemory hook \
  --agent <agent-id> \
  --event <event-name> \
  --project auto|<project-slug>
```

| Option | Description |
|---|---|
| `--agent` | Agent identifier (`agy`, `claude`, `cursor`, etc.) |
| `--event` | CLI event name from the §2 table |
| `--project` | Project identifier (`auto` for auto-detect, or explicit slug) |

---

## 6. Project Auto-Detection

When `--project auto` is used, AutoMemory:

1. Uses an explicit slug if one matches a registered project.
2. Checks the `AGENTMEMORY_PROJECT_NAME` environment variable.
3. Matches the current working directory against registered project paths.
4. Falls back to Git repository root detection.
5. Otherwise proceeds with an inferred slug (local only, no sync).

---

## 7. Hook Installation

### 7.1 Automatic Installation

```bash
automemory connect agy        # workspace scope: writes .agents/hooks.json
automemory connect agy -g     # global scope where the agent supports it
automemory init --agent agy   # writes reference template to ~/.automemory/hooks/
```

- Existing MCP configs are merged, never overwritten (other servers are preserved).
- There is no backup/restore flow. `automemory disconnect <agent>` removes exactly what `connect` wrote (MCP server keys, hooks key, plugin dir); third-party entries and edited files are left intact.
- Run `automemory doctor` to validate event delivery.

### 7.2 Manual Installation

1. Copy the §3 JSON into the agent's hooks file.
2. Replace `<automemory-bin>` with the absolute path to the binary.
3. Run `automemory doctor` to validate.

---

## 8. Troubleshooting

### 8.1 Hooks Not Firing

- Check `<workspace>/.agents/hooks.json` exists (live install), not just `~/.automemory/hooks/`.
- Verify the agent version still supports the registered hook points.
- Run `automemory doctor` to test event delivery.

### 8.2 Hooks Slow Down Agent

- PreToolUse/PreCommit run the gate pipeline in-process (measured ~1.26 ms mean; PRD budget is 5 ms p95). External `exec` handlers are killed past their `latency_budget_ms`.
- If slow, check for custom exec handlers hanging, network timeouts, or database locks.
- Run `automemory doctor` to measure hook latency.

### 8.3 Project Not Detected

- Ensure project is registered: `automemory project list`.
- Check working directory matches registered path.
- Verify Git repository root is correct.

---

## 9. Future Agents

New agent adapters follow the same pattern:

1. Add a registry entry (`adapter.rs`: id, aliases, hook coverage).
2. Implement the `connect_*` installer (MCP config; hooks only where the agent supports them).
3. Test with `automemory doctor`.
4. Document in this guide. List coverage with `automemory adapter list`.
