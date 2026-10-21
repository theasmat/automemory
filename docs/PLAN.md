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
| 6 | Planning Room (two agents, one round) for high-stakes design only; Nuxt dashboard redesign; First-run onboarding tour (CLI + Web UI) | Beats solo + tests on a defined task set; Onboarding green in tests | Built; Onboarding complete |
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

## 10. Configurable embeddings and secure key storage

Today: one provider is set through flat `[embeddings]` fields (`automemory embeddings use ...`), the API key can only come from an
environment variable (`api_key_env`), and the dashboard's **Recall settings** page saves to the browser's `localStorage` only (it changes
nothing in the daemon). Goal: choose and add models freely, keep cloud keys in the OS keychain, and see indexing happen.

### 10.1 Provider profiles and a model catalog
- `[embeddings] active = "<profile>"` and `[embeddings.profiles.<name>]` with `provider`, `model`, `endpoint`, `dimensions`, `key` (a reference such as
  `keychain:openai`, never the secret), `allow_network`, `batch_size`, `timeout_secs`. The old flat fields migrate into a profile named `default`.
- Several profiles can exist (for example `local` = Ollama and `cloud` = OpenAI); switching is one setting. Vectors are already stored per model id, so switching
  never mixes vector spaces.
- A built-in **preset catalog** (static table, with a "verify before release" note because vendor catalogs change): Ollama `nomic-embed-text` (768),
  `mxbai-embed-large` (1024), `bge-m3` (1024), `all-minilm` (384); OpenAI `text-embedding-3-small` (1536) and `-large` (3072, shortenable);
  Gemini (OpenAI-compatible endpoint), Mistral, Jina, Together, LM Studio, vLLM, LiteLLM through the same OpenAI-compatible adapter; any command (`exec`).
  Free-text model ids are always allowed. `automemory embeddings models` lists presets plus what a local Ollama has installed (`/api/tags`).
- Later adapters for providers whose request shape differs (Cohere, Voyage, Gemini native). ONNX stays a cargo feature (section 7).

### 10.2 Secrets in the OS keychain
- New `automemory-core/src/secrets.rs`: `get(name)`, `set(name, value)`, `delete(name)`, `status(name)` (where it comes from: env, keychain or none).
  Lookup order: the profile's env var, then the keychain, then nothing. Service `dev.automemory.secret`, account = secret name.
- Backends through the `keyring` crate (macOS Keychain, Windows Credential Manager, Linux Secret Service): one API, and the secret never appears on a command line.
  Where no keychain exists (headless Linux), an encrypted file `~/.automemory/secrets.enc` (mode 0600, sealed with the existing master key) with a visible warning.
- The existing master key code shells out to `security add-generic-password -w <key>`, which exposes the key in the process list for a moment; move it to the same backend.
- The secret is never written to `config.toml`, the database, the audit log, logs, `doctor` output, hand-offs, instruction files or the sync vault (each machine sets its own),
  and error messages are redacted. Hooks never read it (they are keyword only). Redaction is covered by a test that greps all outputs for a planted key.
- CLI: `automemory embeddings key set <profile>` (hidden prompt, or `--stdin`, or `--from-env`), `key status`, `key remove`. Dashboard: a write-only field (the key is never
  shown back), set/remove behind the human password (section on the password gate), and a "Saved in Keychain" state.

### 10.3 Recall settings page becomes real
- `GET/PUT /api/recall` read and write `[retrieval]` and the profiles (writes behind the human password); the `localStorage` save is removed.
- Layout as steps: 1 Keyword or Hybrid. 2 Where: this computer (Ollama, free), a cloud service, your own endpoint, or a command. 3 Model: dropdown from the catalog
  plus "other", with dimensions. 4 API key (cloud only) stored in the keychain. 5 **Test** (embeds one sentence, shows dimensions, latency, whether text leaves the
  machine) and **Index**.
- A privacy line states exactly what is sent (redacted summary and the first 1500 characters of content) and needs `allow_network` for any non-local endpoint.
  Optional per-project switch: `private` projects never use a cloud profile.

