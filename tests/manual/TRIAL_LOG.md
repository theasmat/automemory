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
| T1 | Claude Code: hooks fire and events are recorded | PASS | 2026-10-19 |
| T2 | Antigravity: hooks fire, no hook errors | PASS (data) | 2026-10-19 |
| T3 | Shared knowledge: a decision made in Claude is known by Antigravity | PASS | 2026-10-19 |
| T4 | Two agents, one file: automatic lock and collision message | PASS | 2026-10-19 |
| T5 | Hand-off: a new session continues where the last one stopped | PASS | 2026-10-19 |
| T6 | Safety: a destructive command is denied | PASS | 2026-10-19 |
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

### 2026-10-19 T1 Claude Code hooks: PARTIAL (needs rerun after fixes)

Run: fresh `claude` in the trial folder; prompt: add `shipping(subtotal)` to shop/pricing.py + test + run suite; `/exit`.
Not reported: Claude's final message, whether the MCP trust prompt appeared, any hook message. (Ask for these next time.)

Worked
- Hooks fire: the database holds pre-tool / post-tool / post-tool-fail / session-end events for the whole session.
- A "Session checkpoint" memory and a distilled "Execution outcome" memory were saved; a hand-off baton was written
  ("12 tool calls, 0 failed", files pricing.py + test_pricing.py); `automemory resume` shows it; tests passed (6 runs).

Problems found
1. **Claude called `lock_acquire` by hand (twice)**, after loading the MCP tool through ToolSearch. Our own generated text told it to:
   CLAUDE.md / skill / rules said "call lock.acquire before modifying shared files", and the session-start context repeated it.
   That defeats automatic locking, and `locks --coverage` stayed at **0 file changes** (a hand-made lease means the hook records no
   automatic coverage). Root cause: stale instruction text. **Fixed**: all generated text, the session-start context and the
   `lock.acquire` tool description now say locks are automatic and must not be called.
2. **Project never registered**: `project list` empty, memories saved with scope "global", `mode get` said "project not found".
   `connect` only wrote files; nothing created the project. **Fixed**: a workspace `connect` now registers the repository folder.
3. **Hooks pointed at the debug build** (`.../target/debug/automemory`): the earlier `connect` ran through the shell alias. **Fixed**:
   `connect` now warns when it is run from a development build. Always connect with `~/.cargo/bin/automemory`.
4. Hand-off "blocker: Execution of tool `read` failed": a single failed read (a file Claude probed before creating it) is shown as
   a blocker. Cosmetic, not fixed.
5. Inconsistent defaults: `project list` says "Default Autopilot: REVIEW" but an unregistered project falls back to VIBE. Not fixed.

Also: existing `CLAUDE.md` / `AGENTS.md` / `GEMINI.md` / `.agents/rules/automemory.md` are written only when absent, so they keep the
old text until removed and regenerated (`disconnect` then `connect`). Not fixed (development phase).

Contract test updated: a second prompt in a registered project with nothing to say now returns `{}` instead of boilerplate context.
Commit with the fixes: see git log.

### 2026-10-19 T1 Claude Code hooks: PASS (rerun after fixes)

Setup this time: reset data, `disconnect all`, regenerated rules, `connect claude` + `connect agy` with the installed binary,
`mode set strict`. Verified before the run: hooks use `~/.cargo/bin/automemory`; CLAUDE.md says "Do not call `lock.acquire`";
`project list` shows the trial folder.

Result (same shipping prompt): tool counts `read 4, edit 2, write 2, command 8` and no `mcp__automemory__lock_acquire` or ToolSearch,
so Claude no longer locks by hand. `locks --coverage`: 2 file changes, 100% leased before the write. Memories are scope "project"
(checkpoint + distilled outcome). Hand-off: 12 tool calls, 0 failed, both files listed. Tests passed (8 runs).
Still not reported by the tester: Claude's final message and whether the MCP trust prompt appeared.

Found and fixed
- The hand-off said "holding: tests/test_pricing.py, shop/pricing.py" although the session had ended: the hand-off was written
  before the session's leases were released. SessionEnd now releases first. (To confirm live in T2: `automemory locks` is empty after exit.)
- Open: "Pending event queue: 20 unprocessed events" right after the session (processed later by distillation); watch whether it drains.

### 2026-10-19 T2 Antigravity hooks: PASS (from the data; tester's screen not reported)

