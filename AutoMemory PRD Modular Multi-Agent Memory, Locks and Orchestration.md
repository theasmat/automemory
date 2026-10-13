# AutoMemory PRD: Modular Multi-Agent Memory, Locks and Orchestration

Oct 12, 2026 · @Asmat

## 1. Overview

AutoMemory is a local-first daemon that gives every coding agent (Claude Code, Codex, Antigravity, Cursor, desktop apps) one shared, verified project memory plus file and symbol locks. Agents reach it through MCP tools and optional hooks, so nobody re-explains the project each session.

Every capability is a swappable module behind a versioned interface. Hooks, agent adapters, storage, retrieval, verification and policies can be changed, disabled or replaced without touching the core.

| It is | It is not |
| --- | --- |
| A local Rust daemon with SQLite (WAL), an MCP server and a CLI | A cloud service or a hosted LLM |
| Memory of why: decisions, constraints and gotchas the code cannot show | A code indexer (codebase-memory-mcp or similar supplies the graph) |
| A referee: locks, trust tiers, verification, an audit ledger | An LLM that writes answers or judges code |
| Pull-based: agents query on demand | A boot-time prompt injector |
| Modular: every part has a port and a default plugin | A monolith with fixed hooks |

Working name: AutoMemory. Status: pre-build, research complete as of the date above.

## 2. Problem and evidence

Multi-agent work fails at three points: collisions, lost context, and untrusted memory. Figures below are as reported by the cited sources, and several are vendor or preprint claims (Sources, section 16).

| Problem | Reported evidence | Covered today? |
| --- | --- | --- |
| Agents collide in one repo | Cross-agent merge conflicts 41.7% vs 19.8% same-agent in a 33,596-PR study; worktrees stop same-directory writes, not merge conflicts | Partly: worktrees, dibs, lockpick, GitGuardex (file-level only) |
| A new agent lacks history | Each harness keeps its own memory format, so switching agents loses decisions | Many small tools (uac, Engrim, AgentMemory, memory-bridge); none verify |
| Context files do not help | Repo context files showed no gain or a drop in task success, and 20%+ more cost | Open: store only what the code cannot show |
| Memory goes stale | Stale memories get treated as equal to fresh ones without versioning | Open |
| Contradictions go unnoticed | MemConflict: conflict recognition at or below 0.25 for six memory systems (preprint) | Open |
| Memory poisoning | About 50% average attack success; selective repair worked 56.1% of the time | Open |
| Retrieval noise hurts | Free retrieval over distilled summaries cut task resolution to 22.22%; the right prior summary lifted it to 34.34% | Open: precision over volume |
| Destructive commands | Nine public data-deletion incidents in 14 months; users approved about 93% of permission prompts | Partly: built-in permissions, DIY hooks |
| Nobody proves memory helps | Vendor benchmarks disputed; one vendor's first suite showed memory made the agent worse | Open: eval harness |

## 3. Goals, non-goals and metrics

**Goals**

- G1: Any agent answers "what did we decide and why" from verified memory in one tool call.
- G2: Two or more agents work in one repo with zero silent overwrites.
- G3: Memory that is stale, contradicted or untrusted is flagged before an agent relies on it.
- G4: Every behaviour (hooks, adapters, stores, policies) is replaceable through a documented port.
- G5: Value is proven by an A/B eval on real repos before automation is added.

**Non-goals for v1**

- Hosted service, team sharing, zero-knowledge sync, dashboard UI.
- Our own code graph, or LLM calls inside the daemon.
- Replacing agents' native memory; we sit beside it.
- More than three agent adapters.

| Metric | How measured | Proposed target |
| --- | --- | --- |
| Collisions prevented | Lock conflicts caught per week; silent overwrites | 0 silent overwrites |
| Context tokens per session | Tokens spent re-explaining, with vs without | Below baseline |
| Task success A/B | Same tasks, memory on vs off | Not worse overall; better on non-inferable tasks |
| Stale-flag precision | Share of stale flags that were truly stale | At least 90% |
| Hook overhead | Pre-edit hook p95 latency | Under 5 ms |
| Install to first value | Time from install to first lock or recall | Under 5 minutes |

