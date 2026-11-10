# Roadmap

Open work, gates, risks and undecided questions. What already shipped: [CHANGELOG](../CHANGELOG.md). What the product does: [features](features.md). How it is built: [architecture](architecture/overview.md). Settled choices: [decisions](decisions.md).

Rule: every phase ends with a measurement. A later phase starts only if the gate passes.

## Phases

| Phase | Ship | Exit gate | State |
|---|---|---|---|
| 1 | Daemon, ledger, hook pipeline, Claude Code adapter, file locks | Zero silent overwrites in a two-agent test; hook p95 < 5 ms | Built; the 5 ms target is about the pipeline (met, ~1.3 ms), the whole process is ~9 ms |
| 2 | Eval harness | Baseline on at least 20 real tasks | Harness built; real tasks not written |
| 3 | Memory with trust tiers, decisions, CodeIntel bridge, verify and stale flags | Memory-on not worse than off; stale-flag precision at least 90% | Built; gate unmeasured |
| 4 | Commit-bound staleness, contradiction detection, guardrail shadow mode, pre-merge overlap check | Fewer stale hits and merge conflicts than phase 3 | Partial |
| 5 | Baton, Codex and Antigravity adapters | Success at least a solo agent; acceptable cost | Built; gate unmeasured |
| 6 | Planning room, dashboard, first-run tour, continuity, library | Beats solo plus tests on a defined task set | Built; the A/B is open |
| 7 | More agents, hosted sync, headless runner, neural vectors | Each justified by measured need | Not started |

**Focus:** Claude Code, Codex and Antigravity working end to end and measured. No new agents or hosted service until that passes.

## Open work

### Needs a person
- Live sessions: `tests/manual/LIVE_CHECKLIST.md` sections A to E; trials T10, T12 (continuity) and T13 (library) with the scripts in `tests/manual/`.
- Phase 2 gate: at least 20 real tasks in `tests/eval/tasks/`, arms `off` / `on` / `verify`, at least 3 runs each (method in `tests/eval/README.md`); then the 10-task memory-off-versus-on proof (X3). This decides what to keep.
- Measure stale-flag precision (target 90%) and precision@3; plant stale and poisoned notes and confirm flagging, quarantine and revoke.
- Planning room A/B: solo plus tests against a room; drop rooms if they do not win (live trial T11).
- Run the Codex, Command Code and Cursor transcript readers on real chats and replace the fixtures with redacted real samples.
- Before deploying `workers/sync-gateway`: set the webhook secrets and `ADMIN_SECRET`; leave `ENABLE_DEV_AUTH_BYPASS` unset.
- Verify in a live Antigravity session that the PreInvocation message reaches the model and Strict-mode deny blocks the call.

### Product
| Item | Why | Done when |
|---|---|---|
| X2 MCP 2026-07-28 compatibility: stateless requests, `resultType`, old handshake kept | Hosts will move to it | Contract tests for both versions pass. **Partly built: version negotiation and `server/discover`; calls already need no handshake. Still to do: check `resultType` and `_meta` against modelcontextprotocol.io, and a contract test with a real 2026 client** |
| X4 `automemory worktree` helper wired to tasks and overlap checks | Practitioners isolate parallel agents with worktrees | Two agents edit one file in separate worktrees; overlap warns before merge |
| X5 SKILL.md export of verified knowledge | One format reaches 30+ tools | A skill written for one agent is read by a second; disconnect removes only ours. **Built: `skill export --agent claude\|agy\|all`, `skill unexport`, disconnect removes only ours; the live check that a second agent reads a skill is still to do** |
| X6 `valid_from` / `invalid_at` and a "what changed" view | Contradiction and forgetting are where memory fails | A superseded decision stays readable with dates; recall never returns an invalid note. **Built without a schema change: `decision.get` returns `valid_from` / `invalid_at`, `automemory changed --since 7d`; the end date of a memory is its last change, not a stored column** |
| X8 Rules text: do only what was asked (no commits, batons or failure housekeeping unasked) | A trial agent committed 16 files unasked | The same neutral prompt no longer commits. **Text changed and tested; the live trial is still to run** |
| X9 Hook adapters for Cursor, Windsurf, Gemini CLI, Copilot from recorded payloads (`automemory hooks record`) | Biggest reach gap; needs real captures | One fixture per agent |
| Rooms R5 to R7: decision-record export, room types and roles, chat-style post ownership | See [specs/rooms.md](specs/rooms.md) | Only after the room A/B shows value |
| Library L6: attachments, model abstracts | See [specs/library.md](specs/library.md) | Decide after use |
| Continuity: Z code reader | Last unwritten reader | Passes the reader testkit |