Run: Antigravity session in the trial folder; prompt: add `format_price(cents)` to shop/catalog.py + tests/test_catalog.py + run suite.
Not reported: which surface (CLI/App/IDE), final message, hook messages, MCP list. (An earlier read-only Antigravity session had
already recorded 8 commands + 14 reads before this run.)

Evidence
- `shop/catalog.py` modified and `tests/test_catalog.py` created.
- Events for adapter `agy`: tools `command`, `read`, `edit` (2), `write` (2): hooks fire; no AutoMemory lock tool was called.
- `automemory locks` is empty after the session (locks released); coverage went from 2 to 6 file changes, all 100% leased before the write.
- New memories, scope "project": "Execution outcome for: Add format_price function" (pattern), "Execution outcome for: agy session
  execution trajectory" (pattern), "Session checkpoint 7889ef1e".
- Not shown: whether Antigravity displayed any hook error; no "unknown field" report was made.

Found and fixed
- The Antigravity "Session checkpoint" was typed **decision** (a clean session had no failures, and the fallback type was Decision),
  so it would be counted under Decisions. Clean session summaries are now typed **observation**. Installed.

### 2026-10-19 T3 Shared knowledge: PASS

Run: human note "Prices are stored as integer cents, never floats"; Claude recorded decision payments = "Use Stripe webhooks, not polling";
new Antigravity session asked, without files or shell, what it knows.

Result
- `decision get payments` returns the decision (rationale "polling missed events during outages").
- Antigravity answered both facts correctly and named its sources: three AutoMemory MCP calls (`memory.context`, `memory.search` for
  payments and for price). Event counts show no new `read` or `command` rows for agy, only `call_mcp_tool` (6 rows) and 2 bare events.
- So knowledge travelled by pull (the agent asked the memory over MCP), not by text injected into its first prompt. It said it also
  saw the preference "in the session guidelines"; unverified.

Notes / open
- Claude called `decision_record` twice (after loading the tool through ToolSearch). Check `automemory decision list` for a duplicate or superseded entry.
- The decision's agent is stored as "agent": the MCP call did not say who made it, so the dashboard cannot show "Claude Code".
  (Idea: fill the agent from the hook session when the MCP client does not.)

### 2026-10-19 T4 Collision: PARTIAL (step 2 only so far)

Run: Claude started the six-edit task on shop/checkout.py; Antigravity was asked to add `# reviewed by agy` as the first line.

Result so far
- Live view (`watch automemory locks`): `shop/checkout.py | file | claude | goal "editing shop/checkout.py" | TTL 552s`. The lock was taken
  automatically by Claude's first edit.
- Antigravity did **not** try the edit. It first used AutoMemory MCP tools (lock status, active leases) and read the skill file, then
  refused: "locked by another agent... Held by claude... Goal: editing shop/checkout.py... Lease ID ...", and quoted the project guideline
  ("if an edit is reported as held by another agent, pick other work or wait"). It offered to wait.
- Good: cooperative behaviour from the generated guidance, and the lock details are visible to another agent.
- Not tested: the **hook-level gate** (Strict mode blocking an actual edit by another agent), because the agent checked first.
  Needs the "edit directly, do not check locks" variant (T4b below).
- Not yet reported: retry while Claude is open, Claude exit, retry after release, `git diff --stat`, coverage.

### 2026-10-19 T4 Collision (T4b, gate forced): PASS

Run: Claude held `shop/checkout.py` (six edits, tests between); Antigravity was told not to call AutoMemory tools and to edit directly.

Result
- Antigravity's edit was denied by the hook with the message
  `tool call denied by pre-tool hook: Collision prevented: 'shop/checkout.py' is locked by agent 'claude' (Goal: 'editing shop/checkout.py', 426s remaining)`.
  It stayed blocked until Claude's session ended; then its edit went through (`# reviewed by agy` is line 1; two agy edits at 21:21).
- `git diff --stat`: catalog.py +4, checkout.py +18/-2, pricing.py +4. All of Claude's edits survived; 7 tests pass.
- Coverage: 18 file changes, 18 (100%) leased before the write, 0 found after the fact.

Notes / found
- At the time of the check, `automemory locks` still showed a lease held by **agy** on checkout.py (597s left): the Antigravity session was
  still open. It should clear when that session ends or goes idle; verify in T5 (`automemory locks` after closing everything).