## 4. Users and use cases

The primary user is one developer running two or three agents on the same project, switching between terminal and desktop apps.

| Persona | Need | Phase |
| --- | --- | --- |
| Solo builder, 2-3 agents (primary) | No re-explaining, no collisions, trust in what agents remember | v1 |
| Developer with background or autonomous agents | Locks, guardrails, audit trail, handoffs | v1-v2 |
| Small team on a shared repo | Shared decisions, attribution, review queue | Later |

| Use case | Flow | Modules |
| --- | --- | --- |
| UC1 Switch agent mid-feature | New agent calls `memory.about` and `task.next`, gets decisions and the last handoff | Memory, Handoff |
| UC2 Parallel agents | Pre-edit hook acquires a symbol lease; conflict returns holder, goal and age | Locks, Hooks |
| UC3 "What auth method do we use?" | `memory.ask` returns ranked entries with verified, changed or stale status | Memory, Verify |
| UC4 Code refactored or commit reverted | Linked memories flip to stale or changed; agent is told | Verify, CodeIntel |
| UC5 Plan a risky feature | Planning Room runs one critique round and records the decision plus dissent | Orchestrator |
| UC6 Catastrophic command | Pre-tool guardrail blocks it; credentials outside scope are flagged | Guardrails |
| UC7 Swap a hook or adapter | User edits hook config or updates an adapter; no daemon restart | Plugin host |

## 5. Design principles

Each principle comes from the useful and useless findings in the research.

| Principle | Why |
| --- | --- |
| Pull, don't push: agents query tools; hooks add only short path-triggered nudges | Context files gave no gain and cost 20%+ more |
| Store only what the code cannot show | Memory converges with no-memory on tasks solvable from the repo |
| Verify before returning | Stale and conflicting memory is the unsolved failure |
| Provenance on every write; trust tiers decide what is injected as fact | Persistent memory is the poisoning surface |
| Facts come from the ledger (exit codes, diffs), prose from agents | Agents misreport their own results |
| Tests and builds are the judge, not agent votes | Debate gains vanish at equal compute |
| Everything is a port with a default plugin | Hook and agent APIs change often; adapter rot is the real cost |
| Degrade gracefully: hooks, then MCP only, then file watcher | Desktop apps and Cursor have weak hooks |
| Measure before building the next layer | Memory benefit is unproven on most benchmarks |
| Local first, daemon never calls an LLM | Offline, private, no second hallucination layer |

## 6. Architecture

One local daemon with five layers: agents call tools or fire hooks, adapters translate, a hook pipeline runs ordered handlers, and a small core hosts swappable modules and backends.

&#91;embedded content: architecture · 5 layers\]

All state sits in the core store. Modules and backends reach it only through ports, so replacing one never changes the others.

## 7. Modularity and extension model

The core is a small microkernel: ledger, store, event bus and plugin host. Everything else is a module behind a versioned port, shipped with a default implementation that users can replace, reorder, disable or update without a daemon restart.

### 7.1 Ports (extension points)

| Port | Job | Default | Example replacements |
| --- | --- | --- | --- |
| AgentAdapter | Install hooks and MCP config; map native events to canonical events | Claude Code | Codex, Antigravity, Cursor, any MCP client |
| HookHandler | One stage in the hook pipeline: allow, warn, block or annotate | Lock check, nudge, ledger logger | Custom policy script, org rules |
| CodeIntel | `symbol_exists`, `symbol_hash`, `callers` | codebase-memory-mcp | Own tree-sitter module, LSP, none (file hash fallback) |
| MemoryStore | Persist memories, decisions, ledger | SQLite WAL | Another SQLite layout, Postgres for teams |
| Retriever | Rank candidates for `ask` and `about` | FTS5 + tags + paths | Add vectors, add reranker |
| EmbeddingProvider | Turn text into vectors | None in v1, then local ONNX | API provider, other local model |
| Verifier | Check a claim against reality | Symbol and file-hash verifier | Test-result verifier, commit verifier |
| TrustPolicy | Map source to tier; decide what is returned as fact | Built-in tiers | Stricter org policy |
| GuardrailRule | Block catastrophic actions | Small deny list | AST shell lexer, credential scope checks |
| LockStrategy | How leases are granted | File and glob, advisory | Symbol-level, enforced, worktree-per-agent |
| Distiller | Extract candidates at session end | Off by default | Retrospective synthesizer |
| SyncBackend | Move op logs between devices | Folder or git remote | Other transports |
| Runner | Launch an agent headless | None in v1 | `claude -p`, `codex exec` |
| RoomProtocol | Format of a planning room | Propose, critique, revise | Review-only, vote, custom |
| Notifier | Tell the human it is their turn | Desktop toast | Slack, email, none |
| Exporter | Write pointers or mirrors | One-line AGENTS.md pointer | Markdown mirror of decisions |

