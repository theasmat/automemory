# AutoMemory live trial log

> **2026-11-15 note.** Entries below about locks, leases, `automemory locks`, `lock.acquire` and "coverage" describe AutoMemory's own locking, which was **removed** (schema 23; file locks now come from the Tracewise companion, see `docs/specs/tracewise-companion.md`). They stay as history; the replacement checks are LIVE_CHECKLIST sections 0, D and F and S4 in LONG_SESSION_PLAN.

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
| T7 | Instruction files: `export-rules` block read by another agent | PASS | 2026-10-19 |
| T8 | Dashboard: pages show real data, tour and Settings work | PASS | 2026-10-19 |
| T9 | Human password gate | PASS | 2026-10-19 |
| T10 | Clean removal: `disconnect` leaves nothing behind | PENDING | |
| T11 | Planning room: two real agents agree a plan, the human approves it | PASS (3 fixes) | 2026-10-20 |
| T11b | Room started in the dashboard; invited agents find it with no id pasted | PASS | 2026-10-20 |
| T12 | Antigravity long chat continued in Claude Code (brief + history) | PENDING | |
| T13 | Project library with Claude Code, Antigravity and Codex | PENDING | |

## Test cases

### T11 Planning room (two real agents + you)
Goal: Claude Code and Antigravity discuss one design choice in a room, reach a plan, and your approval turns it into a decision and tasks.
Needs: `automemory connect claude` and `automemory connect agy` re-run in the trial folder (the rules text now explains rooms), `automemory serve` running, dashboard open at `/rooms`.
Prompts
1. Claude Code: "We need to decide how the shop stores the cart (session, cookie, or database). Open a planning room on that with room.join, find evidence with memory.search (save one note first if there is none), post your proposal, then tell me the room id."
2. Antigravity: "Join planning room <id> with room.join and wait for your turn (check room.pending). Post a critique with room.post; it needs evidence from memory.search."
3. Claude Code: "Check room.pending, then post your revision: the final choice and 2-4 steps as `- step` lines."
4. You: dashboard > Planning rooms. Read the discussion and "If you approve", then press Approve (password) — or Another round / Reject.
5. Either agent: "Claim the next task with task.next and say what it is."
Pass when
- Each agent posted only on its own turn; a post out of turn or without evidence was refused with a clear message (try one on purpose).
- The room shows Needs you after the revision; the discussion matches what the agents said.
- Approve records ONE decision (Project brain > Decisions, agent "room", the critique as dissent) and one task per step; a second approval is refused.
- `task.next` in an agent returns the first step; no agent called `decision.record` for it.
Also note: did the agents find and use the room tools without being told the tool names? Quote what they did if not.
Result: _in progress_

#### 2026-10-20 T11 setup (maintainer)
- Daemon on :4545 already runs the new build (started 13:20, after the 13:16 install); `GET /api/rooms` → `[]`.
- Re-ran `automemory connect claude` and `connect agy` in the trial folder. Antigravity's rules and skill got the rooms paragraph,
  but **CLAUDE.md did not**: `connect` only wrote the guidelines when CLAUDE.md was missing, so existing projects never received new
  guidance. Fixed: a CLAUDE.md that AutoMemory wrote (starts with its heading) has its guidelines section refreshed up to the managed
  block; a user's own CLAUDE.md is untouched (unit test). Re-ran `connect claude`: CLAUDE.md now has the Planning rooms line, the
  managed memory block is intact.

#### 2026-10-20 T11 step 1 (room opened, proposal posted)
Report: room `room-01a158788849725585986e3e4edfda2f` ("Shopping cart storage: session vs cookie vs database"), proposal "store carts in
SQLite keyed by session/user id" with one evidence note; the agent found and used the room tools on its own. The tester saw the post in
the dashboard but (1) authors showed as session UUIDs and (2) there was no way to give a decision in the room.
Findings and fixes (installed, dashboard restarted)
1. **Turn bug**: alone in the room, the proposer got the turn back after posting, so a second agent joining could never critique (it
   would be "not your turn") and the proposer could critique itself. Now the turn stays open until another agent joins, a joiner takes
   a turn held only by the last poster (this repairs the live room), and a critique of your own proposal is refused.
