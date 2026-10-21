# Testing AutoMemory

Everything that can be automated runs with one command from the repo root:

```bash
tests/run_all.sh            # builds a release binary, then runs every check below
tests/run_all.sh --quick    # skips the build, the latency run and the CodeIntel smoke test
```

Requirements: Rust 1.80+, `python3` (3.9+, standard library only). Optional: `codebase-memory-mcp`
(CodeIntel smoke test; skipped when missing) and `pnpm install` in `workers/sync-gateway` (webhook tests).
Every script test runs in a throwaway `HOME` and git repository, so your real `~/.automemory`,
`~/.claude` and `~/.codex` are never touched. Set `AUTOMEMORY_BIN=/path/to/automemory` to test a
specific binary; otherwise the release build, then the debug build, is used.

## What is tested, and how

| Layer | Command | What it proves |
|---|---|---|
| Shared instruction files | `python3 tests/scenarios/rules_export.py` | export-rules into AGENTS/CLAUDE/GEMINI/Cursor files, user text kept, idempotent, refresh on session start, unverified memories excluded, disconnect restores |
| Embeddings modes | `python3 tests/scenarios/embeddings_modes.py` | keyword default, provider switch, remote refusal, reindex, hybrid recall, fallback, hooks never wait |
| Rust unit + integration | `cargo test --workspace` | Domain logic: guardrails, leases (incl. a multi-connection race), migrations, retrieval, sync deltas, overlap check, CodeIntel with a fake engine, dashboard origin guard, hook output formats |
| Agent hook contract | `python3 tests/contract/run.py [agent]` | Recorded Claude Code, Codex and Antigravity payloads produce the right hook output (deny / warn / silent) |
| Guardrail table | `python3 tests/guardrails/run.py` | What is blocked, what only warns, what must pass (false positives and bypasses live here) |
| Two-agent collision | `python3 tests/scenarios/collision.py` | A second agent editing a leased file is told (Vibe) or denied (Strict) with no `lock.acquire`; unrelated files stay silent; parallel lease requests |
| Automatic locking v2 | `python3 tests/scenarios/locking_v2.py` | Leases survive Stop and end at SessionEnd; shell writes are leased before they run; unpredictable script writes are caught afterwards and collisions reported; prompt lists others' files; ignored paths; coverage report |
| Session lifecycle | `python3 tests/scenarios/session_lifecycle.py` | Stop leaves a checkpoint memory for sessions that edited files |
| Registration + hook robustness | `python3 tests/scenarios/registration.py` | `connect` for Antigravity/Claude/Codex then `disconnect all --everywhere` leaves nothing and keeps the user's files; a hook registered twice answers once; hooks never fail (corrupt database, empty or garbage input) |
| Human password gate | `python3 tests/scenarios/human_gate.py` | With a password set, agents cannot decide proposals or change rules (MCP and CLI) |
| Optional LLM helper | `python3 tests/scenarios/llm_helper.py` | Off by default; cloud refused without consent; suggestions land as pending proposals only; reruns skip judged pairs; injection replies create nothing |
| Agent matrix | `python3 tests/scenarios/agents_matrix.py` | 13 agents connect, get their instruction file and disconnect cleanly |
| Self-update | `python3 tests/scenarios/update.py` | `update` asks first, verifies the checksum, refuses tampered or unchecked downloads, replaces atomically |
| Dashboard pages in a browser | `tests/ui/.venv/bin/python tests/ui/smoke.py` | Every page, light and dark, loads with sample data and no console error, exception or 5xx; a Protected action opens the password prompt. Skipped (with a message) without Playwright: `python3 -m venv tests/ui/.venv && tests/ui/.venv/bin/pip install playwright && tests/ui/.venv/bin/playwright install chromium` |
| Hook recording | `python3 tests/scenarios/hook_record.py` | Off by default; payloads saved with secrets removed; converter makes a fixture |
| Backup and restore | `python3 tests/scenarios/backup.py` | create, list, restore with `--yes`, old DB kept, damaged file refused, doctor warns about unreadable copies |
| Hook fuzz | `python3 tests/scenarios/hook_fuzz.py` | Garbage JSON into the real hook: always exit 0 and one JSON object; a command nested 60000 deep is denied (`HOOK_FUZZ_N=2000` harder) |
| Native embedding adapters | `python3 tests/scenarios/embeddings_native.py` | Cohere, Voyage and Gemini profiles reindex through a mock server speaking each API's shape |
| Planning room edge cases | `python3 tests/scenarios/room_edges.py` | Input limits, how Claude and Antigravity are told about a room (and only once), project scoping, MCP rules for agents, a posting race, restart persistence, hook cost with 60+ rooms |
| Room CLI | `python3 tests/scenarios/room_cli.py` | list/show/approve/reject/again, each verdict behind the password, results match the preview |
| Guideline refresh | `python3 tests/scenarios/guidelines_refresh.py` | `connect` updates an AutoMemory-written CLAUDE.md, keeps the memory block, never touches the user's own file |
| Stale MCP servers | `python3 tests/scenarios/stale_mcp.py` | `doctor` names MCP servers started before the binary was replaced (about 10 s; Unix) |
| Continuity brief | `python3 tests/scenarios/continuity.py` | An agent leaves a brief by MCP; another agent's hook hands it over with provenance; no duplicates, secrets removed, a person's words protected, CLI behind the password, token budget, broken config fails closed |
| Continuity chat history | `python3 tests/scenarios/continuity_history.py` | Off until enabled; Claude Code, Antigravity, Codex, Command Code and Cursor chats imported incrementally and read-only, secrets and home folder removed, thinking/tool output never kept; CLI and MCP search; password gate; forget |
| Continuity dashboard | `python3 tests/scenarios/continuity_dashboard.py` | Import drafts a goal and ruled-out options (unconfirmed), confirming works, drafting twice adds nothing. Dashboard API: turning an agent on, editing the brief and deleting history need the password; import, search and Hand off now do not; nothing is read until an agent is on |
| Continuity triggers + history sync | `python3 tests/scenarios/continuity_triggers.py` | Stop imports and drafts only for an agent that is on; PreCompact accepted; sync-history shows sizes, push needs the password, remote holds ciphertext, pull on a second computer is searchable and idempotent; ordinary sync never carries chat |
| Project library | `python3 tests/scenarios/library.py` | Add needs the password and never touches the repository; tiers, search, MCP list/get/search, secrets refused with the line, path escapes refused, session-start index (not content), --move prints the git command and never commits; L2: `.automemory/library` copy + INDEX.md invisible to git status and `.gitignore` untouched, hook restores the copy, `library link` (password) keeps a git-ignored file in place, edits picked up, doctor check |
| Project library sync | `python3 tests/scenarios/library_sync.py` | Two computers through a git vault: shared and kept-in-place files arrive (copy + INDEX.md, file in place), personal file stays, remote is ciphertext only, removal travels and does not come back |
| Library dashboard | `python3 tests/scenarios/library_dashboard.py` | Agent suggestion approved/rejected from the Inbox API, never in bulk. Dashboard API: add/edit/remove/restore/link need the password, secrets and bad paths refused, revisions readable, sync is Safe and keeps the folder out of git status, kept-in-place files are edited on disk |
| Continuity eval (`tests/eval/continuity_eval.py`) | `python3 tests/eval/continuity_eval.py` | 4 golden chats -> imported -> drafted; recall of expected goal/constraint/rejected/decision/next/open answers must be at least 0.8 (now 1.00, but the chats were written alongside the heuristics, so real chats will score lower); secrets and thinking never reach the brief; result in tests/eval/results/continuity.json |
| Planning room | `python3 tests/scenarios/planning_room.py` | Two simulated agents + human: turn and evidence rules, no self-approval, approve stores one decision + a task per step, another round, task claimed by a third agent |
| Dashboard API | `python3 tests/scenarios/dashboard.py` | UI is served, live leases appear, foreign Origin / rebinding Host are rejected, password required for decisions |
| Onboarding checklist | `python3 tests/scenarios/onboarding.py` | `automemory next` step progression, doctor checklist, /api/setup and /api/onboarding REST APIs |
| Guided tour and demo | `python3 tests/scenarios/tour.py` | `automemory tour --auto` 8 sandbox steps, collision prevention, learn topics, demo seed and remove |
| CodeIntel (real engine) | `python3 tests/codeintel/smoke.py` | `memory.about` returns real callers from codebase-memory-mcp; `doctor` reports its version |
| Hook latency | `python3 tests/perf/hook_p95.py [runs] [budget_ms]` | End-to-end wall time per hook call, process start included |
| Size and recall gates | `python3 tests/perf/gates.py [notes]` | Binary within 16 MB, recall p95 within 150 ms on 300 notes |
| Sync gateway | `cd workers/sync-gateway && npx vitest run` | Webhook signature checks (Stripe, LemonSqueezy) |
| Eval harness | `python3 tests/eval/run_eval.py selftest` | Scoring and report maths (the real A/B runs are manual, see below) |

