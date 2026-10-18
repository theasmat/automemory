# AutoMemory live trial log

Trial project: `/Users/as/Developer/test/automemory` (the "ecom" shop, plain Python, one commit `Ecom starter`).
Rule for this trial: **no global hooks**. AutoMemory is registered only inside the trial folder
(`automemory connect claude`, `automemory connect agy`, no `-g`).
Binary under test: `~/.cargo/bin/automemory` (always the installed one, never `target/debug`).
Fresh start: old data archived to `~/.automemory.archive-20261019-0133`, old folder archived to
`/Users/as/Developer/test/automemory-archive-20261019-0133`.

How we work: one test case at a time. Run it, paste the report (what you typed, what the agent
answered, command output, anything odd). The result is recorded below, then the next case is given.
Statuses: `PENDING`, `PASS`, `FAIL`, `PARTIAL`, `BLOCKED`.

## Scoreboard

| # | Test | Status | Date |
|---|---|---|---|
| T0 | Baseline: clean machine, local-only registration | PASS | 2026-10-19 |
| T1 | Claude Code: hooks fire and events are recorded | PENDING | |
| T2 | Antigravity: hooks fire, no hook errors | PENDING | |
| T3 | Shared knowledge: a decision made in Claude is known by Antigravity | PENDING | |
| T4 | Two agents, one file: automatic lock and collision message | PENDING | |
| T5 | Hand-off: a new session continues where the last one stopped | PENDING | |
| T6 | Safety: a destructive command is denied | PENDING | |
| T7 | Instruction files: `export-rules` block read by another agent | PENDING | |
| T8 | Dashboard: pages show real data, tour and Settings work | PENDING | |
| T9 | Human password gate | PENDING | |
| T10 | Clean removal: `disconnect` leaves nothing behind | PENDING | |

## Test cases

### T0 Baseline
Goal: prove the machine is clean and registration is local only.
Steps: see the case message in the chat (commands to run and paste).
Pass when: installed binary is the current build; no global hooks or MCP entries; the trial folder has
its own hooks for Claude and Antigravity; the database is fresh; `doctor` shows workspace hooks active.

### T1 Claude Code hooks
Goal: a Claude Code session in the trial folder is captured by AutoMemory.
Pass when: after a small edit, `automemory memory list` / the dashboard shows the session's events or a
checkpoint, and Claude showed no hook error.

### T2 Antigravity hooks
Goal: same for Antigravity (CLI, App or IDE, say which).
Pass when: the `automemory` MCP server is listed, a tool call produces events, and there is no
"unknown field" or other hook error.

### T3 Shared knowledge
Goal: decision recorded by Claude is known to Antigravity without exploring files.
Pass when: Antigravity states the decision and the note about integer cents without a long file search.

### T4 Collision
Goal: Claude edits `shop/checkout.py` over several steps; Antigravity tries the same file.
Pass when: Antigravity is told (or blocked in Strict mode) that Claude holds the file, `automemory locks`
showed the lock live, and Antigravity's edit works after Claude finishes.

### T5 Hand-off
Goal: a new session starts from the last one's state.
Pass when: the first reply mentions the last hand-off, branch and uncommitted files; `automemory resume` agrees.

### T6 Safety
Goal: `rm -rf /` style command is denied.
Pass when: the call is denied with `CRITICAL STOP` and nothing ran.

### T7 Instruction files
Goal: `automemory export-rules` puts decisions into AGENTS.md; another agent reads them.
Pass when: the block is present, user text untouched, and the agent lists the decisions in force.

### T8 Dashboard
Goal: Home, Project brain, Agents & locks, Inbox, Sync, Safety, Settings show real data; tour replays from Settings.
Pass when: data matches what the CLI shows and no page is blank or errors.

### T9 Human password gate
Goal: after `automemory rule passwd`, approving in the UI asks for the password; wrong one is rejected.

### T10 Clean removal
Goal: `automemory disconnect all --everywhere` then `--dry-run` reports 0 (ignoring path-substring false positives).

## Results

(Newest last. Each entry: date, test, status, what was run, what happened, follow-ups.)

### 2026-10-19 T0 Baseline: PARTIAL (first report)

Reported: output of `automemory connect claude`, `connect agy`, `doctor`, `git status --short`, `ls ~/.automemory`.
Not reported yet: which binary runs, global hook grep, registrations list, `mode get`, `locks`, unit tests.

Findings
1. **Registration is local and correct.** `connect claude` wrote `.mcp.json`, `.claude/settings.local.json`, a CLAUDE.md block and a
   git post-commit hook; `connect agy` wrote `.agents/hooks.json`, MCP config, skill, rules. Re-running it is harmless (idempotent).
   `doctor`: "Workspace hooks active", "Claude/Codex hooks not installed" (global, as intended).
2. **The machine was not at zero.** Before T0 something already ran in the trial folder: `shop/pricing.py` modified, `tests/test_pricing.py`
   new, 4 active memories, a decision and a note recorded, "100% of 2 file changes covered". Looks like an earlier prompt run (the
   first trial prompts) after the cleanup. Treated as pre-T0 activity; needs a reset to start from 0.
3. **Shell alias risk.** In this shell `automemory` is aliased to `target/debug/automemory` (stale build). Hooks always run
   `~/.cargo/bin/automemory`, so CLI checks and hooks could disagree. Fix: `unalias automemory` and remove the alias from `~/.zshrc`.
4. **Bug found and fixed:** `doctor` printed "Missing directories. Run 'automemory init'" on a fresh install although the database
   worked (it needs `~/.automemory/hooks`, which is only created on first use). `doctor` now creates the folders first.
   Installed to `~/.cargo/bin/automemory` (Oct 19 01:50); verified: directories check OK.

Follow-up: reset to zero, remove the alias, rerun the T0 checks with the installed binary.

### 2026-10-19 T0 Baseline: PASS (second report, after reset)

Checked: `which -a automemory` shows only `~/.cargo/bin/automemory` (alias gone); version 0.1.0; no AutoMemory entries in
`~/.claude/settings.json` (count 0) and no `~/.codex/hooks.json`; dry-run lists only entries inside the trial folder plus the two
known path-substring false positives (`~/.claude.json`, Claude Desktop config); `doctor`: all health checks passed, 0 active
memories (the 2 "archived" rows are doctor's own benchmark), no file changes recorded; unit tests OK; `mode get` says the project
is not in the database yet and defaults to VIBE.

Notes
- "Pending event queue: 1 unprocessed event" right after the reset: probably the benchmark or one hook call; not a failure. Watch if it grows.
- The project registers itself on the first hook run, so Strict mode can only be set after T1 (`mode set strict` now would have no project to attach to).
- `connect` writes the AutoMemory block into AGENTS.md / CLAUDE.md / GEMINI.md, so the checklist already shows "Instruction files" done
  before any decision exists (block says how to use the tools; decisions are added later by `export-rules`).

