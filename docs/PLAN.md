# AutoMemory — Plan

Order of work, gates, risks and open decisions. Scope: [FEATURES](FEATURES.md). Design: [ARCHITECTURE](ARCHITECTURE.md). Checklist: [TASKS](TASKS.md).

Rule: every phase ends with a measurement. Later phases start only if the gate passes.

## 1. Phases

| Phase | Ship | Exit gate | State |
|---|---|---|---|
| 1 | Daemon, ledger, plugin host, hook pipeline, Claude Code adapter, file locks | Zero silent overwrites in a two-agent test; hook p95 < 5 ms | Built; two-agent test to confirm |
| 2 | Eval harness | Baseline on ≥ 20 real tasks | In progress |
| 3 | Memory with trust tiers, `ask/about/propose`, decisions, CodeIntel bridge, verify + stale flags | Memory-on not worse than off; stale-flag precision ≥ 90% | Built; gate unmeasured |
| 4 | Commit-bound staleness, contradiction detection, guardrail shadow mode, pre-merge file-overlap check | Fewer stale hits and merge conflicts than phase 3 | Partial |
| 5 | Cross-review with tests as judge, baton, Codex adapter, Antigravity adapter | Success ≥ solo agent; acceptable cost | Built; gate unmeasured |
| 6 | Planning Room (two agents, one round) for high-stakes design only; Nuxt dashboard polish | Beats solo + tests on a defined task set, else Room dropped | Built; A/B pending |
| 7 | Add agents beyond the first three; Cloudflare SaaS; headless Runner; Autopilot; neural vectors | Each justified by measured need | Not started |

Phases 1–3 are the minimum credible product: coordination you can feel, memory you can trust, proof it helps.

**Immediate focus:** make Claude Code + Codex + Antigravity work end-to-end and measured. Do not add agents or Cloudflare until that passes.

## 2. Eval plan (Phase 2)

Tasks: real issues from the owner's repos (Rust, Swift, Kotlin, TypeScript), including ones whose deciding context is not in the code. Arms: memory off / on / on+verify; later solo vs cross-review vs Room. ≥ 3 runs per task per arm, paired comparison. Score from exit codes, not agent claims. Track tokens, tool calls, wall time. Inject stale/contradictory and poisoned memory on purpose. Metrics: success delta, corrections per task, tokens per task, stale-flag precision/recall, retrieval precision@3, collisions, hook p95 and false-block rate.

## 3. Risks

| Risk | Mitigation |
|---|---|
| dibs-style tools already cover locks/handoff | Evaluate dibs; differentiate on verified memory |
| Vendors absorb the idea | Stay cross-vendor; verifier + trust layer are the moat |
| Hook APIs change often | Thin adapters, manifests, contract tests, degrade ladder |
| Agents ignore MCP tools | One-line pointer, nudges, `next_action`, Stop hook |
| Desktop apps/Cursor lack hooks | MCP-only protocol, file-watcher fallback |
| Memory does not help in eval | Cut to locks, decisions, verification |
| Debate adds cost without gain | Room stays gated by its own A/B |
| Guardrail false positives | Shadow mode, measure, fail open for non-catastrophic |
| Plugin attack surface | Capabilities, sandbox, pinned versions |
| Third-party schema drift (codebase-memory-mcp) | `CodeIntel` trait, pinned version, hash fallback |
| Vendor terms restrict headless use | Check before shipping Runner |
| Scope creep | Phase gates; cut list in FEATURES |

## 4. Decisions

Decided: PRD content is the source of truth, now split across these four docs; v1 = Nuxt dashboard + Planning Room + 3 agents; Cloudflare SaaS and more agents come after; all docs live in `docs/`.

Open:

| Decision | Options | Lean |
|---|---|---|
| Locks: build or adopt dibs | build / adopt | Evaluate dibs for a week |
| Main pitch | safety / memory / glue | Trusted memory + no collisions |
| Plugin runtime | exec only / + WASM | Exec first |
| Claude desktop trigger | standing instruction / slash command / toast | Instruction + slash command |
| Sync backend after git vault | folder / other | Folder or git remote of encrypted ops |
| Relation to AgentMesh | one core / shared crate / separate | One core |