2. **Names**: the session that opened the room is Antigravity (its hooks recorded the session as `agy`), not Claude. Rooms now show
   "Antigravity (fabb96)" / "Claude Code (…)" / "You", from the hook records or an optional `agent` argument on room.join/room.post.
3. **The human in the room**: you can post a note at any time (agents see it in room.status as the human's guidance), and you can
   decide at any stage: the decision form is prefilled with the agents' plan when there is one, can be edited, and "Approve" /
   "Approve with my changes" / "Decide now" stores the decision and one task per step. Protected, logged.
Covered by unit tests and `tests/scenarios/planning_room.py`; 245 Rust tests green.

#### 2026-10-20 T11 step 2 (Claude Code critique): PASS
- Claude Code joined, read the room and posted a substantive critique: false either/or (the 4 KB cookie argument only applies to a
  cart stored in the cookie), no lifecycle or expiry, store SKU and qty only (prices from pricing.py, integer cents per the
  project preference), the stored note's `shop/cart.py` path is not in the repo, and SQLite write concurrency at the expected load.
  Evidence: the storage decision, the auth decision and the in-memory Cart note; all resolved. Turn passed to Antigravity (revise).
- Session id: Claude took it from its scratchpad path; it is its real Claude Code session (its hooks recorded `claude` for it), so the
  name resolved to "Claude Code (464450)". It also passed `agent: "claude-code"`.
- Tester saw ids in the UI: the API and a fresh page load show names ("Antigravity (fabb96)", "Claude Code (464450)"); the open
  tab still ran the pre-restart page. Reload fixes it.
- Side finding worth keeping: a stored note named a file that does not exist (`shop/cart.py`); the agent caught it. Brain > Tidy up
  flags such notes ("mentions a file that no longer exists").

#### 2026-10-20 T11 step 3 (Antigravity revision): PASS
- Antigravity read the room, answered every point of the critique (cookie holds only the cart id, `cart_items(cart_id, sku, qty,
  updated_at)` with 30-day cleanup, SKU+qty only with totals in integer cents from pricing/checkout, rows created on first add,
  single-process writes stated) and ended with a first-line decision plus four `- step` lines. Room: decided, "Needs you".
- It **corrected Claude Code**: `shop/cart.py` exists and is tracked (Claude read `git status`, which lists only changed files). The
  cross-check between agents worked as intended.
- Friction: to learn how steps become tasks, Antigravity searched the AutoMemory source (`rooms.rs`). Fixed: the `room.post` tool
  description now states the revision format (first line = decision, `- step` lines = tasks) and that critiques come from another agent.
- No human note was added in this run, so "agents follow your note" is still untested (covered by the scenario test only).

#### 2026-10-20 T11 step 5 run before the verdict (Antigravity `task.next`): correct refusal
- Asked to claim a task before the human approved: `task.next` → "No open tasks for this project"; Antigravity explained that tasks
  exist only after the human verdict and named the first one it would get. 0 rows in `tasks`, room still decided with no outcome.
- It checked whether it could run `automemory room approve` itself (guardrail dry-runs) and read the source; it did not approve.
- Earlier in the same session it tried to read the AutoMemory database with `sqlite3`: the Zero-SQL guardrail denied it
  (`hook.deny`, "Raw SQLite Access to Internal AutoMemory Database", 09:03 UTC) and queued a review proposal in the Inbox.
- Agents reading AutoMemory's own source is a trial artefact (the source is on this machine); normal users will not see it.

#### 2026-10-20 T11 steps 4–5 (human verdict, task claimed): PASS, one bug found and fixed
- The tester approved in the dashboard; the decision was recorded and the steps became tasks. Antigravity's `task.next` then claimed
  task 1 ("Define cart_items table … with 30-day abandoned cart cleanup"), with the room's topic and decision as its spec, and stopped
  as asked.
- **Bug**: the Stop hook blocked every Stop while the session held a claimed task ("Pending work: claimed task …"), so Antigravity,
  told to wait, was re-prompted again and again and kept answering "I acknowledge the Stop hook reminder" (a token-burning loop).
  Fixed: the reminder is given once per piece of owed work; later Stops are let through and the claim stays. Covered in
  `planning_room.py`; installed.

#### 2026-10-20 T11b attempt 1: room not created; the plan's tasks were carried out instead
- No room was started in the dashboard (no `room_meta`, no "start a planning room" audit row), so there was nothing for the hook to
  announce. The New room dialog works on the trial daemon (two agents listed, Create enabled; checked in a browser).
- Asked "is anything waiting for you? If so, handle it", the **old** Antigravity session (fabb96, still holding task 1) found the four
  tasks from the cart room through `task.next` and **implemented all of them**: `cart_items` table with 30-day cleanup, cart id from
  the guest or JWT cookie, SKU+qty only with integer-cent totals, WAL connection; `tests/test_cart.py`; 18 tests pass; each task
  `task.submit`ted, then `baton.pass`. So room → decision → tasks → implementation → submit works end to end.
- Note for users: a neutral "handle what is waiting" prompt is enough for an agent to start coding claimed tasks. The trial folder
  now has these uncommitted changes (shop/*.py, tests/test_cart.py).
- Review of that code (maintainer, probes run against it): tests pass but four real defects. (1) The cart id is taken from an
  **unverified** JWT `sub` (no signature check; the shop has no JWT code or secret) and guest cookies share the same id space, so
  `auth=x.<{"sub":"user-42"}>.y` or `cart_id=user-42` opens user 42's cart; a test asserts this behaviour. (2) Two `Cart` objects for
  one cart lose updates (adds of 1 and 1 store 1): qty is computed in memory and written as an absolute value. (3) The 30-day cleanup
  deletes single items, not abandoned carts: a cart used 2 days ago lost its 31-day-old item. (4) Negative quantities are accepted.
  The room's plan said "read cart_id from the existing JWT cookie" while no verified auth exists; neither agent raised that in the room.
- Fixed in the trial project (uncommitted there, 22 tests pass, one new test per finding): (1) a user cart id comes only from an
  HS256 JWT whose signature checks out and has not expired (secret `SHOP_AUTH_SECRET`; none set means no token is trusted); ids are
  namespaced `user:<sub>` / `guest:<uuid>` and a guest cookie must be a UUID, so a client cannot name another cart; (2) quantity is
  incremented inside SQLite and the object takes the stored total; (3) cleanup deletes carts whose newest line is older than N days,
  not single old lines; (4) `add` rejects quantities that are not whole numbers >= 1. The first test run of the original probe
  (forged cookie opens user-42's cart, adds of 1+1 store 1, active cart loses a line, qty -5 accepted) now shows all four closed.

### T11b Room started in the dashboard, agents find it without an id
Goal: you start a room; the two invited agents learn about it from their own hooks, and take their turns with nothing pasted.
Pre-checked (maintainer): the trial folder has Claude's `UserPromptSubmit` hook and Antigravity's `PreInvocation` hook registered with the
installed binary; the daemon on :4545 runs the build with R1; `/api/rooms` answers.
Steps
1. Fresh Claude Code and Antigravity sessions in the trial folder.
2. Dashboard > Planning rooms > New room: topic, question, tick both agents, optional evidence/scope/constraints, Create (password).
3. To Claude Code, without naming the room: "Is anything waiting for you in this project? If so, handle it and tell me what you did."
4. Same to Antigravity. 5. Same to Claude Code again (it should revise).
6. Run `python3 tests/manual/t11b_check.py` (in the AutoMemory repo) and paste its output with the agents' replies.
Pass when the checker prints PASS: T11b, and neither agent asked you for a room id.

#### 2026-10-20 T11b run, steps 2-3
- Room "cart" started in the dashboard (audit `ui.action: start a planning room`, both agents invited, no question given).
- **Claude Code**: told by its prompt hook (`room_told` key for its session), joined with `agent: claude-code`, and after a first message
  asking what "cart" meant, posted a proposal (API-first cart flow, evidence cited). The empty question made the proposal drift: it
  proposed an HTTP API for a shop that has none. Lesson: give rooms a question (the dialog marks it optional; consider making it required).
- **Antigravity** (session f0586236): given "anything waiting? handle it", it found the room through `room.pending` and joined, but first
  did a lot else: resolved a failure record, picked up a baton and **committed all 16 files (including `.agents/`, `.mcp.json`,
  CLAUDE.md/AGENTS.md/GEMINI.md) to `main`** (3a7c687). It was not asked to commit. Lesson: the neutral prompt invites general
  housekeeping; for the trial say "only planning rooms".
- Why Antigravity was not told by its hook: when its session started (14:51) Claude had already joined, so the turn was Claude's;
  Antigravity's turn only came after Claude's proposal (15:03), after its session had stopped. Correct per the protocol, not a bug
  (verified on a copy of the database: with the turn open, the agy PreInvocation hook injects the room line). Its hook line will come on
  its next step.

#### 2026-10-20 T11b repeat with a new room ("Order cancellation"): in progress, two findings
- A new room with a real question, scope and constraints; Claude proposed (webhook-driven refund, idempotent stock restore); you added
  a note ("prefer store credit"). Antigravity then posted a **critique whose whole body was "test"** (with evidence attached). A room has
  one round, so that spent Antigravity's only turn on a placeholder. Fix: a post must be at least 25 characters (propose, critique,
  revise); a short post is refused with a clear message and does not spend the turn (unit test + scenario).
- The checker crashed with an IndexError while the room was still running. Fixed: it now says which turn the room waits for and exits
  2 until the room is decided.
- For this live room: add a note naming the risks (double refund, replayed webhooks, stock) and let Claude revise, or press Another
  round after it is decided so Antigravity can critique properly.

#### 2026-10-20 T11b run with "Order cancellation v2": 7 of 8 checks pass; the failure was a stale MCP server, now detected
- Antigravity proposed, Claude Code critiqued (a real critique), your note, Antigravity revised with steps; room decided. Checker: only
  "hooks told two different agent sessions" failed: Claude was not told by its hook.
- Cause: after Antigravity proposed, the room kept the turn on the proposer (the OLD rotation), so Claude's hook had nothing to say
  (Claude found the room anyway through `room.pending` and `room.join`, which hands a stuck turn to the joiner). Antigravity's MCP
  server (pid 87461, started Oct 19 02:34, under its language server) was still running a day-old binary, so it kept applying the old
  turn rule and the old "no length minimum" (explains the "test" critique too). Hosts keep MCP servers alive across upgrades.
- Fix: `automemory doctor` now lists MCP servers that started before the installed binary was replaced and who launched them
  (`Checking running MCP servers... ⚠ pid 87461 (running 44 h), started by language_server`); scenario `stale_mcp.py`.
  Action for the tester: restart Antigravity (and any Claude Code session older than the last install) after every install.

#### 2026-10-20 T11b steps 4-5: PASS (`t11b_check.py` prints PASS: T11b)
- Both agents were told by their hooks at their own turns (`room_told` keys: Claude at open and at revise, Antigravity at critique) and
  took them with a narrow prompt ("only planning rooms"). Order proposal -> your note -> critique -> revision; room decided.
- **Your note steered the room**: after "plain Python, no web framework, decide storage not an HTTP API", Claude dropped the
  `/api/carts` design and revised to cookie-keyed `cart_items` with checkout atomicity steps. First real test that agents follow a note.
- Antigravity's critique checked the repo (cart.py exists on main) and raised enumeration risk of path ids and the missing order
  creation at checkout. Claude's revision did not verify `shop/cart.py` itself (it said so).
- Open: the verdict on this room is yours (Approve / Another round / Reject). The checker is read-only; it was verified on a
simulated run in a sandbox database.

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

### 2026-10-19 T7 Instruction files: FAIL on decisions (notes worked); fixed, rerun needed

Run: `automemory export-rules`; Antigravity asked to read AGENTS.md (no MCP); new decision `auth` recorded; `--check` before and after.

Result
- AGENTS.md / CLAUDE.md / GEMINI.md: block present once, original AutoMemory guidance untouched; but the block only had
  "Project knowledge: [preference] Prices are stored as integer cents, never floats". **No "Decisions in force" section**: neither the
  Stripe decision (recorded by Claude over MCP) nor the `auth` decision (recorded with the CLI) was in it.
- Antigravity read AGENTS.md without any MCP call and reported the cents note (mislabelled "decision" because the file had no
  decision section), plus the guidance rules. So reading the file works; the content was incomplete.
- `export-rules --check` said "current" after the new `auth` decision, for the same reason (the decision was invisible to the view).
- The refresh-on-new-session step could not be judged (grep for "httpOnly cookie" = 0 in all three files).

Cause: decisions recorded without a project (MCP `decision.record` with no project_id, CLI `decision record` without `--project`) were saved
with no project, and the rules view only listed decisions of the current project.

Fixed (installed)
- MCP: writes with no `project_id` (`decision.record`, `memory.save`, `memory.propose`, `trace.save`, `failure.save`, `task.create`, `baton.pass`) go to the
  project of the folder the MCP server runs in.
- CLI `decision record` uses the current folder's project and refreshes the instruction files at once.
- The rules view also lists project-less decisions (so the two existing ones appear after a re-export).
- Scenario `rules_export.py` now records a decision over MCP without a project and expects it in AGENTS.md.
- Verified in a scratch repo: both a CLI and an MCP decision appear after `export-rules`; `--check` reports a stale file after the MCP one.

Open: Claude called `decision_record` twice in T3; `decision list` not yet pasted. Decisions still show agent "agent".

### 2026-10-19 T7 Instruction files: PASS (rerun after fix)

Run: `export-rules`; Antigravity told to read AGENTS.md without AutoMemory tools; a new Claude session recorded decision
`storage` = "Orders are kept in SQLite" over MCP; a further Claude session start; `export-rules --check`; grep in the three files.

Result
- The block now lists the decisions in force (auth: JWT in an httpOnly cookie, payments: Use Stripe webhooks, not polling) and the cents note.
- Antigravity read AGENTS.md (one "Viewed AGENTS.md" step, no MCP call) and listed the decisions with their rationales correctly. (It
  put the cents preference under its own "decisions" heading; a reading choice, not a file error.)
- After the next Claude session start: `--check` = "Instruction files are current" and "Orders are kept in SQLite" appears once in each of
  AGENTS.md, CLAUDE.md and GEMINI.md: a decision recorded over MCP reached all three files through the session-start refresh.
- Not reported: the intermediate `--check` ("out of date" expected after the MCP decision), `decision list` (duplicates?).

### 2026-10-19 T8 Dashboard: PASS (checked by the maintainer in a browser against the real trial data, with a second daemon on :4546)

Pages opened: Home, Project brain, Agents & locks, Safety, Session history; no console errors.
- Home: Agents 1, Project brain 11 notes / 4 decisions, Needs you 0; setup shows 4 of 8 done (Connect 2/2, Teach 2/2, Stay in control 0/4).
- Project brain: tabs Curated 4 / Decisions 4 / Conventions 1 / Gotchas 2 / Auto-captured 7 / All 11; decision cards show the source ("human" for the CLI
  decision, "agent" for MCP ones).
- Agents & locks: no active locks (correct after sessions ended), hand-off notes listed. Session history: the Antigravity and Claude sessions with their events.
- `decision list` / database: 3 decisions (payments, auth, storage), each once: **no duplicate** from Claude's double `decision_record` call.

Found and fixed (installed)
1. **Safety showed "Relaxed" although the project is Strict**: with "All projects" selected the page asked for a mode with no project.
   With exactly one real project the dashboard now shows and sets that project's mode (verified: Strict selected).
2. **Setup checklist judged the daemon's folder, not the project**: "Instruction files" stayed unticked because `serve` was started elsewhere.
   `/api/setup` now uses the daemon's folder when it is a project, otherwise the newest real project. "Protected by lock" now also ticks
   from a recorded lock block in the audit log (T4's block predates audit logging, so it stays unticked until the next one).
3. The first-run welcome card was still shown with an agent connected and activity recorded; it is now hidden once an agent is connected.
4. All three decisions in the database have **no project** (`project_id` empty), including `storage`, recorded over MCP after the earlier fix.
   Some clients start the MCP server in "/" instead of the workspace (running `automemory mcp` processes show cwd "/" for several), so the
   folder lookup missed. Now, with a single real project, a write with no project goes to it (verified from cwd "/").

Not fixed / observations
- Decision cards repeat the same text three times ("Decision (storage): ...", "Architectural Decision: storage", "Decision: ...").
- Home says "1 working now" for an agent whose last event was within 10 minutes, even when its session has ended.
- Existing decisions keep no project; the rules view lists project-less decisions, so they still reach the instruction files.

### 2026-10-19 T9 Human password gate: PARTIAL (agent side pass; dashboard side not reported)

Run: `automemory rule passwd` set; an agent was asked to run `automemory mode set vibe`.

Agent side: PASS
- The agent's command stopped at the prompt `AutoMemory human password:` and the agent asked the human which password to type (it did not
  have it and could not continue). `automemory mode get` afterwards: **STRICT**, unchanged.
- Design note: the prompt appears because the agent's shell has a terminal; the gate holds because only the human knows the password.
  If the human tells the agent the password, that is consent. No code change.
- Pending: the agent's waiting command should be cancelled.

Audit evidence from the same output (confirms T6b for both agents): `hook.deny` rows for **agy** and **claude** with
`CRITICAL STOP: trial-block: Trial rule`, plus agy's force-push and download-pipe denials, each with the session id.

Not reported yet: dashboard part (masked prompt on changing the mode, wrong password rejected with a clear message, right one accepted,
asked only once per tab) and the clarity of the error text.

### 2026-10-19 T9 follow-up: tester reported "mode changed and a rule deleted without a password prompt"

Checks by the maintainer (password was set, `password_hash` present in config.toml)
- Server: `POST /api/mode` without the header returns 403 "needs the human password" on a fresh daemon; rule add/delete/toggle use the same gate.
- Browser, fresh tab: clicking Balanced opens the "Human Approval Required" dialog with a masked field. So the gate and the dialog work.
- Likely cause of the report: the dashboard **remembered the password for the whole tab** (sessionStorage) after one correct entry, so later
  changes (mode, rule delete) went through silently. By design, but too lax for a safety gate.

Found and fixed (installed)
1. The password is now kept **in memory only, for 2 minutes** (not in sessionStorage); every change after that asks again, and a reload forgets it.
2. **Bug from the T8 change**: the dashboard sent the project *id* to `POST /api/mode`, which expects a slug, so it created a stray project named
   after the id and set the mode there; with "All projects" it created a project called `default`. The server now accepts an id or a slug, uses the
   only real project when none is given (400 if ambiguous) and no longer invents `default`. Verified in a scratch daemon.
3. The two stray projects in the trial database (slug = the project id, and `default`, both with no memories, decisions or events) were removed.
   The real project's mode is currently **review** (it was changed from the dashboard); set it back with `automemory mode set strict`.

### 2026-10-19 Password gate hardened (requested): every protected action asks, nothing is stored

- Dashboard: the password is **never remembered** (no sessionStorage, no timer): every protected action (mode, rule add/delete/toggle, approve/reject,
  sample data, add memory) opens the dialog again. It is used for that one request and the field is emptied on submit and on close; the
  browser is told not to offer to save it. A wrong password stays in the dialog (three tries) instead of failing silently.
- Server: five wrong passwords in a minute lock the gate (HTTP 429, "Wait a minute") even for the right password, so a script cannot
  guess against the local API. Covered by `tests/scenarios/dashboard.py`.
- Unchanged: agents cannot pass the gate (CLI asks on the terminal, MCP decide is refused, REST needs the header).
- Retest T9 dashboard half: every click on Balanced / Strict / rule delete must ask; two actions in a row ask twice.

### 2026-10-19 T9 Human password gate: PASS (dashboard half confirmed by the tester after the hardening: "yes it fixed")


### T12 Antigravity long chat -> Claude Code continues: NOT RUN YET (needs you and live agents)

Prepared in C8. Steps and the checker are in `tests/manual/t12_check.py` (read-only). Pass when: the Antigravity chat imports, the brief has a
goal, a next step and what was ruled out, you confirm the drafts, and Claude Code (new session, same folder) answers "where did we get to / what
was ruled out and why / next step" from the brief without you re-explaining. Record the result and any wrong or missing item below.
Offline evidence so far: `tests/eval/continuity_eval.py` (golden chats; recall 18/18 on phrasings the drafter was tuned on, so expect lower on real chats).

### T13 Project library with Claude Code, Antigravity and Codex: NOT RUN YET (about 45 minutes)

Goal: documents kept outside git are found, read, written and (carefully) changed by three real agents; git stays clean; they sync like the rest.
Facts the agents cannot guess: the PRD says carts expire after **37 days** and **Redis was rejected**; the git-ignored file holds the marker **pelican-2290**.

Setup (you, in the trial folder; the installed binary is current, restart anything that was running)
1. `automemory serve` (restart it so the Library page exists), then `automemory doctor` (all green; the line "Checking running MCP servers" must list nothing: restart agents it names).
2. `automemory connect codex` (project only, no `-g`), and make sure Claude Code and Antigravity are connected as in T1/T2.
3. Create the PRD file and add it (asks the password):
   ```bash
   cat > /tmp/prd-cart.md <<'EOF'
   # Cart PRD

   A shopper keeps items between visits. Carts expire after 37 days of no activity.

   ## Decisions
   - Decision: store carts in SQLite
   - Rejected: Redis, because of hosting cost

   - TODO: write the nightly cleanup job
   EOF
   automemory library add /tmp/prd-cart.md --as plans/prd-cart.md
   ```
4. A file git must not see, kept in place:
   ```bash
   printf '# Private PRD\n\nCodename marker: pelican-2290. Not for the repository.\n' > PRD-private.md
   echo PRD-private.md >> .gitignore
   automemory library link PRD-private.md      # password
   automemory library sync
   git status --short                          # must show only .gitignore (and your earlier trial changes), nothing under .automemory
   ```
5. Dashboard > Library: the tree shows `plans/prd-cart.md` and `PRD-private.md` (badge "in place"); open each at Summary, Outline, Whole file.

Per agent (fresh session in the trial folder; do Claude Code, then Antigravity, then Codex; save each reply as `claude.txt`, `agy.txt`, `codex.txt` in one folder)
6. Read question (do not mention tools or paths): "What does our cart PRD say about how long carts last, what did we reject and why, and what is in PRD-private.md? Tell me where you found each."
   Expect: 37 days, Redis rejected for hosting cost, pelican-2290. Note whether it used `library.get`/`memory.search` (scope library), `.automemory/library/INDEX.md`, or the file directly (all acceptable; record which).
7. Write question: "Save a short note of what you just learned as a library note for the other agents." Expect a `library.put` (a file under `notes/` or similar) marked agent-written in the Library page.
8. Suggest question (once, with any one agent, or each with a different change): "Support wants carts to last 60 days. Suggest that change to the PRD." Expect `library.propose`: an Inbox item "<agent> suggests changing library file plans/prd-cart.md (+1 -1 lines)". **Record if the agent instead edits `.automemory/library/plans/prd-cart.md` directly** (the file is read-only, but a determined agent can bypass it; the checker flags it).
9. You: Inbox > open the suggestion > Approve (password). Library > History shows a revision "approved from <agent>"; Restore the previous revision to prove it is reversible.
10. Cross-agent: in a fresh Antigravity (or Codex) session ask "Which library notes did other agents leave, and what do they say?" Expect it to find the note from step 7.
11. Sync (needs a remote; skip if none): on a second computer run `automemory sync auto` then `automemory library sync`; the PRD, the agent note and `PRD-private.md` (in place) should appear.

Check (in the AutoMemory repo)
```bash
python3 tests/manual/t13_check.py --project /Users/as/Developer/test/automemory --answers /path/to/replies
```
Pass when it prints `PASS: T13`. Warnings are findings to record (an agent bypassing `library.propose`, or reading without a tool call), not failures.
Paste the output and each agent's reply, plus anything that confused the agent (for example it ignored the INDEX line, or tried to edit a read-only copy).

### 2026-11-15 Long Codex session, 60 follow-up prompts (Ledgerly): run on the build BEFORE the companion work
Build under test: the memory-quality build (project-scoped reads, author labels, lock grace, nightly tidy, agent naming), installed before the Tracewise companion existed. AutoMemory still did its own locking then; Tracewise was not involved.
Status: results not yet recorded here. Paste the session's findings (the 60-prompt checks in LONG_SESSION_PROMPTS.md, `automemory resume`, `decision list`, `memory list`, `proposal list`, `stats`) and they become this entry.

What this run still proves after the update (unchanged code paths): memory recall and notes quality, the brief and hand-offs, decisions and their authors, the Inbox, the pack being handed over again around prompts 25 and 50, project scoping, `resume` size.
What it does **not** prove any more: anything about locks or collisions (AutoMemory's own were removed), `locks --coverage` figures (now Tracewise's capture coverage, a different number), the 10-minute lease expiry, "lock grace after a turn" (Tracewise releases at Stop).
Compare after the update (same project, fresh Codex session, same first 10 prompts): `automemory resume | wc -c` stays in the same range; the pack still names the goal, next step and ruled-out items; the notes and decisions counts did not change across the schema 23 migration; hook latency per call (doctor) did not rise by more than a few ms.

