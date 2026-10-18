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

**Status (Oct 2026):** layers 1, 2, 3, 4 and 6 and the coverage metric are built (`tests/scenarios/locking_v2.py`); layer 5 (worktree launcher) is deliberately not planned. Measure the result with `automemory locks --coverage` after a week of real use; the target is ≥ 80% covered or reported.

**Order and gates**

| Step | Delivers | Verified by |
|---|---|---|
| 1 (layer 1 + 6) | Multi-turn leases, idle expiry, policy | `tests/scenarios/collision.py` gains a multi-turn case; unit tests for idle expiry |
| 2 (layers 2 + 3) | Shell writes and unknown writes covered | New guardrail-style table of shell commands with expected write targets; scenario where agent B's `sed -i` on A's file is reported |
| 3 (layer 4) | Alerts for agents without hooks | Dashboard scenario; manual check with Cursor or Cline |
| 4 (layer 5) | Optional hard isolation | End-to-end: two launched agents, no collision possible, overlap report at merge |

Decide before step 2: whether reconciliation may *deny* the next call of the offending session in Strict mode, or only warn. Decide before step 4: ship the launcher inside v1, or leave it to Phase 7 with the headless Runner.

## 7. Retrieval modes and swappable embeddings

Two modes, one switch, so vectors are never required and never loaded unless asked for:

```toml
[retrieval]
mode = "keyword"            # default. "hybrid" adds vector similarity
[embeddings]                # read only in hybrid mode
provider = "ngram"          # ngram | ollama | onnx | openai | http
model = "nomic-embed-text"
endpoint = ""               # ollama / openai-compatible base URL
api_key_env = ""            # NAME of an environment variable or keychain item, never the key itself
allow_network = false       # hosted providers need this set to true, per project if wanted
fallback = "keyword"        # any provider failure silently degrades to keyword recall
```

* **keyword mode (default):** FTS5 (BM25) + path/tag links + trust tier + recency/usage decay + diversity + token budget. No model, no network, deterministic, same speed as today (about 0.6 ms).
* **hybrid mode:** the same candidates plus vector similarity, merged with reciprocal rank fusion so a bad embedding cannot override an exact keyword or path match.

**Providers (all behind the existing `EmbeddingProvider` port, extended with `id`, `dimensions`, `embed_batch`, `local: bool`):**

| Provider | Cost | Runs | Notes |
|---|---|---|---|
| `ngram` | free | in process, offline | Today's character n-gram projection; fuzzy spelling, no semantics. Default in hybrid until another is configured |
| `ollama` | free | local server (`localhost:11434`) | Free open models (e.g. nomic-embed-text 768-d, bge, mxbai); no runtime bundled in our binary |
| `onnx` | free | in process | Optional cargo feature `embeddings-onnx`; small 384-d model (MiniLM / bge-small class) downloaded once to `~/.automemory/models/` with a checksum and an explicit prompt; keeps the default binary at about 10 MB |
| `openai` | paid | network | Any OpenAI-compatible endpoint (OpenAI, Voyage, Gemini-compat, LM Studio, LiteLLM, Azure) via `endpoint` + `model` + `api_key_env`; text is secret-redacted before it leaves the machine |
| `http` | any | external command or URL | JSON in, vector out, like exec hook handlers; the escape hatch for anything else |

**Swapping safely:** vectors are stored per `(memory_id, model_id)`; a query only ever compares vectors of the active
model. Changing provider or model starts a resumable background re-embed in `serve` (rate-limited);
until it finishes, hybrid recall uses what is embedded plus keyword. Commands: `automemory embeddings status | test |
reindex | eval`. `eval` runs a labelled query set (from `tests/eval`) per configured provider and prints
precision@3, MRR and latency, so a provider is adopted on evidence.

**Where embeddings may run:** never in hooks (separate processes; loading a model per call costs tens to hundreds of
ms). Path-triggered recall in hooks stays keyword/links. Query embedding happens in the long-lived MCP server
process (`automemory mcp` / `serve`) for `memory.ask|recall|about`; document embedding happens in `serve`.

**Search:** brute-force cosine in Rust is enough to about 50k memories at 384 dimensions (single-digit ms); `sqlite-vec` is
an optional later index. No vector database server.

**When to turn hybrid on:** only if `embeddings eval` beats keyword on labelled queries, or the store passes about
2-5k active memories. Evidence from other systems is mixed (docs/research/memory_improvement.md).

**Status (Oct 2026):** built and tested (`embeddings` module, CLI, `tests/scenarios/embeddings_modes.py`). Not built: `onnx` provider, vector sync between machines, background reindex in `serve`.

## 8. Same experience in another session or on another machine