### 7.2 Hook pipeline

Agents emit native events; the adapter converts them to canonical events; the pipeline runs an ordered chain of handlers. Handlers can be built-in Rust, an external executable (JSON in, JSON out) or a sandboxed WASM module.

| Canonical event | Typical handlers |
| --- | --- |
| SessionStart | Print pending items; register agent presence |
| UserPromptSubmit | Add a pending-work nudge only if something is assigned |
| PreToolUse | Lease check, guardrail, path-triggered decision nudge |
| PostToolUse | Ledger append, lease heartbeat |
| PreCommit | Symbol-overlap check against other agents' branches |
| PostCommit | Mark linked memories changed or stale |
| Stop | Release leases; block stop if an assigned review is owed; queue candidates |

Each handler declares: priority (order), a latency budget, a failure mode (fail open or fail closed) and the events it subscribes to. Hot-path handlers must meet the budget or run async.

### 7.3 Changing and updating hooks and plugins

| Task | How | Restart? |
| --- | --- | --- |
| Enable, disable, reorder a hook | `automemory hooks enable\|disable\|move <id>` or edit `hooks.toml` | No: hot reload |
| Add a custom hook | `automemory hooks add --event PreToolUse --exec ./check.sh` | No |
| Test a hook safely | Shadow mode: log what it would have done, never enforce | No |
| Dry-run a policy change | `automemory hooks test --event <e> --input sample.json` | No |
| Update an agent adapter | `automemory adapter update <agent>`: new event mappings from the adapter manifest | No |
| Install or update a plugin | `automemory plugin install\|update\|pin <name>` | No |
| Roll back | `automemory plugin rollback <name>`; versions are kept | No |
| Change a profile | Observe, Assist or Autopilot; each is a named set of enabled modules | No |

Config is layered: built-in defaults, then global (`~/.automemory/config.toml`), then project (`.automemory/config.toml`), then local override. Later layers win.

### 7.4 Plugin manifest and contract

```toml
[plugin]
name = "symbol-verifier"
version = "0.3.1"
api_version = "1"          # which port contract it implements
ports = ["Verifier"]

[capabilities]               # least privilege, shown to the user on install
read_ledger = true
write_memory = false
block_tools = false
network = false

[hooks]
subscribe = ["PostCommit"]
priority = 50
latency_budget_ms = 20
on_failure = "open"
```

| Rule | Detail |
| --- | --- |
| Versioned ports | Port contracts carry `api_version`; the host refuses mismatches instead of misbehaving |
| Capability permissions | A plugin gets only the capabilities it declares and the user approved |
| Namespaced storage | Plugin tables and memory rows are namespaced; plugin writes carry the plugin's own trust tier |
| Sandboxing | External plugins run out of process or as WASM; a crash never takes the daemon down |
| Schema migrations | Core uses `PRAGMA user_version`; plugins ship their own migrations |
| Contract tests | Every adapter and port ships tests run against each supported agent version |
| Degrade ladder | Hooks, then MCP only, then file-watcher fallback, per agent surface |

## 8. Feature catalog

P0 = needed to prove the core loop, P1 = next, P2 = only if the eval or users ask. Evidence says how well the research supports it.

