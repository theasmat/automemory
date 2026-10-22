# Claude Code chat files (reader notes)

Seen Oct 2026 on Claude Code 2.x. Read by `continuity/readers/claude_code.rs`; if a Claude update breaks the reader, start here.

- Location: `~/.claude/projects/<folder>/<session-id>.jsonl`; `<folder>` is the project path with every non-alphanumeric character replaced by `-`
  (`/Users/me/shop` → `-Users-me-shop`).
- One JSON object per line. `type` values seen: `user`, `assistant`, `system`, `attachment`, `ai-title`, `last-prompt`, `mode`,
  `file-history-snapshot`, plus internal ones (`api-request*`, `cost-state`, `atis-latch`). Only `user`, `assistant` and `ai-title` are used.
- `user.message.content` is a string, or a list of blocks; a list with only `tool_result` blocks is tool output (skipped).
  `isMeta: true` marks command echoes and injected context (skipped); `isSidechain: true` marks sub-agent turns (skipped).
- `assistant.message.content` is a list of `text`, `thinking` and `tool_use` blocks. Text is kept; `thinking` is private reasoning and is
  never stored; each `tool_use` becomes one line `→ Name detail` (no tool output).
- `<system-reminder>…</system-reminder>` blocks inside user text are injected hook context and are removed.
- `ai-title.aiTitle` is the session's title. Timestamps are RFC 3339 in `timestamp`.
- Incremental reads: the cursor is a byte offset after the last complete line; a half-written last line waits for the next read; a file
  that became shorter is read from the start.
- Not handled: images, attachments, compaction summaries (they appear as ordinary user text if present).