### 10.4 Indexing you can see and control
- A background worker in `serve` (queue, batch 16 to 64, rate-limit and retry with backoff, resume after restart, cancel) fills missing vectors; progress over the
  WebSocket: "412 of 1,300 memories embedded". `embeddings status --watch` shows the same in the terminal.
- Guards for paid services: an estimate before a large first index (tokens times a stored price table, labelled "estimate"), a daily token cap, and a confirmation above a set size.
- Dimension or model mismatches fail with a clear message; "Remove unused vectors" deletes vectors of models that are no longer active.
- A one-click "Compare keyword and hybrid on my notes" check (queries built from note titles) next to the existing `embeddings eval` file mode.

### 10.5 Phases and acceptance
| Phase | Scope | Done when |
|---|---|---|
| E1 | `secrets.rs` + keychain/encrypted-file backends, `embeddings key set|status|remove`, master key off the command line | A planted key never appears in any output, file or log; works with a fake backend in tests (`AUTOMEMORY_SECRETS_BACKEND=file`) |
| E2 | Profiles, presets, `embeddings profile add|list|use|remove`, `embeddings models`, migration of the flat fields | Two profiles can be switched and tested from the CLI; old configs still work |
| E3 | `/api/recall`, real Recall page with the five steps, key field, Test button | The page changes the daemon's behaviour and survives a restart; a wrong key shows a clear error |
| E4 | Indexing worker, progress, cost guard, unused-vector cleanup | A 1,000-memory index runs in the background with progress and can be cancelled and resumed |
| E5 | Native adapters (Cohere, Voyage, Gemini), ONNX feature | Optional |

Tests: mock embedding server (exists) with an `Authorization` check, key redaction grep, keychain fake, scenario `tests/scenarios/embeddings_modes.py` extended for profiles
and keys, dashboard scenario for `/api/recall` (human gate, key never returned).

**Status (Oct 2026):** E1 to E4 built and tested (`secrets.rs`, profiles and presets, `/api/recall`, Recall page, indexing with progress; scenarios `embeddings_modes.py` and `dashboard.py`). E5 not started.