| Module | Feature | Priority | Evidence |
| --- | --- | --- | --- |
| Core | Daemon with SQLite WAL, event bus, plugin host | P0 | Required |
| Core | Append-only ledger with agent, session and time attribution | P0 | Observability named as top leverage in multi-agent failures |
| Core | Layered config, profiles (Observe, Assist, Autopilot) | P0 | Design choice |
| Locks | File and glob leases with TTL, heartbeat, human override | P0 | Collisions measured at 41.7% cross-agent |
| Locks | Symbol-level leases via CodeIntel | P1 | Closes the merge-time gap worktrees leave |
| Locks | Pre-merge symbol-overlap warning | P1 | Same |
| Locks | Optional worktree per agent; lock state in git common dir | P1 | dibs pattern |
| Memory | `ask`, `about`, `get`, `topics`, `propose` over FTS5, tags, paths | P0 | Pull model supported by context-file study |
| Memory | Non-inferable only: decisions, constraints, gotchas | P0 | Memory helps only where the repo is silent |
| Memory | Candidate tier; promote on verification or human confirm | P0 | Poisoning and noise defense |
| Memory | Vector search (sqlite-vec) with hybrid ranking | P1 | Conditional on eval |
| Memory | Decay and supersede chains | P1 | Stale-memory research |
| Decisions | `decision.record`, `decision.get`, dissent kept | P0 | Core value |
| Verify | Symbol and file-hash verify on recall: verified, changed, stale, unverifiable | P0 | Open gap in market |
| Verify | Commit-bound staleness: revert or rewrite flags dependent memory | P1 | Open problem in 2026 survey |
| Verify | Contradiction detection across active memories | P1 | MemConflict at or below 0.25 |
| Trust | Trust tiers, provenance required, revoke by session or agent | P0 | Poisoning at about 50% attack success |
| Trust | Quote retrieved text as data, never as instructions | P0 | Same |
| Code intel | `CodeIntel` bridge to codebase-memory-mcp | P0 | Graph cuts tokens, grep still wins exact strings |
| Hooks | Pipeline, hot reload, shadow mode, custom handlers | P0 | Hook APIs change often |
| Hooks | Path-triggered one-line nudges | P1 | Only if tool-call rate is low |
| Adapters | Claude Code first, Codex second | P0 / P1 | Mature hooks |
| Adapters | Antigravity, Cursor (advisory plus file watcher) | P2 | Weaker or newer hooks |
| Handoff | Baton packet under 400 tokens, built from the ledger | P1 | Avoids context flooding |
| Guardrails | Catastrophic-only deny list; credential exposure check | P1 | Nine incidents; permission fatigue |
| Guardrails | AST shell lexer | P2 | Bypassable arms race |
| Orchestrator | Task graph, merge queue, cross-review with tests as judge | P1 | MAST: weak verification is a top failure |
| Orchestrator | Headless runner for CLI agents | P2 | After eval |
| Planning Room | Evidence-required propose, critique, revise; human tiebreak | P2 | Debate gains unproven at equal compute |
| Sync | Encrypted op-log sync of memories and decisions | P2 | Needed only for multiple devices |
| Interfaces | MCP server, CLI, review queue, digest | P0 / P1 | Core |
| Eval | Harness: memory on vs off, solo vs room | P0 | Nobody can currently prove value |

## 9. Data model

One global SQLite file (`~/.automemory/global.db`), rows keyed by `project_id` (hash of git remote URL, fallback root commit). Row IDs are UUIDv7; timestamps are hybrid logical clocks; deletes are tombstones so sync never resurrects them.

