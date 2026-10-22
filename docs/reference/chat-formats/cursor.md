# Cursor chat files (for the transcript reader)

Oct 2026. **Status: written from public reverse-engineering notes; nothing checked against a real file.** Cursor is not in use on this machine (its folder holds only the
`mcp.json` AutoMemory wrote). Cursor publishes no schema and changes it between versions (a 3.0 release in April 2026 added a central chat index). Treat the reader as experimental.

## Where
- macOS `~/Library/Application Support/Cursor/User/`, Linux `~/.config/Cursor/User/` (Windows `%APPDATA%\Cursor\User`: not read yet).
- `globalStorage/state.vscdb`: SQLite with tables `ItemTable(key, value)` and `cursorDiskKV(key, value)`; values are JSON (text or blob); WAL mode while Cursor runs.
  - `cursorDiskKV` `composerData:<composerId>`: one conversation: `name`, `createdAt`, `fullConversationHeadersOnly` = ordered `[{bubbleId, type}]` (older chats keep messages inline).
  - `cursorDiskKV` `bubbleId:<composerId>:<bubbleId>`: one message: `type` 1 = user, 2 = assistant, `text`, `createdAt` (ms); tool bubbles carry `toolFormerData`; reasoning is separate.
  - `ItemTable` `composer.composerHeaders` (3.0+): central index, each entry with a `workspaceIdentifier.id` that names the workspace folder.
- `workspaceStorage/<id>/workspace.json` (`folder`: `file:///path`) says which project a workspace is; its `state.vscdb` `ItemTable` `composer.composerData` lists that workspace's `allComposers`.
- Not read: agent transcripts at `~/.cursor/projects/<path>/agent-transcripts/` (sources disagree whether they are `.txt` or `.jsonl`, and the database is authoritative), and `~/.cursor/chats/**/store.db`.

## What the reader does
Finds the workspace whose folder is the project (or contains it), collects its composer ids (workspace index plus the 3.0 central index), then reads each conversation in header order from the global
database, opened read-only (the file is never copied or written). User and assistant text, one line per tool call; reasoning, tool results, checkpoints and everything else are not kept. The reader's cursor is a chat position
count (`n:<headers read>`), so only new messages are read next time.

## Known limits
- A chat started from Cursor's welcome screen has no folder and is not found. A chat never reopened since 3.0 may be missing from the central index (the workspace index still lists it).
- If Cursor holds the database locked, the read fails for that chat and is retried next time.
- Edited or deleted messages: only appended headers are read.

## To verify with a real file
1. Open a folder in Cursor, ask the agent one question.
2. `automemory continuity sources` should show "Cursor ... found: 1" (Cursor may need to be closed for the index to be flushed).
3. `automemory continuity enable cursor`, `automemory continuity import cursor`, `automemory continuity search <a word you typed>`.
4. If nothing is found, run `sqlite3 -readonly "<globalStorage>/state.vscdb" "select key from cursorDiskKV where key like 'composerData:%' limit 3"` and send the key names (not the contents).

Sources: [how Cursor stores chats (cursaves)](https://github.com/Callum-Ward/cursaves/blob/main/docs/how-cursor-stores-chats.md), [Cursor chat history storage](https://extracurricular.ai/en/blog/cursor-chat-history-storage/), [Cursor IDE backend notes (agentgrep)](https://agentgrep.org/backends/cursor-ide/). All third-party.