"Experience" means three things: what the project **knows** (memories, decisions, failures, skills), what is **in
flight** (current task, plan, baton, branch, uncommitted work), and the **agent setup** (hooks, MCP).

**Gaps today**
1. A project's id is a random UUID per machine and `Project` carries no git remote, so the same repository on a second PC is a
   different project: synced memories carry the first PC's `project_id` and do not attach.
2. Only the daemon pass uses the conflict-safe op-log; `sync import` is last-writer-wins. Nothing pulls at session start or pushes at session end.
3. A new session starts cold: no summary of the last session, open task or baton unless the agent asks.
4. Batons, checkpoints and decisions are not part of the delta sync kinds (memory, trace, failure, skill, decision only).
5. No bootstrap for a new machine.

**Design**
1. **Stable project identity.** `project_key` = normalized git remote URL (fallback: root commit hash), stored on
   `projects`. Local `project_id` stays internal; delta records carry `project_key` and are mapped to the local id on
   apply (created on first sight). Per-device path mapping already exists (`project_paths`).
2. **Resume pack (same PC, new session).** At the first prompt or SessionStart: at most about 400 tokens: last session's
   checkpoint and outcome, open task and next step (from the last baton), decisions touching the files in `git status`,
   files other agents hold. Also `automemory resume` and MCP `memory.resume`. Pull-based; shown once per session.
3. **Automatic sync, never blocking.** SessionStart: background pull with a 2 s budget, then deltas apply; SessionEnd/Stop:
   push (debounced); daemon: periodic. Op-log (`sync::deltas`) becomes the only path; the legacy snapshot is export-only.
   Everything is encrypted on the client (BIP-39 key) before it leaves; records keep provenance (device id, trust tier).
4. **Hand-off bundle.** At SessionEnd (or `automemory handoff`) write a baton with branch, HEAD sha, changed-file list and
   stat, current task/plan, failing tests, next step. It syncs as a record kind. On the other PC `resume` says e.g.
   "last on <device>, 14:02, claude: branch feat/x at abc123, 3 files uncommitted (not synced: commit and push, or stash)";
   if the sha is missing locally it suggests `git fetch`. Code itself travels by git, never by AutoMemory.
5. **Add kinds to the op-log:** baton, session checkpoint, review proposals (so a conflict raised on one PC is visible on
   both), tracked docs (the git-ignored plan vault already syncs).
6. **New machine in one command.** `automemory sync bootstrap`: pair with the recovery phrase, pull, `connect` the agents
   (idempotent), `doctor`.
7. **Vectors are not synced** by default (device re-embeds with its own provider); optional later: sync vectors when both devices use the same model id.

**Acceptance test (`tests/scenarios/two_machines.py`):** two sandbox HOMEs, repository checked out at different paths with the
same remote, one shared bare git vault. Machine A saves a decision, ends a session with a checkpoint and a baton, syncs.
Machine B starts a new session: `resume` shows A's task and next step; `memory.ask` returns A's decision with its verified
status; no duplicates after two round trips; an edit on both machines becomes one review proposal; B's vault pull never blocks a hook beyond 2 s.

**Order**

| Step | Delivers | Why first |
|---|---|---|
| 1 | `project_key`, delta mapping, op-log only, pull on start / push on end, two-machine test | nothing else works across PCs without it |
| 2 | Resume pack, hand-off bundle, new record kinds, `sync bootstrap` | the visible "same experience" |
| 3 | `[retrieval]` modes, extended `EmbeddingProvider`, per-model vectors, hybrid fusion, `embeddings status|test|reindex|eval`, providers `ngram` + `ollama` + `openai` | choice of vector or not, evidence-based |
| 4 | `onnx` feature, `http` provider, optional vector sync | heavier or niche |

Decisions needed: sync transport beyond a git remote (folder like Dropbox, or the Cloudflare gateway, later); whether hybrid stays
opt-in per project; which free model to recommend by default after `embeddings eval`.


## 9. One project brain for every agent and IDE

Three delivery tiers, one source of truth: (A) hooks where the vendor documents them (Claude, Codex, Antigravity done); (B) a managed block in the instruction file every agent already reads (built: `export-rules`); (C) MCP plus a skill (MCP everywhere; skills partly). Research (Oct 2026, vendor and community sources): AGENTS.md is read by 20+ tools but is static; MCP standardizes transport, not memory format; nothing verifies or merges what different agents write. Our differentiators: trust tiers, conflict-safe sync across machines, locks.

**Status:** tier B and human input (`note`, dashboard add) built and tested. Not built: tier A hook adapters beyond the first three, a "Project brain" dashboard page, adapter-trait cleanup (see TASKS).
