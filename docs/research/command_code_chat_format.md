# Command Code chat files (for the transcript reader)

Oct 2026. **Status: layout from the public docs, field names guessed, nothing checked against a real file.** Command Code (commandcode.ai, npm `command-code`, runs as `cmd`) is
closed source and not installed on this machine; its docs describe where sessions live but not the record fields. Treat the reader as experimental.

## Where (docs page /docs/sessions)
- `~/.commandcode/projects/<slug of the working folder>/<session-id>.jsonl`: the transcript, append-only. First line a header (session id, creation time, working folder);
  then entries: user messages, model replies (with token usage and cost), model/effort changes, compaction summaries. Entries form a tree through parent pointers
  (rewind and branches append new entries and move a pointer; the file is never rewritten).
- Beside it: `<id>.meta.json` (title, model, lineage, compaction stats), `<id>.share.json`, `<id>.checkpoints.jsonl` (snapshots for `/rewind`), `<id>.prompts.jsonl` (prompt history).
- `~/.commandcode/file-history/<session-id>/` (file backups), `~/.commandcode/auth.json` (login key: never read), `/tmp/commandcode-<uid>/...` (scratchpads).
- `/session-file` inside a session prints its id and path.

## What the reader does
- Finds transcripts by the working folder in the header line (the project folder or any sub-folder); falls back to a guessed folder slug (path with non-alphanumerics as `-`) when the header has no folder field.
- Skips `.checkpoints.jsonl` and `.prompts.jsonl`, the auth file and everything outside `projects/`.
- Per entry, tolerant of field names (`message.role` / `role` / `type`, `content` as text or parts): user and assistant text, one line per tool call, compaction summaries as artifacts. Reasoning, tool results, model changes and unknown entries are not kept.
- Title from `<id>.meta.json`; byte-offset cursor, whole lines only.
- Known limit: after a rewind the abandoned branch is still in the file, so its messages are read too.

## To verify with a real file
1. Run a short session in a project folder: a question, one command.
2. `automemory continuity sources` must show "Command Code ... found: 1".
3. `automemory continuity enable command-code`, `automemory continuity import command-code`, `automemory continuity search <a word you typed>`.
4. If a message is missing, doubled or a branch you abandoned shows up, send the redacted `.jsonl` so the fixture can become a real sample.

Sources: [Command Code docs: sessions](https://commandcode.ai/docs/sessions), [quickstart](https://commandcode.ai/docs/quickstart).