| Entity | Key fields | Notes |
| --- | --- | --- |
| Memory | id, project\_id, topic\[\], paths\[\], symbols\[\], claim, source, trust\_tier, status, confidence, last\_verified, supersedes, embed\_model\_id | Status: candidate, active, stale, changed, superseded, pinned |
| Decision | id, topic, decision, reason, alternatives\[\], dissent, agent, symbols\[\], evidence\[\], status | Never deleted; superseded by a newer decision |
| Ledger event | id, ts, agent, session, tool, input\_hash, exit\_code, files\[\], duration | Append-only; summaries sync, raw rows optional |
| Lease | id, agent, session, scope (file, glob, symbol), goal, ttl, heartbeat | Local only; stored in the git common dir |
| Task | id, room\_id, owner, symbols\[\], spec, state | Created from a decided plan |
| Room | id, state, turn\_holder, round, budget, posts\[\] | Planning Room state machine |
| Post | id, room\_id, author, kind, body, evidence\[\] | Evidence IDs validated on write |
| Baton | id, from, to, changes, files, achievements, blockers, next\_step | Generated from the ledger |
| Plugin | name, version, api\_version, capabilities, enabled | Namespaced tables per plugin |

| Trust tier | Source | Returned as |
| --- | --- | --- |
| 1 Pinned | Human pin | Fact; constitution |
| 2 Verified | Test or exit-code fact, or symbol check passed | Fact with evidence |
| 3 User statement | Said by the user in a session | Fact with source |
| 4 Agent claim | An agent wrote it, unverified | Candidate; not returned by default |
| 5 Untrusted origin | Derived from web, fetched files or other agents' posts | Quarantined until verified |

Synced: memories, decisions, constitution. Never synced: leases, FTS index, vectors (re-embedded per device), code graph.

## 10. Interfaces

MCP carries the whole protocol, so desktop apps with no hooks still work. Hooks and the CLI are upgrades.

### 10.1 MCP tools

| Tool | Purpose |
| --- | --- |
| `memory.ask(question, scope?)` | Ranked short entries with ids, status and provenance |
| `memory.about(path_or_symbol)` | Everything known about a file or function, plus callers' memories |
| `memory.get(id)` | Full detail and evidence |
| `memory.topics()` | What is queryable |
| `memory.propose(claim, evidence)` | Write a candidate; rejected without provenance |
| `decision.record` / `decision.get(topic)` | Record or fetch a decision with history and dissent |
| `lock.acquire(scope, goal)` / `lock.status` / `lock.release` | Leases |
| `task.next` / `task.submit` | Claim work, hand it to review |
| `review.next` / `review.post` | Cross-review a diff |
| `room.pending` / `room.join` / `room.post` / `room.status` | Planning Room turns |
| `baton.pass` / `baton.pickup` | Handoff between agents |

### 10.2 CLI

| Command | Use |
| --- | --- |
| `automemory doctor` | Health check of daemon, adapters, hooks, plugins |
| `automemory review` | Queue of candidates to confirm or reject |
| `automemory pin\|veto\|revoke <id>` | Human control; revoke by `--session` or `--agent` |
| `automemory locks` / `unlock <scope>` | See or override leases |
| `automemory hooks ...` / `adapter ...` / `plugin ...` | Section 7.3 |
| `automemory digest --span 1h` | Activity rollup |
| `automemory eval run` | Memory on vs off comparison |
| `automemory stop` | Kill switch for any autonomous run |

### 10.3 Hook contract

Handlers receive one JSON event and return one decision. The same shape is used by built-in, executable and WASM handlers.

```json
{
  "event": "PreToolUse",
  "agent": "claude-code",
  "session": "s-81",
  "tool": "edit",
  "paths": ["src/auth/session.ts"],
  "symbols": ["auth.session.createSession"]
}
```

```json
{
  "decision": "warn",
  "message": "Decision D-12 applies to src/auth. Call decision.get(\"auth\").",
  "lease": "granted"
}
```

### 10.4 Every response ends with the next step

Agents skip optional tools, so each tool reply tells the agent exactly what to do next. Posts from other agents are returned as quoted data with author and trust tier; only the daemon-written `next_action` line is an instruction.

```text
room R-12 [PLAN] turn: you (critic)
proposal P-1 by claude-code: "JWT in cookie, refresh rotation"
facts: D-12 (auth=JWT cookie, verified), callers(createSession)=7
next_action: room.post(R-12, "critique", ..., evidence=[symbol|decision|test])
```

