# AutoMemory Pro — Hooks Configuration Guide

**Audience:** Coding agents, users, maintainers  
**Purpose:** Describe how hooks are installed and configured for Antigravity and future agents.

---

## 1. Overview

AutoMemory Pro uses out-of-process hooks to capture agent events. Hooks are configured per agent in JSON files under `~/.automemory_pro/hooks/`.

Example files:

- `agy.json` (Antigravity)
- `claude.json` (Claude Code)
- `cursor.json` (Cursor)

Each file lists commands to run for specific lifecycle events.

---

## 2. Hook Lifecycle Events

| Event | When It Fires | Purpose |
|---|---|---|
| `PreInvocation` | Before agent starts processing a new session | Initialize session, identify project, queue sync pull |
| `SessionStart` | First turn of a chat | Inject boot context |
| `PreToolUse` | Before a tool executes | Log tool call metadata, check permissions |
| `PostToolUse` | After a tool completes | Capture tool outcome evidence |
| `PostToolUseFailure` | After a tool fails | Capture failure evidence |
| `TaskCompleted` | When agent marks a task as complete | Create success trace or failure record |
| `Stop` | When agent finishes a response | Final consolidation, sync checkpoint |
| `SubagentStart` | When a subagent is spawned | Create subagent context |
| `SubagentStop` | When a subagent finishes | Merge subagent memories |
| `PreCompact` | Before context auto-compaction | Retrieve relevant memories |
| `PostCompact` | After context auto-compaction | Log what was compacted |
| `UserPromptSubmit` | When user submits a prompt | Auto-detect project from cwd |
| `Notification` | When agent shows a notification | Log notification for replay |

---

## 3. Antigravity Hook Configuration

### 3.1 Example `agy.json`

```json
{
  "PreInvocation": [
    {
      "type": "command",
      "command": "automemory hook --agent agy --event boot --project auto"
    }
  ],
  "PreToolUse": [
    {
      "type": "command",
      "command": "automemory hook --agent agy --event pre-tool --project auto"
    }
  ],
  "PostToolUse": [
    {
      "type": "command",
      "command": "automemory hook --agent agy --event post-tool --project auto"
    }
  ],
  "PostToolUseFailure": [
    {
      "type": "command",
      "command": "automemory hook --agent agy --event post-tool-fail --project auto"
    }
  ],
  "TaskCompleted": [
    {
      "type": "command",
      "command": "automemory hook --agent agy --event task-done --project auto"
    }
  ],
  "Stop": [
    {
      "type": "command",
      "command": "automemory hook --agent agy --event stop --project auto"
    }
  ],
  "SubagentStart": [
    {
      "type": "command",
      "command": "automemory hook --agent agy --event subagent-start --project auto"
    }
  ],
  "SubagentStop": [
    {
      "type": "command",
      "command": "automemory hook --agent agy --event subagent-stop --project auto"
    }
  ],
  "PreCompact": [
    {
      "type": "command",
      "command": "automemory hook --agent agy --event pre-compact --project auto"
    }
  ],
  "PostCompact": [
    {
      "type": "command",
      "command": "automemory hook --agent agy --event post-compact --project auto"
    }
  ],
  "UserPromptSubmit": [
    {
      "type": "command",
      "command": "automemory hook --agent agy --event prompt --project auto"
    }
  ],
  "Notification": [
    {
      "type": "command",
      "command": "automemory hook --agent agy --event notify --project auto"
    }
  ]
}
```

### 3.2 How It Works

1. User runs `automemory init` — CLI auto-generates `agy.json` in `~/.automemory_pro/hooks/`.
2. Antigravity reads `agy.json` on startup.
3. Hooks fire automatically on every event — no manual reminder needed.
4. Hooks are out-of-process (separate Rust binary) — safer, no crash risk.
5. `TaskCompleted` hook auto-updates memory (success traces, error logs) — agent doesn't need to be told to "remember this".

---

## 4. Claude Code Hook Configuration

### 4.1 Example `claude.json`

```json
{
  "SessionStart": [
    {
      "type": "command",
      "command": "automemory hook --agent claude --event boot --project auto"
    }
  ],
  "PreToolUse": [
    {
      "type": "command",
      "command": "automemory hook --agent claude --event pre-tool --project auto"
    }
  ],
  "PostToolUse": [
    {
      "type": "command",
      "command": "automemory hook --agent claude --event post-tool --project auto"
    }
  ],
  "TaskCompleted": [
    {
      "type": "command",
      "command": "automemory hook --agent claude --event task-done --project auto"
    }
  ],
  "Stop": [
    {
      "type": "command",
      "command": "automemory hook --agent claude --event stop --project auto"
    }
  ]
}
```

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
| `--event` | Lifecycle event name |
| `--project` | Project identifier (`auto` for auto-detect, or explicit slug) |

---

## 6. Project Auto-Detection

When `--project auto` is used, AutoMemory:

1. Reads current working directory from environment.
2. Matches against registered project paths.
3. Falls back to Git repository root detection.
4. If no match, creates an unregistered project record (local only, no sync).

---

## 7. Hook Installation

### 7.1 Automatic Installation

```bash
automemory init --agent agy
```

- Detects Antigravity configuration directory.
- Backs up existing hook configuration.
- Adds AutoMemory hook commands.
- Provides `automemory doctor` to validate event receipt.

### 7.2 Manual Installation

1. Create `agy.json` in `~/.automemory_pro/hooks/`.
2. Point Antigravity to this file in its configuration.
3. Run `automemory doctor` to validate.

### 7.3 Uninstallation

```bash
automemory disconnect --agent agy
```

- Removes AutoMemory hook commands.
- Restores backup of original configuration.
- Leaves local memory data intact.

---

## 8. Troubleshooting

### 8.1 Hooks Not Firing

- Check `~/.automemory_pro/hooks/agy.json` exists.
- Verify Antigravity is configured to read hook files.
- Run `automemory doctor` to test event delivery.

### 8.2 Hooks Slow Down Agent

- Hooks should only enqueue events (fast).
- If slow, check for network timeouts or database locks.
- Run `automemory doctor` to measure hook latency.

### 8.3 Project Not Detected

- Ensure project is registered: `automemory project list`.
- Check working directory matches registered path.
- Verify Git repository root is correct.

---

## 9. Future Agents

New agent adapters follow the same pattern:

1. Create `<agent>.json` in `~/.automemory_pro/hooks/`.
2. Implement adapter to normalize vendor events.
3. Test with `automemory doctor`.
4. Document in this guide.