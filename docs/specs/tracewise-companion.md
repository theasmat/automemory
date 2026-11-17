# Tracewise as AutoMemory's companion: locks and file awareness move out

Status: plan, decided with the maintainer in Nov 2026. P1 (Tracewise, merged), P2 (AutoMemory client), P3 (old locks removed, proxies) and P4 (setup UX) are built; P5 is half done: measured and controlled here, the live two-agent run is yours. Roadmap: [roadmap](../roadmap.md). Tracewise: [github.com/cmdworks/tracewise](https://github.com/cmdworks/tracewise) (`~/Developer/project/tracewise`).

## 1. Why

AutoMemory sees only what an agent's hooks tell it. Tracewise sees every writer on the machine: agent hooks, the shell (command wrappers), git (wrapper and hooks), a file watcher, and people. Locks and "who changed this file" are about files, so they belong with the tool that sees the files. Evidence from the first live runs of AutoMemory:

| What went wrong | Why Tracewise does not have it |
|---|---|
| Locks leaked between projects (two projects with `src/app.py` collided) | Tracewise keeps its data per repository (`.git/trail`); it cannot see another project's locks |
| A finished agent's locks kept another agent out for 10 minutes | Tracewise releases a session's locks at Stop and at session end |
| Writes from a shell or a script were invisible until a `git status` diff | Tracewise records them as they happen |
| AutoMemory carries lock tables, a reconcile pass, a coverage report, an idle-holder watcher and a lock dashboard page that all duplicate it | One implementation, in one place |

Two products stay separate. AutoMemory is useful alone (memory, decisions, briefs, rooms, library, sync) and Tracewise is useful alone (history, undo, reasons, locks). Together, AutoMemory uses Tracewise for everything about files.

## 2. The split

| Stays in AutoMemory | Moves to Tracewise (AutoMemory asks it) |
|---|---|
| Notes, trust tiers, recall, verification, staleness | Locks: take, renew, release, expire, `lock list/release/wait` |
| Decisions, the brief, hand-offs, rooms, the library | Shell-write tracking: what a command changed that no lock covered |
| Sync, backup, export/import, tidy, eval | Coverage: share of file changes that were locked first |
| Command guardrails (destructive shell commands) | Collision notices: "files other agents hold now", "held by X" |
| The Inbox, the password gate, `reset` | The dashboard's **Agents & Locks** and **Replay** pages read Tracewise's data |

AutoMemory with no Tracewise installed has **no locking at all**. It says so in `connect`, `doctor` and the dashboard, and offers to install Tracewise. Memory is unaffected.

## 3. Nested hooks

One hook per agent, registered by `automemory connect` as today (`automemory hook --agent X --event E`). When the companion is on, that hook calls Tracewise inside itself:

```
agent ──hook──▶ automemory hook ──(same stdin, nested)──▶ tracewise hook E --from X --neutral
                    │                                         │
                    │◀── {"api":1,"decision","reason","context":[...]} ──┘
                    └─ runs its own work (memory, brief, guardrail), merges, answers in X's schema
```

- **Order:** Tracewise first. A lock denial ends the call before AutoMemory does any memory work.
- **Neutral output:** Tracewise's new `--neutral` flag prints agent-independent JSON (`api`, `decision` allow/ask/deny, `reason`, `context` lines, `notes`). AutoMemory already has the per-agent emitters (Claude, Codex, Antigravity's strict schema), so it renders the merged result once.
- **Merge:** the strictest decision wins; reasons are joined; context lines join AutoMemory's own injection under one budget.
- **Fail open:** a missing, slow (over 150 ms) or erroring Tracewise is logged and skipped; the agent is never blocked by the companion being broken.
- **No double hooks:** an agent must not have both products' hooks registered. `automemory connect` removes a Tracewise-only registration for that agent when the companion is on (and says so); `tracewise agent install` refuses while AutoMemory nests it.
- **Budget:** the whole hook stays under 25 ms p95 (Tracewise about 7 ms plus AutoMemory about 9 ms plus the spawn). Measured before the flip, not assumed.

### Detecting the companion
`[companion] tracewise = "auto" | "on" | "off"` in `config.toml` (default `auto`). Auto means `tracewise` is on `PATH` or in `~/.tracewise/bin` and `tracewise companion --json` answers `{"api":1,...}`. The answer is cached for a minute so hooks do not probe on every call. An unknown API number means "too new or too old": the companion is off and `doctor` says which side to update.

## 4. Changes in Tracewise (P1: built and released in Tracewise 0.4.0, 2026-11-16)

| Change | Contract (as built) |
|---|---|
| `tracewise hook <event> --from <agent> --neutral` | Reads the agent's raw payload on stdin, does the same work as the agent-schema hook (takes and renews locks, records, builds notices) and prints **one line** `{"api":1,"event":"pre-edit\|post-edit\|pre-command\|post-command\|stop\|session-end\|prompt\|none","decision":"allow\|ask\|deny","reason":null\|"...","context":["..."]}`. Exit code 0 always; a bad payload, an unknown agent or an event Tracewise does not handle is `allow` with event `none`. Agents: claude, codex, cursor, gemini, antigravity, and adapter files. Without the flag the output is exactly as before |
| `tracewise companion [--json]` | `{"api":1,"name":"tracewise","version":"0.4.0","features":["neutral-hook","locks","lock-list-json","notices"]}`; works outside a repository |
| `tracewise lock list --json` | The record's own fields (`path`, `holder`, `since`, `expires`, `intent`) plus `agent`, `session` and `expires_in_s` |

