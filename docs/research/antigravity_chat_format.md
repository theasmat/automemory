# Antigravity chat files (reader notes)

Seen Oct 2026 (macOS). Read by `continuity/readers/antigravity.rs`; if an Antigravity update breaks the reader, start here. Everything
below was found by inspecting the app's own files read-only; it is not a documented format.

- Apps and folders: `~/.gemini/antigravity`, `~/.gemini/antigravity-cli`, `~/.gemini/antigravity-ide` (`antigravity-backup` is a copy and is
  skipped). Each has `conversations/<conversation id>.db`, `annotations/<id>.pbtxt`, `brain/<id>/`.
- `conversations/<id>.db` is SQLite (journal mode may be WAL while the app runs; the reader opens it read-only, falling back to
  `immutable=1`). Tables: `steps(idx, step_type, step_payload, …)`, `trajectory_metadata_blob`, `gen_metadata`, `executor_metadata`, others unused.
- `step_payload` is protobuf without a published schema. Only these paths are decoded (field numbers):
  - `step_type 14` user input: `19 → 2` text (also `19 → 3 → 1`).
  - `step_type 15` planner response: `20 → 1` visible reply (duplicated at `20 → 8`); `20 → 3` is the model's reasoning summary (never read);
    `20 → 7` repeated tool call: `2` name, `3` JSON arguments (`CommandLine`, `Path`, `AbsolutePath`, `TargetFile`, `Query`, `Url`, …).
  - `step_type 132` tool results, `90` injected messages (including AutoMemory's own hook text), `17` errors: skipped.
  - Timestamp: `5 → 1 → 1` seconds since the epoch.
- The conversation's workspace is `trajectory_metadata_blob` field `1 → 1`, a `file://` URI; a conversation belongs to a project when it equals the project folder.
- Title: `annotations/<id>.pbtxt` line `title:"…"`.
- Plans the agent wrote: `brain/<id>/*.md` (the `.metadata.json` and `.resolved` files are skipped); imported once per modification time as an `artifact`.
- Cursor: `"<last step idx>;<newest plan file time>"`.
- Not handled: images, `conversation_summaries.db` (not needed; plans cover the agent's own summaries), steps of sub-trajectories.
- Fixture: `tests/continuity/make_antigravity_fixture.py` writes a synthetic conversation in this format.
