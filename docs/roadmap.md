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

### Memory quality plan (Nov 2026 research)
Source: [the report](research/reports/AutoMemory%20memory%20system%20improvements.md) (five note files beside it). Verdict: the design is on the right side of the evidence; what is missing is proof, a stricter write policy and trust that bites at retrieval time. Items run in order; item 1 gates the rest.

| # | Change | Done when | State |
|---|---|---|---|
| 1 | Labelled recall set plus `automemory eval recall` (recall@5, nDCG@10, MRR per query kind); then replay real sessions (at least 50 queries per project type) and a coding-task harness on a VibeMemBench or SWE-ContextBench slice | Numbers published with intervals; no claim without them | **Started.** Scorer, command and a 40-note, 56-query synthetic set exist (48 plus 8 held-out). Baseline before item 5: recall@5 0.946, nDCG@10 0.900, MRR 0.884; paraphrase queries 0.750 (the lexical gap, not ranking). Real replayed sessions and the coding harness are still to do |
| 2 | Write policy: refuse or demote notes derivable from the repository; cap the managed block near 100 lines with the code-precedence sentence | Tokens injected and steps per task fall, resolve rate does not | **Built (write side):** `memory.save` keeps a layout, overview or standard-command note an agent wrote out of recall as a candidate with the reason (`write_policy.rs`; decisions, lessons, warnings and anything a person wrote are never judged). The managed block is already capped at 3 KB. Still to measure: tokens per session and resolve rate |
| 3 | Trust tiers as occupancy caps, not score weights (at most N of k slots for agent claims; never displace pinned or verified on the same key) | Seeded-poison attack success falls, benign recall unchanged | **Built:** with `include_untrusted`, agent claims and untrusted notes take at most `limit/3` slots (at least 1) and trusted notes move up (`cap_low_tiers`, tested). Tier labels were already on every row. Still to do: the seeded-poison attack-success measurement |
| 4 | Decay ablation, then a floor or per-kind half-life; decay as a staleness filter after hash verification | nDCG@10 current against floored against per-kind | **Part built.** Finding: decay was never a ranking multiplier in keyword recall; it fed only the `stale` label, and one 14-day half-life made every unverified note read `stale` after about 31 days. Now by kind: decisions, preferences and person-written notes never go stale by age; facts and observations 30 days; lessons, warnings, patterns, skills 90 (first guesses, tested by `age_flags_depend_on_the_kind_of_note`). Still to do: confirm the numbers on real notes and add access reinforcement |
| 5 | Index-time key expansion (dates, paths, symbols, project synonyms), porter tokenizer with identifier characters, trigram fallback | recall@5 up on the set; hook p99 under 5 ms at 5K and 50K notes | **Part built:** query-time synonym groups (built-in plus `[retrieval] synonyms`): paraphrase recall@5 0.750 to 0.917, all 0.946 to 0.982, 1.10 ms per recall at 5,000 notes. The groups were written knowing the set's vocabulary, and the 8 held-out queries were already perfect, so real sessions must confirm it. Still open: index-time expansion, tokenizer change (needs a migration), trigram fallback, 50K-note latency, "back office" (two-word phrases) |
| 6 | Valid-from, invalid-at and superseded-by on notes; a deterministic nightly pass (re-verify hashes, merge duplicates, demote unverified claims) | Newer fact supersedes older in a stale-fact test | **Built (the pass; not the columns):** `automemory tidy [--dry-run]` and a nightly run inside `serve` file three kinds of Inbox item and change nothing themselves: a note that repeats another (the better-trusted, older one stays; a person's note is never the one offered), a note whose files are all gone (only judged on a machine where the project folder exists), and the unused-note offers. `doctor` shows when it last ran. Decisions already carry valid-from and invalid-at dates; notes get no new date columns (an archived note keeps its dates). Still to do: re-verify hashes in the pass, demote old unverified claims |
| 7 | Progressive disclosure in MCP: search rows of 50 to 100 tokens with ids, then get | Session-start and per-task tokens fall, resolve rate unchanged | **Built:** `memory.recall` takes `detail: "index"` and returns one row per note without the body (with `content_chars`), about a third of the size; `memory.get` fetches the body. Default stays `full`. Not yet measured: session-start tokens and resolve rate |
| 8 | Write-time secret scan with embedded gitleaks rules plus entropy | No known-pattern secrets in a seeded corpus; under 2% false positives | **Part built:** nine more secret shapes (Slack, Stripe, Google, npm, SendGrid, GitLab, Hugging Face, passwords inside connection URLs, AWS secret keys) on top of the existing set; a seeded corpus catches all of them and passes eight ordinary sentences about tokens and keys. Not built: the full gitleaks rule set, an entropy check, a per-vault allowlist |
| 9 | The ledger records reads (what was served or refused, with tier) | Every poison-test retrieval traceable | **Built:** every `memory.recall` writes an audit row (`memory.recall.served`: connection, which notes at which tier, whether untrusted notes were requested). Refusals are not logged yet |
| 10 | Versioned JSON export of notes, decisions, brief and library with tier, signature and evidence, plus a Markdown export; importers for Claude Code auto memory and claude-mem | Export, wipe, import gives the same ledger head | **Part built:** `automemory export [--out file] [--markdown dir] [--project p]` writes a versioned JSON file (notes with tier and status, decisions with evidence) and a Markdown folder with YAML front matter; `automemory import <file>` adds what is missing, never overwrites, asks for the human password, and refuses files of another or a newer version (round trip tested). Not built: library files and the brief in the export, importers for Claude Code auto memory and claude-mem, signatures in the file |
| 11 | Backup through the online backup API; verified restore (integrity, count, ledger head, atomic swap); scheduled drill and its age in `doctor` | Drill passes with zero errors | **Built:** backups already use `VACUUM INTO`. Now `automemory backup drill` restores the newest backup to a scratch file, opens it like a real restore and counts notes, decisions and events; restore checks its own copy before swapping; `serve` saves a backup when the newest is over 20 hours old and runs a drill weekly; `doctor` shows the drill's age. Not built: encrypted archive export to a second medium |
| 12 | Remote compaction (fold old deltas into snapshots) and opaque remote names | Cold restore reads about 12 files; no plaintext names remotely | Open |
| 13 | Per-field version and device clock for deterministic last-writer-wins | Two-device concurrent edit converges in either order | Open |
| 14 | Team keys: device, per-person and vault keys with rotation on removal | A revoked device cannot read the next delta | Open |
| 15 | Optional vectors: static embeddings (model2vec) in the binary, int8 brute force, fusion; only if item 1 shows a gain | Hybrid beats keyword on the set; binary size and hook p99 stay in budget | Open (the paraphrase gap in item 1 is the case for it) |

Deliberately not planned: a graph store (about 2 points for 2 to 3 times the cost), a reranker on the hook path, LLM-driven add, update and delete (the design the market leader abandoned).

### Tracewise as companion (locks and file awareness move out)
Decided Nov 2026, P1 and P2 built: [specs/tracewise-companion.md](specs/tracewise-companion.md). AutoMemory stops locking; when Tracewise is installed it is called from inside AutoMemory's own hook (nested, one reply per tool call) for locks, shell-write tracking, collision notices and the Agents & Locks and Replay pages. Without it AutoMemory has no locks and says so. Phases P1 (Tracewise `--neutral` hook output and lock JSON: **done, merged**), P2 (companion client: **done, uncommitted**; p95 14.6 ms), P3 (remove lock code and tables, proxies: **done, uncommitted**), P4 (setup and doctor: **done, uncommitted**), P5 (live two-agent test: **latency measured, p95 17 ms on tool hooks; live run pending**).

### Technical
- Recall measurement: the sentence-query fix is tested on five notes; measure hit rate on real notes and real agent questions (eval `non_inferable` tasks) before trusting it. Synonyms and plurals beyond a prefix need hybrid mode.
- Verification sprint ([research/verification_sprint.md](research/verification_sprint.md)): all items fixed or documented. Hook-captured notes keep the file-level link (hook payloads carry no symbols).
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