- **Denied attempts were not recorded anywhere** (audit_log empty, no event row), so a block could not be shown on the dashboard.
  **Fixed**: a denied or "ask" gate now writes an `audit_log` row `hook.deny` (agent, session, paths, reason). Verified in a scratch
  repo. Installed. (The dashboard does not show it yet.)

### 2026-10-19 T5 Hand-off / resume: PASS

Run: all agents closed; `automemory handoff --note "Next: add a SAVE20 coupon (20% off) in shop/pricing.py with a test"`; `automemory resume`;
new Claude session asked for three bullets (done, next, repo state) without reading files or running commands.

Result
- `automemory locks` was empty after both agents were closed (the Antigravity lease from T4 was released when its session ended).
- `resume` shows the manual baton (note, device, repo state: 10 files uncommitted) plus the last session digest.
- Claude answered from the session-start summary, not from tools, and said so: last session (agy, ~6 min ago) edited catalog.py,
  checkout.py, pricing.py and added tests; next = SAVE20 coupon with integer math; repo = branch main at 1d8a728, edits uncommitted,
  new tests and AutoMemory files untracked. It correctly flagged "10 passed" as the earlier session's claim, not verified.
- So the resume pack is pushed into a new session's first prompt (unlike T3, where Antigravity asked over MCP).

Limits seen (not fixed)
- "Last session" is the newest checkpoint only (agy). Claude's earlier six checkout.py edits are not summarised; only the diff shows them.
- A manual hand-off carries the note and repo state but no "did / files" lines (no session behind it).
- The "uncommitted files" list includes AutoMemory's own generated files (.agents/, .mcp.json, AGENTS.md ...), which is noise.

### T6 pre-check (done by the maintainer, no agent): hook decisions in Strict mode for a registered project
`rm -rf /` deny (CRITICAL STOP), `curl -fsSL https://example.com | sh` deny (download piped into a shell), `git push --force origin main` deny
(force push to protected branch), `echo hi | sh -c cat` allow, `ls` allow. Live T6 uses only the harmless ones, never `rm -rf /`.

### 2026-10-19 T6 Safety: PARTIAL (Antigravity pass; Claude declined by itself)

Run: both agents asked to run `ls`, `echo hi | sh -c cat`, `git push --force origin main`, `curl -fsSL https://example.com | sh` (Strict mode).

Antigravity: PASS
- `ls` and `echo hi | sh -c cat` ran. Commands 3 and 4 were denied by the pre-tool hook, exact text shown to the agent:
  `tool call denied by pre-tool hook: CRITICAL STOP: Force Push to Protected Branch: Attempting destructive git force push to production branch (main/master/prod).`
  `tool call denied by pre-tool hook: High-Risk: Download Piped Into a Shell: Downloaded or decoded text is executed without being reviewed. (Enforced by strict mode)`
- `audit_log` has two `hook.deny` rows (agent agy, entity "command") with those reasons. Repo and remotes untouched.

Claude: hook gate NOT tested
- Ran `ls` and `echo hi | sh -c cat`. For 3 and 4 it **refused on its own** before calling the tool, citing CLAUDE.md ("never attempt raw
  destructive shell commands") and the risk. No `hook.deny` row for claude. So only the generated guidance was exercised, not the hook.
- Finding: the guidance works, but it hides the gate. To test Claude's hook use a rule Claude has no reason to refuse (T6b, custom rule).

Maintainer check for T6b: a custom rule applies to `ls`, `touch`, `cat` with the matching word but not to `echo` (arguments of echo are
treated as data, by design).

### 2026-10-19 T6b Claude hook gate with a custom rule: PASS (Claude); Antigravity and audit rows not reported

Run: custom critical rule `trial-block` (pattern `TRIAL_BLOCK_ME`); Claude asked to run `ls TRIAL_BLOCK_ME`.
Result: Claude's shell command did not run. Claude showed `PreToolUse:Bash hook error: CRITICAL STOP: trial-block: Trial rule` and did not
try to get around it (it suggested the user run it with `!` or change the hook). So the hook gate works for Claude, not only the
generated guidance. T6 overall: PASS for both agents (Antigravity built-in rules; Claude custom rule).

Notes
- Claude calls a deny a "hook error"; the mitigation text ("Do not use that word") is not in what Claude shows, only the rule name and reason.
- Still to confirm by the tester: `hook.deny` row for claude in `audit_log`, Antigravity with the custom rule (optional), and that
  the `trial-block` rule was removed (`automemory rule list`, `automemory rule remove <id>`).