### Technical
- Symbol-level hash for staleness: wire `CodeIntel::symbol_hash` into the verifier (file hashes today).
- Contract fixtures per agent version once more than one version is in use.
- Connect and disconnect are two parallel per-agent lists; move `rules_target()` and `disconnect()` onto the adapter trait.
- Optional wait-for-lock mode; hard/soft per-path policy beyond `ignore`; decide whether Strict mode reconciliation denies the next call (today it warns).
- The watcher only sees registered projects.
- ONNX embedding provider and vector sync between machines (each machine re-embeds today).
- Per-person trust tiers for team sync and a person-setting UI.
- Coverage-guided fuzzing in CI (seeded fuzzing exists).
- Read `automemory locks --coverage` after a week of real use; below 80%, look at which writes dominate.
- From the Nov 2026 architecture research (not built; weak evidence marked in the report):
  - Sync v2 is built (see [specs/sync-v2.md](specs/sync-v2.md)); still open: a live two-computer trial with the real keychain, a per-person signing identity for team vaults, and a dashboard notice for a revoked computer. The hybrid logical clock from the research was not needed.
  - A smaller default MCP tool list (about 10 of 49) with the rest behind a category listing; changes the tool contract, so decide first.
  - Eval arms beyond off/on/verify: a static AGENTS.md arm and seeded stale and poisoned memories; report cost with success.
  - A "changed by" field next to a stale memory, read from Tracewise (`history.rs`), and one owner for locks when both tools are installed.
  - An IDE extension for one-click `connect`, a lock and approval status bar and an approval inbox (no chat participant).

### Packaging and hosting
- Windows: the CI job is written but has not run; make it required after its first green run.
- Apple notarisation and a Homebrew tap (need a developer account).
- Cloudflare managed service (`workers/`), team sharing, headless runner (check vendor terms first), autopilot profile, WASM plugins.

## Evaluation plan

Tasks: real issues from the owner's repositories, including ones whose deciding context is not in the code. Arms: memory off, on, on plus verify; later solo against room. At least 3 runs per task per arm, paired comparison. Score from exit codes, not agent claims. Track tokens, tool calls and wall time. Inject stale, contradictory and poisoned notes on purpose. Metrics: success delta, corrections per task, tokens per task, stale-flag precision and recall, precision@3, collisions, hook p95 and false-block rate.

## Risks

| Risk | Mitigation |
|---|---|
| Lock and hand-off tools already exist (dibs, lockpick) | Differentiate on verified memory |
| Vendors absorb the idea | Stay cross-vendor; the verifier and trust layer are the moat |
| Hook APIs change often | Thin adapters, contract tests, degrade ladder |
| Agents ignore MCP tools | One-line pointer, nudges, `next_action`, Stop hook |
| Desktop apps and Cursor lack hooks | MCP-only protocol, watcher fallback |
| Memory does not help in the eval | Cut to locks, decisions and verification |
| Rooms add cost without gain | Gated by their own A/B |
| Guardrail false positives | Shadow mode, measure, fail open for non-catastrophic |
| Third-party schema drift (codebase-memory-mcp) | `CodeIntel` trait, pinned version, "unknown" fallback |
| Vendor terms restrict headless use | Check before shipping a runner |
| Scope creep | Phase gates; cut list in [features](features.md) |

## Undecided

| Decision | Options | Lean |
|---|---|---|
| Main pitch | safety / memory / glue | Trusted memory plus no collisions |
| Plugin runtime | exec only / plus WASM | Exec first (plugin and adapter commands are hidden until a second implementation exists) |
| Sync transport after the git vault | folder / hosted gateway | Git remote of encrypted deltas |
| Relation to AgentMesh | one core / shared crate / separate | One core |
| Should hybrid recall stay opt-in per project | yes / no | Yes until `embeddings eval` beats keyword |

## Landscape

Closest overlap: dibs, lockpick, GitGuardex (locks), Entire CLI (ledger), unified-agent-context, Engrim, Observational Memory (shared memory), codebase-memory-mcp (our CodeIntel engine), Mem0, Zep, Letta (generic memory). Evidence behind the design (reported claims, several vendor or preprint): a 33,596-PR agent-collision study; the ICLR 2026 AGENTS.md evaluation (context files gave no gain, plus 20% cost); the ICML 2026 memory-poisoning study (about 50% attack success); MemConflict; the MAST multi-agent failure taxonomy; coding-agent deletion incidents and permission fatigue (about 93% of prompts approved). More: [research/](research/).
