# AutoMemory — Features

What the product does, for whom, and how far along each piece is. How it is built: [ARCHITECTURE](ARCHITECTURE.md). Order of work: [PLAN](PLAN.md). Open work: [TASKS](TASKS.md).

## 1. Product

AutoMemory is a local-first daemon that gives every coding agent one shared, verified project memory plus file locks and handoffs. Agents reach it through MCP tools and optional hooks, so nobody re-explains the project each session.

| It is | It is not |
|---|---|
| Local Rust daemon, SQLite WAL, MCP server, CLI, dashboard | A hosted service or LLM |
| Memory of *why*: decisions, constraints, gotchas | A code indexer (CodeIntel bridge supplies the graph) |
| A referee: locks, trust tiers, verification, audit ledger | An LLM that writes answers or judges code |
| Pull-based: agents query on demand | A default boot-time prompt injector |
| Modular: every part behind a port | A monolith with fixed hooks |

**Problems it targets:** agent collisions (cross-agent merge conflicts reported at 41.7% vs 19.8% same-agent), lost context between agents, stale and contradicted memory, memory poisoning, retrieval noise, destructive commands, and no proof that memory helps. Deeper gap analysis: [research/multi_agent_ecosystem_needs.md](research/multi_agent_ecosystem_needs.md).

**Primary user:** one developer running 2–3 agents on one project across terminal and desktop apps. Later: small teams.

**Use cases:** switch agent mid-feature (`memory.about` + `baton.pickup`); parallel agents without overwrites (lease conflict shows holder/goal/age); "what did we decide?" (`memory.ask` with verified/changed/stale status); refactor or revert flips linked memories; plan a risky change in a Planning Room; catastrophic command blocked; swap a hook or adapter without restart.

## 2. v1 scope

| In v1 | Later (only after v1 works) |
|---|---|
| Core: ledger, store, plugin host, hook pipeline | More agents beyond the first three (17 connectors already exist, unsupported) |
| Adapters: **Claude Code, Codex, Antigravity** | Cloudflare managed SaaS / team sharing |
| Memory, decisions, trust tiers, verification | More agents beyond the first three |
| Locks, baton handoff, cross-review | Headless Runner, Autopilot as default |
| **Planning Room orchestration** | WASM plugins, neural embeddings |
| **Nuxt dashboard** (local `serve`) | Automated PRD/ADR generation, policy engine |
| Guardrails + human password gate | |
| Git-vault sync with encrypted keys | |
| Eval harness (memory on vs off) | |

Cut list: symbol-level leases, own AST/code graph (codebase-memory-mcp supplies it), auto-distill of every session, auto-generated AGENTS.md, shell-regex arms race, storing raw agent reasoning, agent-edited memory, debate as default, LLM calls in the daemon.

## 3. Feature catalog

Status: ✅ built · 🔧 in progress · ⬜ planned. "Verified" = measured or tested in this repo.

### Memory and recall
| Feature | Status | Notes |
|---|---|---|
| `ask`, `about`, `get`, `topics`, `search`, `recall`, `propose` | ✅ | FTS5 BM25 × time decay × confidence; ~0.56 ms over 500 memories (bench) |
| Candidate tier, promote on verify/human confirm | ✅ | `propose` rejected without provenance |
| Scope-aware dedup + version history | ✅ | In-place upsert |
| Decay, supersede chains, contradiction flag | ✅ | Contradiction = same tool, different version |
| Vector search | ✅ keyword (default) or hybrid; providers ngram, ollama, openai-compatible, exec | `onnx` build feature |
| Procedural skills from trajectories | ✅ | `memory.skills` |
| Failure catalog (`failure.save/resolve`, proactive warning) | ✅ | |
| Constitution + token-capped context | ✅ | `boot_injection` off by default |
| Time-span digest (`15m`…`session`) | ✅ | Edits, files, red→green, failures paired with fixes |

