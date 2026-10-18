# AutoMemory — Tasks

Open work, derived from [PLAN](PLAN.md). Update as items land. Tick = done and verified.
How to run the checks: [tests/README.md](../tests/README.md) (`tests/run_all.sh`).

## Needs a person (cannot be scripted)

- [ ] Live sessions: run `tests/manual/LIVE_CHECKLIST.md` sections A (Claude Code), B (Codex, trust hooks in `/hooks`), C (Antigravity), D (two agents, one repo), E (dashboard in a browser); log results in that file
- [ ] Phase 2 gate: write ≥ 20 real tasks in `tests/eval/tasks/`, run `off` / `on` / `verify` arms ≥ 3 times each, read `run_eval.py report` (method in `tests/eval/README.md`)
- [ ] Measure stale-flag precision (target ≥ 90%) and retrieval precision@3 on those tasks; plant stale and poisoned memories and confirm flagging, quarantine and revoke
- [ ] Planning Room A/B: solo + tests vs Room on a defined task set; drop the Room if it does not win
- [ ] Decide whether the PRD's "5 ms p95" means the pipeline (met, ≈1.3 ms) or the whole process (≈9 ms p95, ≈5 ms of it binary start-up); if the latter, reduce binary start-up
- [ ] Before deploying `workers/sync-gateway`: set `STRIPE_WEBHOOK_SECRET` / `LEMONSQUEEZY_WEBHOOK_SECRET`, `ADMIN_SECRET`, leave `ENABLE_DEV_AUTH_BYPASS` unset

## Code

- [ ] Park or delete the 17 unsupported connectors (~700 lines in `connect.rs`); `connect all` already configures only agy, claude, codex and says so
- [ ] Symbol-level hash for staleness: wire `CodeIntel::symbol_hash` into the verifier's `changed` status (it uses file hashes today)
- [ ] Contract fixtures per agent *version* once more than one version is in use (`tests/contract/fixtures/<agent>/<version>.json`)
- [ ] Wire `sync_cycle` into the CLI `sync import/export` paths (today only the daemon pass uses the conflict-safe op-log; the legacy snapshot import is last-writer-wins)
- [ ] Not yet audited: the crypto/BIP-39 code, and the worker's `dashboard.ts` / `sync.ts` routes

## Done (verified by `tests/run_all.sh`)

- [x] Worktree-aware project resolution, shared leases; committed
- [x] `connect`/`disconnect` for Claude Code and Codex (hooks + MCP), Codex via `config.toml`; refuse unparseable configs, atomic writes
- [x] Dashboard origin/host guard; human password on proposal decisions (dashboard, CLI, and MCP refuses agents); UI prompts once per tab
- [x] Lease acquisition atomic across processes; migrations atomic and loud; poisoned mutex recovers
- [x] Guardrail precision (printed text, here-documents and file content are not commands) and bypass coverage; warnings now reach the agent as hook context
- [x] CodeIntel via codebase-memory-mcp with file-hash fallback; `doctor` checks engine version and installed hooks
- [x] Automatic `Session checkpoint` memory on Stop; `automemory overlap` for pre-merge file overlap; revert/contradiction staleness covered by PostCommit and the consolidation worker
- [x] Dashboard Agents & Locks page (`/api/coordination`)
- [x] Delta sync wired into the daemon pass; fixed two bugs found by wiring it (edits were never pushed after a pull; old delta files looked like conflicts)
- [x] Hook opens the database without re-running the schema: end-to-end p95 21 ms → 9 ms
- [x] Sync gateway: webhook signatures verified, unsigned test payload only with the dev flag, constant-time admin secret
- [x] `tests/` directory: contract, guardrail, collision, lifecycle, password gate, dashboard, CodeIntel, latency, eval harness, live checklist

## Automatic locking v2 (design in PLAN §6)