**Decisions (settled Oct 2026)**
1. Keys use the **`keyring` crate** (native Keychain / Credential Manager / Secret Service), not per-OS shell shims; the encrypted file is only the fallback where no keychain exists.
2. First cloud support is the **OpenAI-compatible adapter** (OpenAI, Gemini's compatible endpoint, Mistral, Jina, Together, LM Studio, vLLM, LiteLLM). Native Cohere / Voyage / Gemini
   adapters wait for E5.
3. **Provider choices are not synced** between machines; each machine picks its own profile. **Keys are never synced.**
4. The master key moves to the same `keyring` backend in E1, so no secret is ever passed on a command line.
5. Cloud use is **off by default**: a non-local endpoint needs `allow_network`, and a `private` project can forbid cloud profiles.

## 11. Control everything safe from the dashboard

**Status: built (U1–U5).** Tiers live in `serve/policy.rs` (`GET /api/policy`, route audit test), jobs in `serve/jobs.rs` (`/api/jobs`, Jobs tray), actions in `routes/control.rs`, pages Tools / Agents / Brain / Settings. Command logic is reused via subprocess (`run_self`) instead of a separate `ops/` module. Not built: lock icons from `/api/policy`, native Cohere/Voyage/Gemini adapters (E5).

Goal: anything a person does in the terminal that is safe can be done (and watched) in the dashboard: start or stop indexing, sync, export instruction files,
run a health check, connect an agent, record a decision. The terminal stays the way to do the few things that must not be reachable from a web page.

### 11.1 Rules
1. **One implementation.** Command bodies move into `automemory-core/src/ops/` (functions returning data, no printing). The CLI prints them, REST returns them as JSON. No
   logic lives only in a command or only in a route.
2. **Three tiers, one table** (`automemory-core/src/policy.rs`: action name to tier, used by every route and shown in the UI as a lock icon):
   - **Safe**: read-only, or derived from verified data, local and undoable. No password.
   - **Protected**: creates trusted knowledge, changes what agents may do, or sends data off the machine. Human password every time, never stored (section 10 gate).
   - **Terminal only**: never reachable over HTTP.
3. **Decided by effect, not by name.** Indexing with a local model is Safe; the same button with a cloud profile is Protected. Sync pull is Safe; push to a remote is Protected.
4. **Why "safe" is narrow.** The dashboard API listens on localhost, so an agent with a shell can call any Safe endpoint. A Safe endpoint must therefore be harmless in an agent's hands:
   notes, decisions, pins, approvals and mode changes create trust or loosen safety, so they stay Protected.
5. **Long work is a job.** One job runner (`/api/jobs`, WebSocket `job_progress`, cancel) replaces the one-off indexing state; a Jobs tray in the top bar shows what is running.
6. **Preview before writing.** Anything that writes files shows the exact change first (export-rules diff, connect file list), then applies.
7. **Everything is traced.** Each UI action writes an `audit_log` row (action, tier, result); a "Recent activity" view shows it next to the blocks agents hit.

### 11.2 What goes where
| Operation | Tier | Dashboard home |
|---|---|---|
| Health check (`doctor`), hook dry run, guardrail evaluate, overlap check, resume preview, recently blocked, audit view | Safe | Settings > Health, Safety, Agents |
| Index memories: start, stop, re-index, clean (local model) | Safe | Recall settings (done, today Protected for all models) |
| Index with a cloud model, store or remove an API key, add or remove a model profile | Protected | Recall settings (done) |
| Export instruction files (preview, apply, drift check), write a hand-off now | Safe | Project brain > Preview export; Agents > Hand-off |
| Rescan and compile docs, index the code graph, run eval suites, backup the database now | Safe (jobs) | Settings > Tools |
| Sync: status, pull | Safe | Sync |
| Sync: push, set or change the remote, bootstrap another machine | Protected | Sync |
| Add note, record or supersede a decision, edit memory, pin, veto, revoke, restore | Protected | Project brain, new Decisions view |
| Approve or reject suggestions | Protected | Inbox (done) |
| Connect or disconnect an agent in this project (shows the files first) | Protected | Agents > Set up an agent |
| Release a lock, kill switch (release every lock) | Protected | Agents & locks |
| Safety mode, guardrail rules, lock settings (ignore patterns, idle time) | Protected | Safety (done), Settings > Locks |
| Set or change the human password, show the recovery phrase, `disconnect --everywhere`, `init`, `serve`, `hook`, `mcp`, export of keys | Terminal only | n/a (the page explains the command) |

### 11.3 Phases
| Phase | Scope | Done when |
|---|---|---|
| U1 | `policy.rs` tier table, route audit test (every POST/PUT/DELETE route must have a tier), job runner + `/api/jobs` + Jobs tray, move indexing onto it, effect-based gating (local index is Safe) | The audit test fails when a route is added without a tier; indexing progress appears in the tray from any page |
| U2 | `ops/` extraction for doctor, export-rules, handoff, sync, docs rescan, graph index, eval, backup; Settings > Tools and Health cards | Each card runs the same code as the CLI and shows its result; scenario compares CLI and REST output |
| U3 | Agents > Set up an agent (list adapters, status here, file preview, connect, disconnect) | Connect then disconnect leaves no registration (`disconnect --dry-run` empty); preview equals what is written |
| U4 | Decisions view (record, supersede, history), memory restore/pin/veto/revoke in Project brain | Every action asks for the password; each is undoable or logged |
| U5 | Settings > Locks and Data (lock settings, backup now, export, archive data), activity log view | Config changes persist and apply to the next hook run |

Tests: a policy-table unit test; the route audit; a scenario per tier (Safe works without a password, Protected is refused without one and throttled after five wrong tries,
Terminal-only paths return 404); a manual checklist item per new card.

Decisions to confirm: (a) notes and decisions stay Protected (the password is asked every time, as you chose); (b) the kill switch stays Protected so an agent cannot call it; (c) local
indexing needs no password.

## 12. Next: proof, quality, reach

**Status: N1–N8 built to the extent listed in `docs/TASKS.md`.** Open edges: live A/B numbers (N1 needs real agent runs), recorded hook payloads for Cursor/Windsurf/Gemini/Copilot (N4), per-person trust tiers (N5), Apple notarisation and the Homebrew tap (N7).

Order matters. N1 first (it says whether anything else helps), N8 before N5, N2 after N1 so its effect can be measured.

### 12.1 Phases
| Phase | Scope | Done when |
|---|---|---|
| N1 Proof | Extend `tests/eval/` to run the same tasks with and without AutoMemory (2+ agents, 3 runs each); metrics: task success, repeated mistakes, collisions, tokens, time. `eval report` writes a table; CI runs a small deterministic subset | Numbers are in FEATURES.md with the date and agent versions; a drop below the stored baseline fails the gate |
| N2 LLM profile | `[llm.profiles.<name>]` mirroring embeddings profiles (OpenAI-compatible and local Ollama first), key in the keychain, `allow_network`, daily token cap, cost estimate, redaction before send. Jobs: `contradiction-check` (before promote), `merge-suggest` (cluster near-duplicates), `distill-session` (events to decision/gotcha/convention candidates), `staleness` (memory vs files). Output is only candidate-tier proposals with sources, shown in the Inbox | Off by default; with no profile nothing changes; a hallucinated proposal can be rejected and never reaches trusted recall; calls are audited; N1 shows the effect |
| N3 Hygiene | Brain shows confidence, last used, times recalled; filter "never recalled" and "stale"; bulk archive (Protected, undoable) | A 500-memory demo set can be pruned in under a minute |
| N4 Agents | Cursor, Windsurf, Gemini CLI, Copilot: export-rules targets, MCP registration, hook where the agent has one, contract fixtures per agent | Each passes `connect`, `disconnect --dry-run` empty, and a contract fixture |
| N5 Team sync | Shared git remote with per-person identity, trust tiers by person, conflict view | Two people, two machines: a human-approved memory reaches the other; an unapproved one stays a proposal |
| N6 Perf gate | `cargo bench` thresholds in `run_all.sh --full`: hook p95, recall p95, binary size | Regression fails the run |
| N7 Packaging | Homebrew formula, signed and notarised macOS build, Linux tarball, `automemory update` (checks hash, never silent) | Fresh machine installs and runs `init` with no Rust toolchain |
| N8 Hardening | Review `serve/` and the hook path: Safe endpoints an agent could abuse, path traversal, subprocess arguments, SSRF in key/test routes, audit coverage | Findings fixed or recorded; tests added for each fix |

### 12.2 The "no LLM in the daemon" rule
The rule becomes: the daemon makes no LLM call unless the user configures an LLM profile, and an LLM never writes trusted memory, runs a lock, or decides a guardrail.
Update `CLAUDE.md`, ARCHITECTURE and FEATURES when N2 starts, not before.

### 12.3 Risks
- N2 adds cost and a prompt-injection path (memory text is untrusted input to the model): treat output as data, no tool use, strict JSON, length caps, proposals only.
- N1 results depend on agent versions and variance: report medians and the spread, not one run.
- N5 widens the attack surface: N8 first.

## 13. Next improvements (after §12)

What the work so far showed is weakest, in order of payoff. Each is small enough to ship alone.

| # | Improvement | Why | Done when |
|---|---|---|---|
| I1 | **Value report**: `automemory stats` and a Home card built from data we already keep (audit_log, leases, recalls): collisions prevented, notes recalled, hand-offs picked up, blocks by rule, tokens spent on the LLM helper | Nothing today tells a user the product is working, and N1 needs real usage numbers as well as lab runs | A week of use shows non-zero counts; numbers match the audit rows in a scenario test |
| I2 | **Record real hook payloads**: `automemory hook --record <dir>` (off by default, secrets redacted) so contract fixtures for Cursor, Windsurf, Gemini CLI and Copilot come from normal use, not guesses | N4 stopped at MCP and rules because hook formats cannot be invented | A fixture per agent passes `tests/contract/run.py`; the agent leaves "experimental" |
| I3 | **Trust overlay for teams** (N5b): a receiver-side table `trust_overrides(memory_id, tier, set_by)` instead of editing synced records, so a teammate's pin arrives as a request you approve | Clamping the record itself made fingerprints diverge | Teammate's pin shows as a pending request; approve or reject changes only this machine; no sync conflicts |
| I4 | **UI smoke test in a real browser** (Playwright or the built-in browser driver): every page loads with demo data, no console errors, the tour completes, a Protected action asks for the password | Only colour contrast is tested in the UI today; render crashes were found by hand twice | `tests/run_all.sh` fails when a page throws |
| I5 | **Lock hints from `/api/policy`**: a small lock icon and "needs your password" caption on every Protected button; Safe buttons say nothing | The tier table exists but the UI only reveals it when the prompt appears | Icons match the table; a test compares the page's buttons to `/api/policy` |
| I6 | **Restore and verify backups**: `automemory backup restore <file>` (previews counts, keeps the current DB as a copy) and a monthly "backup is readable" check in `doctor` | Backups can be written from the dashboard but there is no tested way back | Restore round trip in a scenario; doctor warns when the newest backup is older than 30 days or unreadable |
| I7 | **LLM profiles and native adapters** (E5): a map of `[llm.profiles.*]` like embeddings, Cohere/Voyage/Gemini native embedding adapters | Cloud choice is limited to OpenAI-compatible endpoints; one LLM profile only | Switching profiles from the Tools page; adapters covered by mock-server tests |
| I8 | **Fuzz the parsers**: `cargo fuzz` targets for hook JSON, shell command parsing and delta files, run for a fixed time in CI | Hooks run on every tool call; a panic there blocks an agent | 10 minutes per target with no crash; found crashes become unit tests |
| I9 | **Windows**: `update`, hooks and `connect` on Windows (the installer already exists) with a CI job (advisory job and code written; first Windows run still to come) | `update` and the matrix tests are macOS/Linux only | Windows CI job runs the scenario subset |
| I10 | **Cosmetic backlog**: decision card repeats its topic, "1 working now" wording, Activity view as its own page | Noted during the trial, not blocking | Fixed and seen in the browser |

Order: I1 and I4 first (they make every later change safer and prove value), then I2 and I6, then I3, I5, I8, I7, I9, I10.
Not planned: ONNX (see §1 reasons), a hosted service, agent-written trusted memory.

## 14. Planning rooms that people actually use

Detailed spec (flows, screens, API, data, hook change, tests): [plan/room-plan.md](../plan/room-plan.md).

Today a room works only if an agent opens it and every agent is told by hand to join and post. The human can only
judge the final plan as it is. This section makes the room something you start, steer and correct, while keeping the two
rules that make it worth having: **turns** (no endless chat, bounded tokens) and **evidence** (claims point at stored notes).

Not planned: free-form chat between agents, more than three rounds, votes, rooms that write code. Each costs tokens
and hides who decided what.

### 14.1 What the user gets
| # | Capability | How it works |
|---|---|---|
| R1 | **Start a room from the dashboard** | "New room": topic, the question to answer, which agents take part (from Agents), optional context: notes picked by search (become evidence), files/symbols in scope, constraints ("no new services"). Protected (password). |
| R1 | **Agents are told, not asked** | The prompt/boot hook adds one line to an invited agent's context: "Planning room *topic* waits for your proposal/critique (room id …)". No more pasting room ids. The MCP `room.pending` stays the source. |
| R2 | **You can speak in the room** | A human note (constraint, answer to a question, "prefer option B") is added without taking a turn and shown to the next agent with its trust tier (human = tier 3). |
| R2 | **Edit the plan before approving** | "If you approve" becomes editable: decision text, steps (add, remove, reorder, reword), and for each step an owner (agent), files in scope and a check command. Approve stores the edited version and marks it "edited by you". |
| R3 | **Plans become trackable work** | Tasks carry order, owner, files and a check. The room page shows each task's state (open, claimed by …, done, check passed/failed). Files in a claimed task are locked for its owner. |
| R3 | **Change it later** | An approved room links to its decision and tasks. "Revise" opens a follow-up room seeded with the old decision; approving it supersedes the decision (existing `decision.record` supersede) and cancels unclaimed tasks it replaces. Tasks can be renamed or cancelled from the page. |
| R4 | **Rounds and reviewers** | Per room: 1–3 rounds; with three or more agents every non-author critiques in a round (order fixed). Turn timeout (default 30 min): you are told on Home, and can skip the turn or end the room. |
| R4 | **Live** | Room events go over the existing WebSocket; the page updates without polling. |
| R5 | **Keep and learn** | Approved rooms export as a short decision record (Markdown, in the repo's docs folder if one is tracked). The value report counts rooms, approvals, tasks done. The Planning Room A/B (TASKS) runs on R1–R3 before R4 is built. |

### 14.2 Rules that stay
- Agents never approve: only the human verdict (password) stores the decision and tasks. A human note is never evidence.
- Evidence must resolve (memory, trace, failure, and from R1 a decision id); posts are capped; other agents' text is quoted data.
- Everything the human does is in the audit log and the Activity page.

### 14.3 Phases
| Phase | Scope | Done when |
|---|---|---|
| R1 | `POST /api/rooms` (topic, question, invited agents, evidence ids, scope, constraints); New room dialog with note search; hook context line for invited agents; `room.join` returns the question and constraints | A room started in the dashboard is joined and proposed in by a real agent with no room id pasted (trial T11b) |
| R2 | `POST /api/rooms/:id/note`; editable plan (decision, steps with owner/files/check) stored as a draft and used by approve; "edited by you" provenance | Approve after an edit stores exactly the edited text and steps; agents see the human note on their next turn |
| R3 | Task fields (order, owner, files, check) via a migration; room page task list with live state; claiming a task locks its files; Revise → follow-up room that supersedes on approval; cancel/rename task | Superseding leaves one active decision and no orphan open tasks; a claimed task's files show as locked |
| R4 | Rounds 1–3, multi-critic order, turn timeout + skip/end; WebSocket room events | A three-agent, two-round room finishes; a timed-out turn shows on Home |
| R5 | Markdown decision record export, value-report counts, the A/B on R1–R3 | A/B result recorded in FEATURES; drop or keep R4 based on it |
| R6 | Room types (decision, plan, review, incident, retro), roles (chair, member, observer), versioned draft agents can edit, amendments, deep link by id, opt-in delegated approval by the chair for chosen types | Stored decisions change only through Revise; delegation off by default and never for superseding or pinned decisions |
| R7 | Posts owned like a chat app: authors edit and delete their own (history and "deleted" placeholder kept), replies, @mentions that reach the agent's next prompt, agree/object reactions, seen and unread; nobody edits or deletes another's post (the chair can only collapse; you can delete with the password) | Ownership enforced by the daemon on every edit and delete, logged |

Order: R1 and R2 first (they remove the copy-paste and give the human real control), then R3, R6 and R7; R4 only if the A/B shows
rooms beat one agent plus tests.

## 15. What to add next (research, Oct 2026)

Inputs: our own database and trials, [docs/research/memory_improvement.md](research/memory_improvement.md) (benchmarks), and a short web
check on four topics. Web sources are mostly blogs and aggregators; where a claim matters it is marked **verify** (check the primary
source before building on it).

### 15.1 What we saw
| Signal | Source | Meaning |
|---|---|---|
| The trial database holds **6,794 procedural skills with 4 names and 29 distinct step lists** ("Add format_price function" ×2500), 216 traces with 4 titles; 18 MB for 14 active notes | `~/.automemory` (today) | The distiller still inserts a copy per run instead of updating one. Wasteful, slows sync, and any skill list shown to agents is noise |
| Boilerplate notes fell from 76% to 29%; 12 of 14 notes have been recalled once | same, vs the Oct 18 audit | Write-path gate and recall tracking work; skills and traces were not fixed |
| Memory systems rarely beat memory-off; verified experience and **selection** do | VibeMemBench, SWE-ContextBench (see research file) | Measure value before adding recall features |
| MCP spec **2026-07-28** removes protocol sessions and the `initialize` handshake (stateless, per-request `_meta`), reworks Tasks, deprecates Roots, Sampling and Logging | [blog summary](https://blog.mcpservers.org/posts/mcp-spec-2026-07-28), [changelog mirror](https://modelcontextprotocol.net/specification/2026-07-28/changelog) (**verify** on modelcontextprotocol.io) | Hosts will move to it; our stdio and `/mcp` servers answer `initialize` today |
| Git **worktrees** are the standard isolation for parallel agents; coordination on top is a shared task list, a mailbox and claim locks (Claude Code "agent teams", experimental) | [Augment guide](https://augmentcode.com/guides/git-worktrees-parallel-ai-agent-execution), [Morph](https://www.morphllm.com/claude-code-git-worktree) | Our locks, tasks and rooms cover coordination; we do not help create or clean worktrees |
| **SKILL.md** (agentskills.io, Dec 2025) is read by 30+ tools; AGENTS.md is near universal | [noqta](https://www.noqta.tn/en/news/skill-md-open-standard-30-ai-coding-tools-adoption-2026) (**verify** counts) | We write skills only for Antigravity; verified knowledge could reach every agent as a skill |
| Trial T11/T11b: a day-old agent MCP server ignored new rules; a "test" post spent a turn; agents did unrequested housekeeping on a broad prompt | `tests/manual/TRIAL_LOG.md` | Upgrades, input floors and narrow prompts matter more than features |

### 15.2 Ranked proposals
| # | Proposal | Why now | Size | Done when |
|---|---|---|---|---|
| X1 | **Fix the write path for skills and traces**: one skill per (name, steps hash) updated in place (`success_count++`), traces deduped by signature, `automemory memory prune` merges the 6,794 into ~30 (reversible, backup first), distiller refuses empty or generic names | Biggest measured defect; makes sync, recall and the Brain view useful | M | Trial DB shrinks to tens of skills; a rerun adds none; before/after in the trial log |
| X2 | **MCP 2026-07-28 compatibility**: accept stateless requests (no `initialize`, protocol version in `_meta`), return `resultType`, keep answering the old handshake; test against both | Hosts will switch; breaking silently loses every agent | M | Contract tests for both protocol versions pass; `doctor` reports the version a host used |
| X3 | **Value proof**: run the eval harness (N1) on 10 real tasks, off vs on, publish medians; add "memory off" switch per project for fair runs | Research says most memory does not help; we must show ours does or cut it | M | A table in FEATURES.md with dates and agent versions |
| X4 | **Worktree helper**: `automemory worktree add <task>` (branch per task, copies env files, registers the path so locks and overlap checks know it), cleanup on task submit, claim-to-worktree in the Planning room task flow | Where practitioners already are; turns tasks into isolated work | M | Two agents claim two tasks and edit the same file in separate worktrees; overlap check warns before merge |
| X5 | **Skills export (SKILL.md)**: write verified decisions, conventions and good procedures as `.agents/skills/` / `.claude/skills/` folders, refreshed like instruction files | Reaches 30+ tools through one format; reuses `export-rules` machinery | S | A skill written for one agent is read by a second; disconnect removes only ours |
| X6 | **Memory validity and history**: `valid_from` / `invalid_at` on decisions and notes; Revise (room) and supersede set `invalid_at` instead of hiding; "what changed since" view | Forgetting and contradiction are where systems fail (research file §2) | M | Superseded decision stays readable with dates; recall never returns an invalid note |
| X7 | **Upgrade hygiene**: `connect`/`update` print which agents hold an old MCP server (doctor check exists); dashboard banner "Restart Antigravity (server 44 h old)" | T11b lost an hour to it | S | Banner appears after an install and clears after restart |
| X8 | **Narrow-task prompts in the rules text**: say "do only what you were asked; never commit, resolve failures or pick up batons unless told" in the connect guidelines | Trial: Antigravity committed 16 files unasked | S | Same neutral prompt in the trial no longer commits |
| X9 | **Hook-level adapters for Cursor, Windsurf, Gemini CLI, Copilot** from recorded payloads (I2 tooling) | Biggest reach gap; needs real captures, not guesses | M | One fixture per agent; agent leaves "experimental" |
| X10 | **Rooms: only if X3 shows value**: R2 (edit plan steps), then R3 (task state); park R4–R7 | Rooms worked in T11/T11b but cost tokens and need proof | M | A/B on 5 design tasks: room vs one agent plus tests |

Order: X1 and X7/X8 first (cheap, visible, fix real defects), X2 before the next MCP-host release, X3 in parallel (it decides X10), then X5, X4, X6, X9.

### 15.3 Not recommended
- A vector database or graph store: retrieval is not the bottleneck (research, our data).
- Agent-to-agent chat beyond rooms, voting, or LLM-judged consensus: cost and blurred responsibility.
- Auto-approving anything an agent proposes: the human verdict is the product's trust boundary.
- Cloud sync service before X1 (a bloated store would sync bloat).

## 16. Continuity: the same understanding in any agent

Detailed spec: [plan/continuity-plan.md](../plan/continuity-plan.md). Decided with the maintainer: brief plus rejected ideas plus
searchable history; captured both by the agent and by reading local transcripts (opt-in per agent, secrets removed, local only);
refreshed automatically and on demand; brief syncs automatically, chat history only when asked; agent-written brief for agents we
cannot read yet, readers added per agent later; code modular (one reader file + registry line + fixtures per agent).

| Phase | Scope | Done when |
|---|---|---|
| C1 | Brief store with provenance, `continuity.brief` MCP tool, hook injection, rules text, CLI, brief sync | Any agent updates it; a different agent receives it |
| C2 | Reader trait + testkit, Claude Code reader, searchable archive, `chat.search`/`chat.get` | Long Claude session imported incrementally and searchable |
| C3 | Antigravity reader (conversation db, summaries, brain files) | Same for Antigravity |
| C4 | Dashboard Continuity page: opt-in, import, brief editor, preview, Hand off now | Human controls everything from the page |
| C5 | Deterministic drafter and confirm flow | Fixture transcript yields the expected items |
| C6 | Automatic triggers (Stop, SessionEnd, PreCompact), manual history sync | Mid-task switch needs no manual step |
| C7 | Research and readers for Codex, Gemini CLI, Command Code, Z code, Cursor, … | Each passes the testkit |
| C8 | Continuity eval, live trial T12, optional LLM drafting | Eval recorded; T12 passes |

## 17. Project library: agent documents outside git (plan only)

Detailed spec: [plan/library-plan.md](../plan/library-plan.md). Requested by the maintainer: PRDs, plans and notes for agents that do not
belong in the repository but must sync between computers and be found by every agent, in the spirit of
[OpenViking](https://github.com/volcengine/OpenViking) (files addressed by path, three-tier loading, directory-first search), without its
model-at-write-time design. Decided: folder `.automemory/library/`, sync to all your computers by default, teammate sharing later, files can also be listed in the global config and synced in place whether or not git tracks or ignores them, day-one agents Claude Code, Antigravity and Codex. Three smaller questions remain at the end of the spec. Not part of v1.

| Phase | Scope | Done when |
|---|---|---|
| L1 | Store, revisions, secrets check, deterministic tiers, CLI, MCP list/get/search, hook index | **Built.** An agent lists and reads a library file; `git status` untouched |
| L2 | Materialize with `.git/info/exclude`, INDEX.md, two-way sync (hook-driven, no always-on watcher), linked files, doctor check | **Built.** Edits land in the library; git stays clean |
| L3 | Vault sync (record kind `library`, content inside the encrypted delta), personal vs shared, conflict handling | **Built.** Readable on the second computer; ciphertext remotely |
| L4 | Dashboard Library page; Docs page shows both sources | All actions from the page |
| L5 | `library.put`/`propose`, links from brief/decisions/rooms, move-out-of-git helper | Agent files labelled; one-command move |
| L6 | Attachments, optional model abstracts as suggestions | Decide after use |
