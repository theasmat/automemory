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

## Shared knowledge across agents and IDEs (plan: PLAN §9)

- [x] `build_rules_view`, `export-rules`, session-start refresh, `disconnect` removes the block; `tests/scenarios/rules_export.py`
- [x] `automemory note`, dashboard **+ Add memory** (`POST /api/memories`)
- [ ] Hook adapters per agent (Gemini CLI, Cursor, Copilot, Cline, Kiro, OpenCode, Windsurf): read each vendor's hook docs first; replace the `matches!` helpers in `hook.rs` with a per-agent schema
- [ ] `rules_target()`/`disconnect()` on the adapter trait (connect and disconnect are still two parallel per-agent lists)
- [ ] Dashboard "Project brain" page (plain-language Decisions / Conventions / In flight; which agent saw what)
- [ ] Skills install for agents with a skills folder; refresh from MCP `decision.record` (today: next session start or `export-rules`)

## Dashboard UI/UX redesign (spec: [plan/ui-ux.md](../plan/ui-ux.md))

- [x] First-run tour (CLI + dashboard): brief for an implementing agent in [plan/onboarding-tour-brief.md](../plan/onboarding-tour-brief.md) (L1 shared core, L2 CLI tour, L3 dashboard, L4 agent notice)
- [ ] P1 foundation + Home, P2 Project brain, P3 Agents & locks + Inbox, P4 Sync wizard + Safety, P5 polish

## Configurable embeddings and secure keys (plan: PLAN §10)

- [x] E1 `secrets.rs` on the `keyring` crate (encrypted-file fallback only without a keychain); `embeddings key set|status|remove`; master key written through keyring, not the command line, `embeddings key set|status|remove`, master key off the command line
- [x] E2 profiles + preset catalog, `embeddings profile add|list|use|remove`, `embeddings models` (lists installed Ollama models); the flat fields stay as the `default` profile
- [x] E3 `/api/recall*` (all writes behind the human password, keys write-only) and a real Recall settings page
- [x] E4 indexing with progress, retries, daily token cap and cost estimate, cancel, unused-vector cleanup (`embeddings reindex|clean`, `/api/recall/index`)
- [ ] E5 native Cohere/Voyage/Gemini adapters, ONNX feature (not started; the OpenAI-compatible adapter covers most services)

## Control everything safe from the dashboard (plan: PLAN §11)

- [x] U1 `policy.rs` tiers + route audit test, job runner (`/api/jobs`, Jobs tray), indexing on it, local indexing without password
- [x] U2 Settings > Tools / Health (doctor, export-rules, hand-off, sync pull, docs rescan, graph index, eval, backup); logic reused via subprocess (`run_self`) and library calls, no separate `ops/` module
- [x] U3 Agents > Set up an agent (preview, connect, disconnect)
- [x] U4 Decisions view; restore, pin, veto, revoke in Project brain
- [x] U5 Settings > Locks and Data; activity log view

## Next: quality, proof, reach (plan: PLAN §12)

- [x] N1 Proof: eval harness with/without AutoMemory, published numbers, CI gate
- [x] N2 Optional LLM helper: conflicts, duplicates, session distill (proposals only; one profile). Staleness without an LLM: Tidy up flags notes that mention files that no longer exist
- [x] N3 Memory hygiene UI: confidence, last used, never recalled, bulk archive
- [x] N4 Agent matrix test: 13 agents connect, get their instruction file and disconnect cleanly. Hooks/contract fixtures for Cursor, Windsurf, Gemini CLI, Copilot still need recorded real payloads (they stay experimental, MCP + rules only)
- [x] N5 Team sync (first step): records carry their author, notes from a teammate show "from <name>", approved notes arrive active and unapproved ones stay candidates (tested with two people on a keyed vault). Not built: per-person trust tiers (a receiver-side overlay is needed; clamping the record itself makes fingerprints diverge and creates false conflicts), person setting UI
- [x] N6 Performance gate in CI (hook latency, recall, binary size)
- [x] N7 `automemory update` (asks first, verifies .sha256, refuses unchecked downloads; scenario with a mock release server), installers verify checksums, `scripts/make_formula.sh` prints a Homebrew formula. Not done: Apple signing/notarisation (needs a developer account), publishing the tap
- [x] N8 Hardening audit of `serve/`, sync and export-rules. Fixed: plaintext sync deltas accepted by a keyed vault (forged trusted memories), `is_loopback` substring bypass (`localhost.evil.com` skipped consent and redaction), export-rules following symlinks out of the repo, unbounded backups, agent name as CLI flag. Not covered: fuzzing the hook JSON parsers
- [ ] Small leftovers: T10 trial, lock icons from `/api/policy`, richer "recently blocked" view, decision-card wording

## Next improvements (plan: PLAN §13)

- [x] I1 Value report: `automemory stats`, `/api/value`, Home "Last 7 days" card (figures come from existing rows; sample data excluded)
- [ ] I2 `hook --record` to capture real payloads; fixtures for Cursor/Windsurf/Gemini/Copilot
- [ ] I3 Trust overlay table for teams (teammate pins arrive as requests)
- [x] I4 Browser smoke test (Playwright): 21 pages x light/dark, password prompt; skipped with a message when Playwright is missing
- [ ] I5 Lock hints from `/api/policy`
- [ ] I6 `backup restore` + doctor backup check
- [ ] I7 LLM profile map, native Cohere/Voyage/Gemini embedding adapters
- [ ] I8 Fuzz hook JSON, shell parser, delta files
- [ ] I9 Windows CI for update/hooks/connect
- [ ] I10 Cosmetic backlog

## Docs hygiene

- [x] Keep tool and test counts in ARCHITECTURE/FEATURES in sync with code (60 MCP tools, 228 Rust tests at last check)

## Later (after v1 passes its gates)

- [ ] Promote additional agents one at a time (Cursor, Cline, …) with contract fixtures
- [ ] Cloudflare managed SaaS (`workers/`), team sharing
- [ ] Headless Runner (check vendor terms first), Autopilot profile
- [ ] Neural embeddings, WASM plugins
