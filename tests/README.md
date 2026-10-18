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
| Dashboard API | `python3 tests/scenarios/dashboard.py` | UI is served, live leases appear, foreign Origin / rebinding Host are rejected, password required for decisions |
| CodeIntel (real engine) | `python3 tests/codeintel/smoke.py` | `memory.about` returns real callers from codebase-memory-mcp; `doctor` reports its version |
| Hook latency | `python3 tests/perf/hook_p95.py [runs] [budget_ms]` | End-to-end wall time per hook call, process start included |
| Sync gateway | `cd workers/sync-gateway && npx vitest run` | Webhook signature checks (Stripe, LemonSqueezy) |
| Eval harness | `python3 tests/eval/run_eval.py selftest` | Scoring and report maths (the real A/B runs are manual, see below) |

Single Rust tests: `cargo test -p automemory-core <name>`. Benchmarks (in-process pipeline means):
`cargo bench -p automemory-core --bench perf`.

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