## 11. Orchestration and Planning Room

Orchestration is optional and gated: the Planning Room is off by default and ships only if its own A/B test beats a solo agent with tests (phase 6).

&#91;embedded content: room states · 1 decision point\]

A room advances only when the current turn holder posts, so desktop apps that run only when prompted still work. The daemon rejects any post whose evidence IDs do not resolve. Each tool reply ends with a `next_action`, so one call always leads to the next. Stored rationale is 3 to 5 lines, never raw reasoning.

### 11.1 Automated pipeline

| Stage | Automated action | Human gate (configurable) |
| --- | --- | --- |
| Intake | Feature request becomes a spec | Approve spec |
| Plan | Planning Room runs, or a single agent plans | Approve plan, or auto-approve when agreed and low risk |
| Decompose | Plan splits into tasks with owners and symbol scopes |  |
| Lock | Symbol leases granted per task |  |
| Build | Agents implement, in separate worktrees where possible |  |
| Verify | Tests and build run; results come from exit codes |  |
| Cross-review | A different agent reviews each diff |  |
| Merge | Merge queue, one branch at a time, symbol-overlap check first | Approve merge on risky paths |
| Learn | Decisions and lessons saved as candidates with trust tiers | Confirm candidates |

### 11.2 Autonomy profiles

| Profile | Behaviour |
| --- | --- |
| Observe | Records and warns only |
| Assist (default) | Plans and coordinates; a human approves each stage |
| Autopilot (opt-in per repo) | Runs the whole pipeline and stops only at risk triggers |

### 11.3 Always stop for a human

- Schema or migration changes, auth or secrets, file deletion
- Edits outside the agreed symbols
- The same test failing three times
- Token or time budget exceeded
- Unresolved disagreement after the round cap

### 11.4 How each agent surface joins

| Surface | MCP | Hooks | Joins by |
| --- | --- | --- | --- |
| Claude Code CLI | Yes | Full set | Hooks, optional headless launch |
| Codex CLI | Yes | Reported; verify per version | Hooks or headless launch |
| Antigravity | Yes | JSON hooks at several lifecycle points | Hooks plus manager surface |
| Cursor | Yes | Limited | MCP plus nudge file or file watcher |
| Claude desktop app | Yes | None known; not verified | Standing instruction to call `room.pending()`, an MCP prompt slash command, or a desktop toast to the human |

## 12. Trust, security and privacy

Persistent memory is an attack surface, so every write is checked and every memory can be traced and revoked.

| Threat | Defense |
| --- | --- |
| Poisoned memory from web, files or other agents | Trust tiers; untrusted origin is quarantined and never returned as fact |
| Fake evidence in a post or memory | Every evidence ID must resolve before the write is accepted |
| Prompt injection through retrieved text | Retrieved entries are quoted data; only `next_action` is an instruction |
| Poisoned memory discovered late | Revoke by session or agent; derived memories are flagged with it |
| Stale memory after refactor or revert | Symbol-based verify; commit-bound staleness |
| Runaway autonomous agents | Round caps, token budgets, wall-clock timeouts, `automemory stop` |
| Destructive commands | Catastrophic-only deny list in shadow mode first; credential exposure check; scope tokens and sandbox rather than a regex arms race |
| Hook or plugin abuse | Capability permissions, sandboxing, signed plugins optional, shadow mode before enforcing |
| Secrets in the ledger | Redact before write; raw ledger is local only by default |
| Sync leaks | Sync memories and decisions only, as an encrypted op log; no code or secrets in v1 |

Privacy: all data stays local by default. The daemon makes no network calls and no LLM calls. Any embedding provider or sync backend is an explicit opt-in plugin.

## 13. Evaluation plan

The eval harness ships in phase 2, before any automation, because memory benefit is unproven on most public benchmarks and one vendor's own first suite showed memory made the agent worse.