## 5. Landscape and sources

Closest overlap: dibs (claims, handoff), lockpick, GitGuardex (locks), Entire CLI (ledger), unified-agent-context / Engrim / Observational Memory (shared memory), codebase-memory-mcp (our CodeIntel default), Mem0/Zep/Letta (generic memory). Evidence behind the design (reported claims, several vendor/preprint): 33,596-PR agent-collision study; ICLR 2026 AGENTS.md evaluation (context files gave no gain, +20% cost); ICML 2026 memory-poisoning study (~50% attack success); MemConflict (conflict recognition ≤ 0.25); MAST multi-agent failure taxonomy; coding-agent deletion incidents and permission fatigue (~93% prompts approved). Full links were in the former PRD §16.4; recover with `git show HEAD:docs/PRD.md`.

## 6. Automatic locking v2

Today (done): the pre-edit hook takes a file lease on the first edit, renews it on each edit, and Stop releases it; a second agent is warned (Vibe) or denied (Strict). Goal: agents never think about locks, and a collision is stopped or at least seen in every case that matters.

**Where the current design leaks**

| Leak | Case |
|---|---|
| Shell writes are invisible | `sed -i`, `>`/`tee`, `mv`, `cargo fmt`, `git checkout -- f`, `npm install` change files with no lease taken |
| Lease dies each turn | Stop fires after every response in Claude Code, so work spread over several turns loses the lease between them |
| Unhooked agents | Cursor, Cline, Aider and desktop apps run no hooks, so nothing locks anything |
| Advisory only | In Vibe the second agent is told but can ignore it |
| Whole-file, whole-turn | A long edit session blocks the other agent even on parts that do not overlap (merge-time overlap check covers this) |

**Layers, cheapest first (each stands alone)**

1. **Lease lifecycle (small).** Tie the lease to the session, not the turn: renew on any tool event (reads too), release on SessionEnd or after N minutes idle (default 10), not on Stop. Optional `wait` mode: the hook waits up to N seconds for a held file before warning or denying.
2. **Shell write prediction (medium).** Parse the command for write targets (redirects, `tee`, `sed -i`, `mv`/`cp`/`rm`, `git checkout|restore|stash|rebase`) and lease those paths the same way edits do. Unparseable formatters/installers fall to layer 3.
3. **After-the-fact reconciliation (medium).** PostToolUse compares `git status --porcelain` before and after each tool call. Files that changed without a lease are attributed to the session and leased retroactively, and if another session holds one, both agents are told. Catches anything layer 2 cannot predict.
4. **Watcher for unhooked agents (medium, alert-only).** `serve` watches the project tree; a change while two or more sessions are active, or a change to a leased file by anyone else, raises a dashboard alert and a note in the next hook output. It cannot attribute precisely or block, only warn.
5. **Hard isolation (large).** `automemory run <agent>` creates a git worktree per session and starts the agent inside it (hooks cannot change an agent's working directory, so a launcher is required). Same-file collisions become impossible; `automemory overlap` and the PreCommit check handle merge time. This is the Runner port from the PRD.
6. **Policy (small).** Per-path rules in config: `hard` paths always deny, `soft` paths warn, `ignored` paths (build output, lockfiles) never lease.

**Order and gates**

| Step | Delivers | Verified by |
|---|---|---|
| 1 (layer 1 + 6) | Multi-turn leases, idle expiry, policy | `tests/scenarios/collision.py` gains a multi-turn case; unit tests for idle expiry |
| 2 (layers 2 + 3) | Shell writes and unknown writes covered | New guardrail-style table of shell commands with expected write targets; scenario where agent B's `sed -i` on A's file is reported |
| 3 (layer 4) | Alerts for agents without hooks | Dashboard scenario; manual check with Cursor or Cline |
| 4 (layer 5) | Optional hard isolation | End-to-end: two launched agents, no collision possible, overlap report at merge |

Decide before step 2: whether reconciliation may *deny* the next call of the offending session in Strict mode, or only warn. Decide before step 4: ship the launcher inside v1, or leave it to Phase 7 with the headless Runner.

