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