| Design choice | Detail |
| --- | --- |
| Tasks | Real issues from the user's own repos (Rust, Swift, Kotlin, TypeScript), including tasks whose deciding context is not in the code |
| Arms | Memory off, memory on, memory on with verify; later solo agent vs cross-review vs Planning Room |
| Runs | At least 3 runs per task per arm; paired comparison |
| Scoring | Tests pass from exit codes, not agent claims |
| Cost | Tokens, tool calls, wall time per task |
| Hygiene | Stale and contradictory memory injected on purpose to test flags |
| Security | Poisoned memory planted to test quarantine and revoke |

| Metric | Question it answers |
| --- | --- |
| Task success rate delta | Does memory help? |
| Corrections per task | Does the user repeat themselves less? |
| Tokens per task | Does it cost less or more? |
| Stale-flag precision and recall | Can the verifier be trusted? |
| Retrieval precision at 3 | Is `ask` returning the right entries? |
| Collisions and conflicts | Do locks and overlap checks work? |
| Hook p95 latency and false-block rate | Is it fast and unobtrusive? |

## 14. Phased roadmap

Every phase ends with a measurement and a gate. Later phases start only if the gate passes.

| Phase | Ship | Exit gate |
| --- | --- | --- |
| 1 | Daemon, ledger, plugin host, hook pipeline, Claude Code adapter, file locks | Zero silent overwrites in a two-agent test; hook p95 under 5 ms |
| 2 | Eval harness | Baseline numbers on at least 20 real tasks |
| 3 | Memory store with trust tiers, provenance, `ask`/`about`/`propose`; decision log; CodeIntel bridge; verify and stale flags | Memory-on not worse than memory-off; stale-flag precision at least 90% |
| 4 | Commit-bound staleness, contradiction detection, catastrophic guardrail in shadow mode, symbol leases, pre-merge overlap check | Fewer stale hits and fewer merge conflicts than phase 3 |
| 5 | Cross-review with tests as judge, baton handoff, Codex adapter | Task success at or above solo agent; cost increase acceptable |
| 6 | Planning Room (two agents, one round), only for high-stakes design choices | Beats solo plus tests on a defined task set, else dropped |
| 7 | Orchestrator with headless runner, Autopilot profile, vectors, sync, more adapters | Each justified by measured need |

Phases 1 to 3 form the minimum credible product: coordination you can feel, memory you can trust, proof it helps.

## 15. Risks and mitigations

| Risk | Mitigation |
| --- | --- |
| dibs and similar tools already cover locks and handoff | Evaluate dibs; adopt or extend rather than rebuild; differentiate on verified memory and symbol-level leases |
| Vendors absorb the idea (Agent Teams, native memory) | Stay cross-vendor and neutral; the verifier and trust layer are the moat |
| Hook APIs change often | Thin adapters, adapter manifests, contract tests per agent version, degrade ladder |
| Agents ignore MCP tools | Standing one-line pointer, path-triggered nudges, `next_action` on every reply, Stop hook for owed reviews |
| Claude desktop and Cursor lack hooks | MCP-only protocol, prompt-triggered joins, file-watcher fallback; hook support unverified for desktop |
| Memory does not help in the eval | Cut scope to locks, decisions and verification; keep the plugin host |
| Debate adds cost without gain | Planning Room stays optional and gated by its own A/B |
| Guardrail false positives | Shadow mode first; measure false-block rate; fail open for non-catastrophic rules |
| Plugin ecosystem adds attack surface | Capability permissions, sandboxing, pinned versions, optional signing |
| Third-party schema drift (codebase-memory-mcp) | `CodeIntel` trait, pinned version, file-hash fallback |
| Headless or automated use may be restricted by vendor terms | Check each vendor's terms before shipping the Runner; not yet verified |
| Scope creep | Phase gates; cut list in section 16 |

## 16. Open decisions and appendix

### 16.1 Open decisions