### Verification and trust
| Feature | Status | Notes |
|---|---|---|
| Trust tiers 1–5, provenance, revoke | ✅ | Tier 4–5 withheld from default recall |
| Symbol/file-hash verify on recall | ✅ | File hashes built in; symbols from codebase-memory-mcp |
| PostCommit flips linked memories to `changed` | ✅ | |
| Revert/rewrite staleness | ✅ | A revert is a commit: PostCommit flips memories linked to its files to `changed` |
| Contradiction across memories | ✅ | Consolidation worker: replacement cues, mutually exclusive tool groups, version mismatch |

### Coordination
| Feature | Status | Notes |
|---|---|---|
| File/glob leases, TTL, heartbeat, override | ✅ | Advisory in Vibe, hard in Strict |
| Automatic locking (hook leases, shell prediction, `git status` reconciliation, prompt visibility, idle-holder watcher, coverage report) | ✅ | Agents never call `lock.acquire`; see ARCHITECTURE §7. Hook-less agents get alerts only |
| Worktree-shared memory/decisions/leases | 🔧 | Uncommitted work in `sync/` |
| Baton handoff | ✅ | |
| Task queue + cross-review (tests as judge) | ✅ | |
| Planning Room state machine + MCP tools | ✅ | Gated by its own A/B eval |
| Blackboard / checkpoint | ✅ | `lock.status` is the blackboard; `memory.digest auto_save` and the automatic `Session checkpoint` on Stop cover checkpoints |
| Pre-merge file-overlap check | ✅ | `automemory overlap`; file-level, git only |

### Safety
| Feature | Status | Notes |
|---|---|---|
| Shell tokenizer guardrail, Vibe/Strict/Review | ✅ | ~1.26 ms hook pipeline mean |
| Custom rules, Zero-SQL rule, review proposals | ✅ | |
| Human password gate for policy writes | ✅ | |
| Shadow mode for new rules | ✅ | Measure false-block rate |

### Interfaces
| Feature | Status | Notes |
|---|---|---|
| MCP server (60 tools), CLI | ✅ | |
| Nuxt dashboard + REST + WebSocket | ✅ | v1 scope; includes Agents & Locks (leases, rooms, batons) |
| Hook pipeline, hot reload, custom exec handlers | ✅ | |
| `next_action` on every tool reply | ✅ | |

### Sync
| Feature | Status | Notes |
|---|---|---|
| Git-ignored plan vault, AES-256-GCM, BIP-39 pairing | ✅ | |
| Cloudflare managed mode | ⬜ later | `workers/` |

### Proof
| Feature | Status | Notes |
|---|---|---|
| Benchmarks (`cargo bench`) | ✅ | Hook ~1.26 ms, recall ~0.56 ms, distill (40 events) ~0.11 ms, binary 10.3 MB, 204 Rust tests + 11 script suites ([tests/README.md](../tests/README.md)) |
| Eval harness | 🔧 | `tests/eval/run_eval.py` (prepare / score / report) is ready; needs ≥20 real tasks and the runs (Phase 2 gate). `automemory eval` is only a self-check suite |

## 4. Success metrics

| Metric | Target |
|---|---|
| Silent overwrites | 0 |
| Hook p95 | < 5 ms for the pipeline (≈1.3 ms mean). End to end, process start included: ≈9 ms p95, ≈5 ms of it binary start-up (`tests/perf/hook_p95.py`) |
| Task success, memory on vs off | Not worse overall; better on non-inferable tasks |
| Stale-flag precision | ≥ 90% |
| Install → first lock or recall | < 5 min |
| Tokens per session vs baseline | Lower |

Not measured and therefore not claimed: distillation compression ratio, dedup row counts, third-party latency comparisons.

## 5. Acceptance criteria (v1)

New chat continues from prior decisions; projects never leak into each other by default; global preferences apply unless overridden; capture works with no agent cooperation; a failure with a later fix becomes a linked record; works fully offline; synced data is encrypted before leaving the machine.
