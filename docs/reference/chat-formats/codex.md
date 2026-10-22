# Codex chat files (for the transcript reader)

Oct 2026. **Status: written from public descriptions, not from a real file.** This machine has the Codex desktop app's state (`~/.codex/state_5.sqlite`,
`sqlite/codex-dev.db`) but no chats yet (0 threads, no `sessions/` folder), so the reader and its fixture follow the documented format and are
**unverified until someone runs a real Codex session and checks `automemory continuity import codex`**. Treat Codex as "experimental" until then.

## Where
- `$CODEX_HOME` (default `~/.codex`)`/sessions/YYYY/MM/DD/rollout-<start time>-<id>.jsonl`, one file per session.
- Finished or archived sessions can be moved to `$CODEX_HOME/archived_sessions/rollout-*.jsonl` (flat).
- A SQLite index next to them (`state_5.sqlite`, table `threads`: `id`, `rollout_path`, `cwd`, `title`, `first_user_message`, `archived`, git fields, `created_at_ms`) serves
  `codex resume` and the app. The reader does **not** use it: the rollout files carry what is needed, and the index schema changes between versions (this machine: schema 59 migrations).
- Sources disagree on compression (`.jsonl.zst` is mentioned as a retention suggestion for old files); the reader reads plain `.jsonl` only.

## Lines
Every line: `{"timestamp": ISO-8601, "type": ..., "payload": {...}}`.
| type | payload | Reader |
|---|---|---|
| `session_meta` (first line) | `id`, `cwd`, `originator`, `cli_version`, git info; can embed a long system prompt (20-27 KB seen), so the first line is read up to 1 MB | project match by `cwd` (the project folder or any sub-folder), session id |
| `turn_context` | per-turn `cwd`, model, approval policy, sandbox | ignored |
| `event_msg` | `user_message {message}`, `agent_message {message}`, `token_count`, task/turn events | user/assistant text (the clean copy) |
| `response_item` | `message {role, content[{input_text/output_text/text}]}`, `function_call {name, arguments(JSON string), call_id}`, `custom_tool_call {name, input}`, `function_call_output`, `reasoning` | messages only as a fallback; tool calls become one line `→ shell cargo test`; outputs and reasoning are never kept |
| `compacted` | summary from `/compact` | kept as an `artifact` (the agent's own summary) |
| `item.completed` | the `codex exec --json` stream: `agent_message {text}`, `command_execution {command}` | handled like the above |
Injected context (`<environment_context>`, `<user_instructions>`, `# AGENTS.md instructions ...`, developer/system messages) is not the person's words and is dropped.

## Quirks
- Each message is written twice (event for the screen, response item for the model): the reader takes the events and falls back to response items only when a chunk has no events.
- Names drift between versions (`assistant_message` became `agent_message`; `item_type` became `type`): the reader accepts both.
- Forked sessions replay the parent's history; the importer's per-message cursor makes this harmless but the replay appears in the fork's chat.
- Reasoning blocks exist in the files and are never read.

## To verify with a real file (the C7 sample step)
1. Run any short Codex session in a project folder (a question, one command, one edit).
2. `automemory continuity sources` must list "Codex ... chats of this project found: 1".
3. `automemory continuity enable codex`, `automemory continuity import codex`, then `automemory continuity search <word you typed>`.
4. If a message is missing or doubled, send the rollout file (redacted) so the fixture can be replaced with a real sample.

Sources: [Codex session history and rollout format](https://codex.danielvaughan.com/2026/06/01/codex-cli-session-history-local-search-rollout-format-knowledge-mining/),
[session management](https://codex.danielvaughan.com/2026/04/13/codex-cli-session-management-resume-fork-transcripts),
[codeburn provider notes](https://raw.githubusercontent.com/getagentseal/codeburn/main/docs/providers/codex.md). All third-party; none is OpenAI documentation.