- [x] Session-bound leases (heartbeat on any activity, SessionEnd releases, idle TTL 600 s); per-path ignore policy (`[locks] ignore`)
- [x] Shell write prediction; `git status` reconciliation with retroactive leases and PostToolUse warnings
- [x] Prompt hook lists files other agents hold; `locks --coverage` and `doctor` coverage line
- [x] Watcher alerts for changes under an idle holder's lease (dashboard)
- [x] Antigravity hook output follows its documented schema (found by a live run: PostToolUse `note` was rejected); notes delivered on PreInvocation
- [x] `mode set` / `skill` used the folder name as the project while hooks used the registered project (live run: Strict was set on `automemory`, hooks ran under `auto`, so nothing was denied); both now use `project_slug_for_dir`
- [x] `connect agy` registered hooks twice (workspace + plugin copy), so every agy event ran and was recorded twice; plugin copy removed
- [x] Antigravity: single hook registration (global or workspace), safe merge into an existing hooks.json, CLI + App + IDE skill paths, no stale plugin copies; hooks always answer in schema and fail open; duplicate registrations replay one reply; `disconnect all [--everywhere] [--dry-run]` removes every AutoMemory registration; `connect` no longer overwrites a user's git post-commit hook
- [ ] Verify in a live Antigravity session that the PreInvocation message reaches the model and Strict-mode deny blocks the call
- [ ] Re-run `automemory connect claude -g` (and codex) so the new SessionEnd hook is installed; leases still expire after 10 idle minutes without it
- [ ] After a week of real use: read `automemory locks --coverage`; if below 80%, look at which `reconciled`/`unattributed` writes dominate
- [ ] Optional wait-for-lock mode (hook waits a few seconds for a held file); hard/soft per-path policy beyond `ignore`
- [ ] Watcher only sees registered projects; register the project (`automemory init`/`link`) for hook-less agent alerts
- [ ] Decide: in Strict mode, should reconciliation deny the offending session's next call (today it warns)

## Better memory (plan in docs/research/memory_improvement.md)

- [ ] Idempotent distillation + quality gate (76% of active memories are `Execution outcome for:` boilerplate; 4,790 traces have 37 distinct titles; 7,283 skills have 11 names)
- [ ] `automemory memory prune`: merge duplicate traces/skills, archive boilerplate (reversible)
- [ ] Capture user corrections, failure signatures (error line + fix), commit messages as decision candidates
- [ ] Recall telemetry (0 of 77 memories ever recalled): count hits, then decay/reinforce
- [ ] Agent-in-the-loop distillation at PreCompact/Stop via `memory.propose` with evidence
- [ ] `valid_from` / `invalid_at` on memories; deterministic offline consolidation in `serve`
- [ ] Forgetting cases in `tests/eval`; fix "N edits across 0 files" in the session digest

## Cross-session and cross-machine continuity, retrieval modes (design in PLAN §7-8)

- [x] Stable `project_key` (git remote / root commit) on projects; deltas carry it and map to the local project on apply
- [x] Op-log as the only sync path; pull on SessionStart (2 s budget), push on SessionEnd; retire last-writer-wins import
- [x] `tests/scenarios/two_machines.py`: two HOMEs, different checkout paths, one shared vault
- [x] Resume pack (`automemory resume`, `memory.resume`, shown once at session start) and hand-off bundle (branch, sha, dirty files, next step)
- [x] Sync baton, checkpoint, review-proposal and doc kinds; `automemory sync bootstrap` for a new machine
- [x] `[retrieval] mode = keyword|hybrid`; `EmbeddingProvider` v2 (model id, batch, local); vectors per (memory, model); lazy indexing during hybrid search (a background reindex in `serve` is not built; use `embeddings reindex`)
- [x] Providers: `ngram`, `ollama`, `openai`-compatible, `exec` (any command); `automemory embeddings status|test|reindex|use|off|eval`; `tests/scenarios/embeddings_modes.py`
- [ ] `onnx` provider (optional cargo feature); sync of vectors between machines (each machine re-embeds today)

## Docs hygiene

- [ ] Keep tool and test counts in ARCHITECTURE/FEATURES in sync with code (60 MCP tools, 204 Rust tests at last check)

## Later (after v1 passes its gates)

- [ ] Promote additional agents one at a time (Cursor, Cline, …) with contract fixtures
- [ ] Cloudflare managed SaaS (`workers/`), team sharing
- [ ] Headless Runner (check vendor terms first), Autopilot profile
- [ ] Neural embeddings, WASM plugins