Two items in the first draft of this plan turned out to need no new command:
- **Notices** are the `prompt` event's `context` (what other actors changed in files this session works on, the first-prompt digest, overlaps), already in the neutral answer.
- **Coverage** in Tracewise means how much of the capture is in place (per-agent hooks, shell block, shims, git wrapper, daemon) and how much of the history has a named actor; it is already in `tracewise doctor --json` under `coverage` and `attribution`. It replaces AutoMemory's "share of file writes locked first", which Tracewise does not measure.

Tracewise releases a session's locks at Stop and at session end, and keeps its data per repository, which is what removes the two lock problems seen in the live runs.

Checks: 5 new integration tests (`tests/neutral_hook.rs`), 362 workspace tests, clippy and `cargo deny` clean, and the read-only acceptance files for hooks and locks (t03, t06, t13, t15, t19, t28, t28b, t38, t53) pass against the branch build.

## 5. Changes in AutoMemory

1. **Companion client:** detect, cache, nested call, merge (`companion.rs`), with a fake `tracewise` script for contract tests.
2. **Remove locking:** the `leases` table (schema migration drops it; leases are ephemeral), `lock.acquire` and `lock.release` MCP tools, auto-lease in the hook pipeline, shell reconcile, coverage, the idle-holder watcher in `serve`, `stop_grace_seconds`. A removed tool answers "was removed: locks come from Tracewise (`tracewise lock`)".
3. **Proxies:** `lock.status` (MCP), `automemory locks` and `unlock` call `tracewise lock list|release --json` and print an install hint when it is absent. `stats` collisions come from Tracewise; the coverage line comes from `tracewise doctor --json`.
4. **Dashboard:** Agents & Locks and Replay read through the proxies; with no companion they show "Install Tracewise for file locks and history".
5. **Setup:** `automemory setup --tracewise auto|yes|no` (the installer asks once), `doctor` shows companion state, API version and the nested call's last latency, `connect` warns about a duplicate Tracewise hook.
6. **Docs:** README, features, architecture, decisions, `LONG_SESSION_PLAN.md` (S4 now tests Tracewise locks).

Follow-up (Nov 2026): the dashboard also shows Tracewise's file changes, read-only: [dashboard-changes.md](dashboard-changes.md).

## 6. Phases and exit gates

| Phase | Work | Exit gate |
|---|---|---|
| P1 | Tracewise: `--neutral`, `companion`, lock list JSON, on a branch | **Done on branch `companion-p1`** (not merged): neutral output covered for Claude-style and Antigravity payloads, the hook and lock acceptance files green |
| P2 | **Done (uncommitted).** AutoMemory companion client, nested call and merge, built **beside** the existing locks (`[companion] own_locks`, default true). Measured: hook p50 10.4 ms without, 13.9 ms with the nested call, p95 14.6 ms (budget 25). `tests/scenarios/companion.py`: 13 checks | Contract tests with a fake Tracewise; one scenario with the real one; no double reply on Antigravity |
| P3 | **Done (uncommitted).** Flip: lock code, tables (schema 23), `lock.acquire/release`, `[locks]`, `[companion] own_locks` removed; `locks`/`unlock`/`stop`/`lock.status`/dashboard routes proxy to Tracewise (`companion::held_locks`, `json_in`); 47 tools; scenarios `collision`, `lock_grace`, `locking_v2` deleted, `companion.py` covers the replacement. Nuxt source and compiled dashboard bundle updated (Agents & Locks and Settings name Tracewise) | Full script suite green with and without Tracewise; migration tested on a database that has leases |
| P4 | **Done (uncommitted).** `automemory companion [status\|install\|auto\|on\|off]` (checksummed install from the Tracewise release, asks first), `install.sh` asks once on macOS (`AUTOMEMORY_TRACEWISE=yes\|no` skips the question), `doctor` shows version, API, probe time and duplicate hooks, `connect` removes Tracewise's own hooks for the agent. Not done: `tracewise agent install` refusing while AutoMemory nests it (a Tracewise change). Original scope: `setup` and `doctor` UX, installer question, Windows note (Tracewise is macOS only: no locks there, memory unaffected) | Fresh-machine run of both options |
| P5 | Live: the S4 two-agent test with Tracewise locks, hook p95 measured, memory-off control | **Automated half done (uncommitted); live half waits for you.** Measured on the release build, real Tracewise main build, 50 calls each: pre-tool p50 10.8 → 15.0 ms, p95 11.4 → 17.3 (budget 25: met); post-tool p95 9.2 → 13.6; prompt p95 27.9 → 34.2 and stop p95 31.3 → 36.1 (above 25 even without the companion: they build the resume pack and checkpoint; the nested call adds about 4 ms). Memory-off control: same decision, same hook replies for session start, prompt, edit, a blocked `rm -rf /` and stop with the companion off and on; the notes list is identical. Live S4 is rewritten in `tests/manual/LONG_SESSION_PLAN.md` (Second agent told who holds a file before it edits; no leftover lock after a finished turn)

## 7. Risks

- **Latency:** two products, one extra process per tool call. Measured in P2 against the 25 ms budget; if exceeded, the nested call moves to a long-lived Tracewise socket.
- **Version skew:** guarded by the `api` number; either side can be updated alone.
- **Windows and Linux:** no companion yet, so no locks there. Stated, not hidden.
- **Two stores:** file history lives in Tracewise, memory in AutoMemory; the dashboard joins them by project folder.
- **Existing users:** leases are short-lived, so dropping the table loses nothing worth keeping; the migration notes it.