| Decision | Options | Current lean |
| --- | --- | --- |
| Relation to AgentMesh | One core with AgentMesh as the coordination layer, shared crate, or separate | One core |
| Locks: build or adopt | Build, or adopt and extend dibs | Evaluate dibs for a week first |
| Main pitch | Safety, memory, or multi-agent glue | Memory you can trust, plus no collisions |
| First two adapters | Claude Code, Codex, Antigravity, Cursor | Claude Code, then Codex |
| Vectors | v1 or after the eval | After the eval, for memory text only |
| Default autonomy | Observe, Assist, Autopilot | Assist; Autopilot opt-in per repo |
| Plugin runtime | Executables only, or also WASM | Executables first, WASM later |
| Claude desktop trigger | Standing instruction, MCP prompt slash command, toast | Standing instruction plus slash command |
| Sync backend | Folder, git remote, other | Folder or git remote of encrypted ops |

### 16.2 Cut list (v1)

Boot-time injection, auto-distill of every session, auto-generated AGENTS.md, own AST graph, shell-regex guardrail arms race, zero-knowledge BIP-39 sync, storing raw agent reasoning, agent-edited memory, debate as default, dashboard, team sharing, and more than three adapters.

### 16.3 Competitor landscape

| Tool | Slice | Overlap |
| --- | --- | --- |
| [dibs](https://pkg.go.dev/github.com/polymatx/dibs) | File claims with expiry, enforcement hooks, presence, handoff notes | Closest: locks and handoff |
| [lockpick](https://github.com/simke9445/lockpick) | Advisory locks plus git-index lock | Locks |
| [GitGuardex](https://github.com/recodeee/gitguardex) | Worktrees, file lock claims, PR-only merges | Locks |
| [Entire CLI](https://pkg.go.dev/github.com/entireio/cli@v0.5.2) | Sessions indexed with commits | Ledger |
| [unified-agent-context](https://pypi.org/project/unified-agent-context/) | Shared memory and skills for Claude Code and Codex | Memory with attribution |
| [Observational Memory](https://pypi.org/project/observational-memory/0.4.0/) | Shared local markdown memory | Memory |
| [Engrim](https://pypi.org/project/engrim/1.4.8/) | Local SQLite memory across harnesses | Memory |
| [codebase-memory-mcp](https://dev.co/ai/mcp/codebase-memory-mcp) | Tree-sitter code knowledge graph over MCP | Our CodeIntel default |
| Mem0, Zep, Letta | App-oriented memory platforms | Generic memory |

### 16.4 Sources

These came from search results and were not fetched in full; treat figures as reported claims, several from vendors or preprints.

- [When Agents Collide: 33,596 pull requests](https://codex.danielvaughan.com/2026/07/28/agent-pr-merge-conflicts-concurrent-coding-agents-codex-cli-worktree-isolation-coordination-defence/)
- [Evaluating AGENTS.md (ICLR 2026)](https://iclr.cc/virtual/2026/10021142)
- [Hindsight 0.9.0 memory benchmark notes](https://hindsight.vectorize.io/blog/2026/08/06/hindsight-0-9-0)
- [SWE-ContextBench discussion](https://dev.to/feed/memorax-ai)
- [Always-On Agents survey](https://arxiv.org/pdf/2606.30306)
- [Memory poisoning study (ICML 2026)](https://icml.cc/virtual/2026/67964) and [MemSecBench](https://promptfoo.dev/lm-security-db/vuln/persistent-agent-memory-poisoning-and-incomplete-repair-718886f2/)
- [MemConflict discussion](https://takase.com/library/development-notes/the-agent-that-reads-its-own-diary)
- [Why do multi-agent LLM systems fail? (MAST)](https://iclr.cc/virtual/2025/33314)
- [Single vs multi-agent systems](https://medium.com/@mjgmario/single-agent-vs-multi-agent-systems-when-coordination-helps-hurts-and-pays-off-57735ee7916d)
- [Code graph vs grep](https://graphify.com/blog/code-knowledge-graph-vs-grep) and [awgraph](https://pypi.org/project/awgraph/)
- [Nine coding-agent data-deletion incidents](https://adversa.ai/blog/ai-coding-agent-incidents/) and [permission fatigue](https://scalex.dev/blog/ai-agent-permissions/)
- [AGENTS.md as an open standard](https://codex.danielvaughan.com/2026/04/07/agents-md-open-standard-cross-tool-portability)
