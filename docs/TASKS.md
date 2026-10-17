# AutoMemory — Tasks

Open work, derived from [PLAN](PLAN.md) and the current working tree. Update as items land. Tick = done and verified.

## Now (finish v1 on three agents)

- [ ] Review and commit the uncommitted worktree-sync work (`sync/deltas.rs`, `sync/git.rs`, `leases.rs`, `projects.rs`, `decisions.rs`, `schema.sql`, `connect.rs`, `hook.rs`, `config.rs`, `tests/worktree_sync_test.rs`)
- [ ] Two-agent collision test (Phase 1 gate): zero silent overwrites; record result
- [ ] Verify Claude Code, Codex, Antigravity adapters end-to-end: install, hooks fire, MCP tools reachable (contract test per agent version)
- [ ] Mark the other 17 connectors as unsupported in `connect`/`doctor` output until promoted
- [ ] Confirm hook p95 < 5 ms (bench reports mean only)

## Eval (Phase 2 gate)

- [ ] Collect ≥ 20 real tasks across Rust/Swift/Kotlin/TS repos
- [ ] Run `automemory eval` arms: memory off / on / on+verify, ≥ 3 runs each
- [ ] Measure stale-flag precision (target ≥ 90%) and retrieval precision@3
- [ ] Plant stale and poisoned memories; confirm flags and quarantine

## Planning Room and dashboard (v1)

- [ ] Room A/B: solo + tests vs Room on a defined task set; drop Room if it does not win
- [ ] Dashboard: confirm all views work against current API; add Room and lease/baton views
- [ ] Rebuild `clients/web` and refresh embedded assets

## Gaps from research (verify before building)

- [ ] Check whether `lock.status` + `baton` already cover the "blackboard" idea; add `memory.checkpoint` only if not
- [ ] Auto-checkpoint on `stop` so sessions leave a progress memory
- [ ] `memory list --group-by category|time|agent` — confirm shipped or add
- [ ] Commit-bound staleness on revert/rewrite
- [ ] Cross-memory contradiction detection beyond version mismatch
- [ ] Symbol-level leases and pre-merge overlap check (needs CodeIntel symbols)

## Docs hygiene

- [ ] Keep MCP tool count and test count in ARCHITECTURE/FEATURES in sync with code (59 tools, 120 tests at last check)
- [ ] Generate CLI/MCP reference from code if a reference doc is wanted again

## Later (after v1 passes its gates)

- [ ] Promote additional agents one at a time (Cursor, Cline, …) with contract tests
- [ ] Cloudflare managed SaaS (`workers/`), team sharing
- [ ] Headless Runner (check vendor terms first), Autopilot profile
- [ ] Neural embeddings, WASM plugins
