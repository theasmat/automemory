# File locking

AutoMemory does not lock files. Locks, shell-write tracking and "who changed this file" come from the optional companion [Tracewise](https://github.com/cmdworks/tracewise), which sees every writer on the machine (agent hooks, the shell, git, a watcher, people) and keeps its data per repository.

- **Nested call:** AutoMemory's own hook calls `tracewise hook <event> --from <agent> --neutral` with the agent's payload and merges the answer (strictest decision wins; a missing, slow or other-API Tracewise is skipped). Protocol and phases: [the spec](../specs/tracewise-companion.md).
- **No Tracewise:** nothing is locked. `connect`, `doctor`, `automemory locks` and the dashboard say so. Memory, decisions, briefs, rooms and sync are unaffected.
- **Reading and releasing:** `automemory locks`, `automemory unlock <file>`, `automemory stop` and the MCP tool `lock.status` ask Tracewise (`tracewise lock list|release --json`). `lock.acquire` and `lock.release` were removed.
- **Capture coverage:** `automemory locks --coverage` shows `tracewise doctor --json` (`coverage`, `attribution`).
- **Upgrade:** schema 23 drops the `leases`, `lock_alerts`, `lock_coverage` and `workspace_snapshots` tables (leases were short-lived; the rest was derived data). The `[locks]` section and `[defaults] auto_lease` in `config.toml` are ignored.