Single Rust tests: `cargo test -p automemory-core <name>`. Benchmarks (in-process pipeline means):
`cargo bench -p automemory-core --bench perf`. Harder fuzzing: `FUZZ_ITERS=100000 FUZZ_SEED=7 cargo test -p automemory-core --test fuzz_like --release`.

### Reading the latency number

`hook_p95.py` measures a whole `automemory hook` process as the agent sees it. On the development
machine: p95 ≈ 9 ms, of which ≈ 5 ms is simply starting the binary (`automemory --version` alone
takes that long). The PRD's "5 ms p95" budget therefore applies to the pipeline, which `cargo bench`
puts at ≈ 1.3 ms mean; the end-to-end figure is the honest upper bound. Re-run after changes to
database opening or hook startup.

## Manual checks

| What | Where | When |
|---|---|---|
| Real Claude Code, Codex and Antigravity sessions, two agents on one repo, dashboard in a browser | [`manual/LIVE_CHECKLIST.md`](manual/LIVE_CHECKLIST.md) | After changes to `connect`, `hook`, guardrails or the dashboard; before a release |
| Does memory help? (Phase 2 gate) | [`eval/README.md`](eval/README.md) | Once at least 20 real tasks are written |

## Adding a test

* **A bypass or a false block in the guardrails:** add a row to `guardrails/run.py` (`BLOCK`, `WARN` or `PASS`) and a Rust case in `crates/automemory-core/src/guardrails/evaluator.rs`. Assemble risky strings at runtime in Rust (`["r","m"].concat()`) so the repo's own guardrail does not refuse to write the test file.
* **An agent changed its hook format:** record the new payload as a case in `contract/fixtures/<agent>/cases.json` (or a new `<version>.json` beside it), then fix the adapter until the whole file passes.
* **A new scenario:** copy `scenarios/collision.py`; use `Sandbox` from `lib/harness.py` for an isolated HOME, `sb.hook(...)` to call a hook, and `sb.mcp(tool, **args)` to call an MCP tool.
* **Schema change:** bump `LATEST_SCHEMA_VERSION` in `db/connection.rs` with a migration; `a_fresh_database_reports_the_latest_schema_version` fails if you forget.

## Known limits

* The collision script's 8-process race is a smoke test; process start-up is slower than the race window. The authoritative race test is the Rust unit test `concurrent_connections_never_double_grant_one_scope` (it failed 3 of 3 times before the fix).
* Nothing here starts a real coding agent; that is what the live checklist and the eval harness are for.
* The dashboard password prompt is JavaScript in the browser and is covered only by the manual checklist.
