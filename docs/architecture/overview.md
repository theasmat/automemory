# AutoMemory — Architecture

How the system is built. What it does and for whom: [features](../features.md). Settled choices: [decisions](../decisions.md). Open work: [roadmap](../roadmap.md). Subsystems: [locking](locking.md), [retrieval](retrieval.md), [sync](sync.md), [dashboard control](dashboard-control.md), [identity](identity.md).

Status: describes the code as built (Oct 2026). Items marked *(planned)* are not built.

---

## 1. System overview

A local Rust daemon and CLI (single binary) with SQLite (WAL), an MCP server, and an embedded dashboard. Agents reach it through MCP tools and optional hooks. Nothing requires a network; the daemon makes **no LLM calls** unless you turn on the optional `[llm]` helper, which only produces suggestions for the Inbox (`llm.rs`).

```
agents ──MCP / hooks──► adapters ──canonical events──► hook pipeline ──► core (ledger · store · event bus · plugin host)
                                                                           │
                                          ports: Retriever · Verifier · CodeIntel · Sync · Distiller · …
```

Workspace layout:

| Crate | Role |
|---|---|
| `automemory-core` | Domain logic: `db/` (SQLite, migrations, queries), `guardrails/`, `extraction/` (distiller, time-span digest, skills), `retrieval/`, `codeintel.rs` (CodeIntel bridge to codebase-memory), `crypto/`, `sync/`, `models/` |
| `automemory-cli` | CLI, hook handler, `serve` daemon, embedded dashboard assets |
| `automemory-mcp` | JSON-RPC 2.0 MCP server over stdio and HTTP |
| `clients/web` | Nuxt 3 dashboard source |
| `workers/` | Cloudflare worker (**later phase**, not v1) |

## 2. Design principles

| Principle | Consequence in the design |
|---|---|
| Pull, don't push | Agents query tools; boot-time injection exists but is gated by `[defaults] boot_injection` (default off) |
| Store only what the code cannot show | Decisions, constraints, gotchas; no repo summaries |
| Verify before returning | Recall labels results verified / changed / stale; PostCommit flips linked memories to `changed` |
| Provenance on every write | Trust tiers decide what is returned as fact |
| Facts from the ledger, prose from agents | Exit codes, diffs, test results are authoritative |
| Tests are the judge | Cross-review decided by test results, not votes |
| Everything is a port | Hooks, adapters, stores, policies replaceable without touching core |
| Degrade gracefully | Hooks → MCP only → file watcher |
| Local first, no LLM in the daemon by default | Deterministic distillation, ranking, guardrails; an optional LLM only suggests |

## 3. Core and ports

Microkernel: ledger, store, event bus, plugin host. Everything else sits behind a versioned port with a default implementation.

| Port | Job | Default |
|---|---|---|
| AgentAdapter | Install hooks/MCP config; map native → canonical events | Claude Code, Codex, Antigravity (v1); 17 more connectors exist, unsupported until promoted |
| HookHandler | One pipeline stage: allow / warn / block / annotate | Guardrail, nudge, ledger logger (file locks: Tracewise, nested) |
| CodeIntel | `symbol_exists`, `symbol_hash`, `callers` (PostCommit uses `symbol_hash`: a memory saved with `symbols` and `files_modified` records its symbols' hashes when the engine is present, and is marked `changed` only if one of them changed; otherwise any edit to a linked file marks it) | **codebase-memory-mcp** via `codeintel::CbmCodeIntel` (`cli --json`; located by `AUTOMEMORY_CBM_BIN`, `PATH`, `~/.local/bin`; min version 0.11, checked by `doctor`). `NoCodeIntel` (answers "unknown") when the engine is absent or a call fails. Symbol hash = sha256 of the symbol's own source. AutoMemory builds no AST graph |
| MemoryStore | Persist memories, decisions, ledger | SQLite WAL |
| Retriever | Rank candidates | FTS5 BM25 × decay × confidence; local n-gram vector projection |
| EmbeddingProvider | Text → vectors | `[embeddings]` provider: n-gram (default), Ollama, OpenAI-compatible HTTP, or any command (`exec`); vectors stored per model in `memory_vectors`. Saved as profiles (`[embeddings.profiles.<name>]`, active one in `[embeddings] active`); cloud API keys live in the OS keychain through `secrets.rs` (the `keyring` crate; encrypted-file fallback), never in config, database, logs or sync. Used only when `[retrieval] mode = "hybrid"`; hooks are always keyword-only; remote endpoints need `allow_network` and get redacted text; any provider failure falls back to keyword |
| Verifier | Check a claim against reality | Symbol / file-hash verifier |
| TrustPolicy | Source → tier; what is returned as fact | Built-in tiers |
| GuardrailRule | Block catastrophic actions | Shell tokenizer + rule set |
| Distiller | Candidates at session end | Deterministic trajectory distiller |
| SyncBackend | Move ops between devices | Git vault (Cloudflare later) |
| Runner | Launch an agent headless | None in v1 |
| RoomProtocol | Planning room format | Propose → critique → revise |
| Notifier | Tell the human it is their turn | Desktop toast |

Plugins declare a manifest (`api_version`, `ports`, capabilities, hook subscriptions, priority, latency budget, failure mode). Rules: versioned ports (host refuses mismatches), least-privilege capabilities, namespaced storage with the plugin's own trust tier, out-of-process or WASM sandboxing, per-plugin migrations, contract tests per agent version. Config is layered: defaults → `~/.automemory/config.toml` → `.automemory/config.toml` → local override.

## 4. Hook pipeline

Agents emit native events → adapter normalizes → pipeline runs ordered handlers (built-in Rust, external executable JSON-in/JSON-out, or WASM *(planned)*). Each handler declares priority, latency budget, failure mode (open/closed), and subscribed events. Strictest decision wins. Exec handlers are killed past budget.

| CLI `--event` | Canonical | Behaviour |
|---|---|---|
| `boot` | SessionStart | Silent by default; with `boot_injection` prints token-capped constitution + recent decisions |
| `pre-tool` | PreToolUse | Gate: guardrail → custom exec (Tracewise's lock answer merged in) |
| `post-tool`, `post-tool-fail` | PostToolUse | Append outcome (exit code, stderr) to ledger; failures feed the failure catalog |
| `task-done` / `stop` | Stop | Distill and save a `Session checkpoint` memory if the session edited files (locks, when Tracewise is there, are released by it); `stop` blocks if a review is owed |
| `session-end` | SessionEnd | Hand-off and sync (Tracewise releases the session's locks) |
| `pre-commit` | PreCommit | Gate over staged files; Tracewise's answer is merged in. `connect` installs a git pre-commit block that shows the warning in the commit output and blocks only when the hook answers deny (Strict mode); it fails open |
| `post-commit` | PostCommit | Flip linked memories to `changed`; distill |
| `user-prompt-submit` | UserPromptSubmit | One-line nudge only if work is assigned |
| `subagent-*`, `pre/post-compact`, `notify` | — | Ledger only, for attribution/replay |

**Antigravity surfaces.** The CLI (`agy`), the desktop App (2.x) and the IDE share one hook schema and the same workspace file `.agents/hooks.json`; the global file is `~/.gemini/config/hooks.json` (the CLI also reads `~/.gemini/antigravity-cli/settings.json` and plugin `hooks.json`). They differ in data folder (`~/.gemini/antigravity-cli`, `antigravity`, `antigravity-ide`) and skill path (App/IDE `~/.gemini/config/skills`, CLI `~/.gemini/antigravity-cli/skills`). Forum reports from Aug 2026 said hooks ran only in the CLI; they fire in App 2.24. `connect agy` registers hooks exactly once (global file with `-g`, workspace file otherwise, never both), merges into an existing hooks.json under its own `automemory` key, writes no plugin copy and no Claude `.mcp.json`, and the global connect also installs the CLI skill when the CLI is present. Any duplicate registration is harmless at runtime: `hook` claims each invocation (agent + event + payload hash) so a twin within 1.5 s replays the first reply instead of acting twice (only for payloads that carry a step id such as `tool_use_id` or `stepIdx`, so a genuine retry always gets a fresh decision).

**Hooks never fail.** `hook` always ends in exactly one valid reply in the agent's schema: panics, errors and an unreachable or corrupt database become a fail-open reply (`{decision: allow}` for Antigravity PreToolUse/Stop, `{}` otherwise) on stdout with exit code 0. A Stop with `fullyIdle: true` releases an Antigravity session's leases (it has no SessionEnd).

**De-registering.** `automemory disconnect <agent|all> [-g|--everywhere] [--dry-run]` removes only what `connect` wrote (MCP entries, hook keys and groups, skills, generated rules/AGENTS.md/GEMINI.md/CLAUDE.md when still byte-identical, legacy plugin folders, and AutoMemory's block in `.git/hooks/post-commit`, which `connect` appends to instead of overwriting). `doctor` shows how many registrations exist; `~/.automemory` data is left intact.

**Antigravity output schema** (antigravity.google/docs/hooks; it rejects unknown fields): PreToolUse `{decision: allow|ask|deny, reason}` (`reason` on an allow is not shown to the model), PostToolUse `{}` only, PreInvocation `{injectSteps:[{ephemeralMessage}]}`, Stop `{decision: continue|<other>}`. So for `--agent agy` the hook prints only those: anything it wants the model to see (waiting notes from reconciliation, files other agents hold, boot context on step 0) goes out on the next PreInvocation as an `ephemeralMessage`, and a Stop never blocks except with `decision: continue`. A deny in Strict mode is a hard block. Notes wait in `session_notes`.

Hook contract: one JSON event in (`event, agent, session, tool, paths, symbols`), one JSON decision out (`decision, message`). Hot-path budget **< 5 ms p95** (measured ~1.26 ms mean). Hook invocations distill synchronously on Stop/PostCommit; the `serve` daemon also runs extraction and consolidation workers.

Project detection: exact root path → git root → remote fingerprint → approved alias; unregistered projects are never auto-created in the cloud.

Hook management: `automemory hooks enable|disable|move|add|test`, shadow mode (log only), hot reload, `adapter update`, `plugin install|update|pin|rollback`.

## 5. Data model

One SQLite file (`~/.automemory/automemory.db`), rows keyed by `project_id`. Migrations gated by `PRAGMA user_version`, run atomically under `BEGIN IMMEDIATE`; a failed step aborts and rolls back instead of bumping the version. Lease acquisition is also one `BEGIN IMMEDIATE` transaction, because every hook is its own process. Single `Arc<Mutex<Connection>>`, WAL, `busy_timeout` 5 s.

**Tables:** `projects`, `project_paths`, `sessions`, `tasks`, `tool_events` (append-only ledger), `memories`, `memory_versions`, `memory_vectors`, `success_traces`, `failure_records`, `preferences`, `tracked_documents`, `document_revisions`, `review_proposals`, `audit_log`, `custom_guardrail_rules`, `procedural_skills`, `decisions`, `batons`, `reviews`, `rooms`, `room_posts`, `sync_devices`. (Schema v20 drops the old `graph_entities` and `graph_edges`; v21 adds `sync_devices`; v22 drops `memory_links`, which nothing read or wrote; v23 drops `leases`, `lock_alerts`, `lock_coverage` and `workspace_snapshots`: file locks moved to Tracewise.)

**Memory:** `scope` (global|project), `type` (fact, decision, lesson, pattern, warning, observation, preference, skill), `status` (active, candidate, pinned, stale, changed, superseded, archived, invalid, review), `source_kind`, `confidence`, `importance`, `freshness`, tags/entities/metadata. Dedup: scope-aware in-place upsert with a version row.

**Trust tiers:**

| Tier | Source | Returned as |
|---|---|---|
| 1 Pinned | Human pin | Fact; constitution |
| 2 Verified | Test/exit-code fact, symbol check passed | Fact with evidence |
| 3 User statement | Said by user | Fact with source |
| 4 Agent claim | Agent wrote it, unverified | Shown in a capped slot (a third of the result limit, at least one), labelled "unverified, written by <agent>"; withheld under `trust_policy = "strict"` |
| 5 Untrusted origin | Web, fetched files, other agents' posts | Quarantined until verified; only with `include_untrusted`, inside the same cap |

Synced: memories, decisions, constitution. Never synced: FTS index, vectors.

## 6. Retrieval and context

Pipeline: candidate selection (project, type, status, tags) → FTS5 BM25 → `× e^(−λΔt) × confidence` → diversity → token budget. Contradiction flagging at recall (same tool, different version). Measured: ~0.56 ms over 500 memories.

Context assembly order: locked preferences → project identity → high-confidence decisions/facts → patterns → warnings/failures → success traces → document excerpts → provenance. `memory.constitution` is token-capped (~1–2k).

## 7. Locks, handoff, orchestration

- **File locks (Tracewise, optional):** AutoMemory has none of its own. When the Tracewise companion is installed, the hook calls `tracewise hook <event> --from <agent> --neutral` with the agent's payload and merges the answer (strictest decision wins; a missing, slow or other-API Tracewise is skipped). Locks, shell-write tracking and collision notices come from it; see [locking](locking.md) and the [companion spec](../specs/tracewise-companion.md). A server started in a folder that is no registered project gets a private folder scope, never another project's memory.
- **Baton:** < 400-token handoff built from the ledger (`baton.pass` / `baton.pickup`).
- **Tasks/review:** `task.create|next|submit`, cross-review with tests as judge (`review.next|post|submit`).
- **Planning Room** (v1 scope): turn-gated state machine (propose → critique → revise, human tiebreak). A room advances only when the turn holder posts; the daemon rejects posts whose evidence IDs do not resolve. Every tool reply ends with a daemon-written `next_action`; posts from other agents are returned as quoted data with author and trust tier. Stored rationale is 3–5 lines, never raw reasoning. A decided room waits for the human verdict: approve records one decision (the last critique as dissent) and a task per step of the final revision; reject stores nothing; another round reopens the critique. The verdict is kept in `sync_kv` (`room_outcome:<id>`), so approving twice is refused.
- **Autonomy profiles:** Observe (record/warn), Assist (default; human approves each stage), Autopilot (opt-in per repo). Always stops for: schema/migration, auth/secrets, deletion, edits outside agreed files, same test failing 3×, budget exceeded, unresolved disagreement after round cap. Kill switch: `automemory stop`.
- **Overlap check:** `automemory overlap [--base main]` lists other local branches that change the same files as this one (git three-dot diff), for use before merging.
- **Worktree sync:** memory and decisions are shared across git worktrees via canonical git-root resolution (see the roadmap for open edges).

## 8. Guardrails

Recursive shell tokenizer (unwraps `sh -c`, `bash -c`, `--eval`, pipelines) plus rules. Modes: **Vibe** (default; block catastrophic only, warn on high-risk), **Strict** (halt high-risk pending confirmation), **Review** (queue as `ReviewProposal`).

- Critical (all modes): root/home deletion, raw disk write/format, protected-branch force-push, DB drop/truncate, plaintext secrets in commands.
- High-risk (Strict/Review): prod migrations, destructive git reset/clean, infra teardown, secret-file edits, raw `sqlite3` on AutoMemory's DB (Zero-SQL rule).
- Custom rules via `automemory rule add|list|toggle|remove`.
- Regex rules see what would execute: the printed text of `echo`/`grep`/`git commit -m` and here-document bodies are ignored, `$(...)`/backticks still count, newlines separate commands, and file tools (Write/Edit) are judged by path, never content. Extra critical checks: `find <root> -delete`, a `cd` to root followed by a recursive delete, recursive `chmod/chown` on root, `+main` pushes; `curl|sh` style pipes are high-risk.
- **Human password gate:** rule/mode/hook-pipeline mutations and dashboard write endpoints require a human password (PBKDF2-HMAC-SHA256, 600k rounds). CLI accepts it only from `AUTOMEMORY_PASSWORD` or a `/dev/tty` prompt; piped stdin (how agents run) fails closed. Dashboard uses `X-AutoMemory-Password`.

## 9. Interfaces

**MCP (47 tools advertised, stdio + HTTP).** `automemory mcp --agent <name>` (written by `connect`) makes the server sign every record with that name and ignore a different `agent` argument (audited as `mcp.agent_mismatch`); a room proposal cannot be critiqued by the same server connection. The server also replaces itself with the installed binary after an update (checked between requests; Unix). This stops mix-ups and casual spoofing, not an agent with a shell that starts its own server: anything that must not be faked stays behind the human password. Twenty-one older names were removed; a call by an old name answers "was removed, use …" instead of working: `memory.ask/context/topics/digest/constitution/skills`, `chat.search`, `library.search`, `baton.pass/pickup` are now `memory.recall`, `memory.list {view}`, `memory.search {scope}`, `continuity.brief {action: handoff}`, `memory.resume`; `lock.acquire`/`lock.release` are gone (locks are Tracewise's); `graph.*` belong to the `codebase-memory` server every agent already has; `doc.list/get` are `library.list/get` (the repository's own documents appear there with source `repo`); cross-review (`review.*`) is gone (use a planning room).

| Group | Tools |
|---|---|
| Read | `memory.recall/about/get/search/list/stats` (`search {scope: notes\|chat\|library\|all}`, `list {view: memories\|constitution\|context\|topics\|skills\|digest}`), `trace.list/get`, `failure.list/get`, `project.list/get`, `library.list/get`, `preference.get`, `sync.status`, `decision.get` |
| Write | `memory.save/update/archive/restore/propose/redact`, `decision.record`, `trace.save`, `failure.save/resolve`, `preference.set/lock`, `project.register`, `library.put` (own files only), `library.propose` (Inbox item) |
| Coordination | `lock.status`, `task.create/next/submit`, `room.join/pending/post/status`, `continuity.brief` (`get\|add\|handoff\|confirm\|correct\|resolve\|remove`), `memory.resume`, `chat.get` |
| Governance | `memory.proposals.list/summary/decide/decide_all` |

Not built: `memory.upsert/feedback/consolidate/provenance/export/import`, `trace.invalidate`, `project.configure`, `doc.track/untrack`, `sync.push/pull` (CLI only).

**CLI.** `init`, `doctor`, `connect <agent|all> [-g]`, `disconnect`, `serve --port`, `mcp`, `memory list|search|save|get|archive|digest`, `proposal summary|list|decide|approve-all|reject-all`, `mode get|set`, `rule …`, `sync remote|auto|bootstrap|pair`, `adapter`, `hooks`, `plugin`, `eval`, `stop`, `locks`/`unlock`, `decision`, `pin|veto|revoke`, `baton`. Run `automemory <cmd> --help` for flags; that is the reference.

**Dashboard / REST** (`automemory serve`, `127.0.0.1:4545`, embedded Nuxt 3 build): `/api/proposals[/summary|/:id/decide|/decide_all]`, `/api/coordination` (locks read from Tracewise, rooms, batons; the Agents & Locks page), `/api/memories`, `/api/stats`, `/api/guardrails/{rules,evaluate}`, `/api/mode`, WebSocket `/ws` for live events. Views: proposals queue, memory explorer, skills, failures. Every request must carry a loopback `Host` and, if it has an `Origin`, the dashboard's own origin (blocks CSRF and DNS rebinding; no CORS). Proposal decisions and guardrail/mode writes also need the human password header when one is set; the UI prompts once per tab. Once a password is set, the MCP tools `memory.proposals.decide` and `decide_all` refuse agents, and the CLI `proposal decide|approve-all|reject-all` need the password too.

**Shared instruction files** (`commands/rules_export.rs`, `resume::build_rules_view`): a derived, deterministic, redacted digest (active decisions plus user-written memories, ≤3 KB) inside `<!-- automemory:begin/end -->` markers in each agent's native instruction file. The DB stays the source of truth; hooks refresh only files that already carry the block; `disconnect` strips only the block (a file that held nothing else is deleted).

## 10. Sync and security

- **Conflict-safe op-log:** each daemon sync pass (`sync::deltas::sync_cycle`) applies other devices' content-addressed delta files, then writes this device's changes. Convergence is by fingerprint, not clock; a record edited on two devices is kept locally and filed as a `sync_conflict` review proposal; versions already seen are ignored, so old delta files cannot look like new edits. The readable snapshot export still runs after it.
- **v1:** one private git remote carries everything (`sync remote`, `sync auto`, `sync bootstrap`): encrypted delta files in `deltas/` (memories, traces, failures, skills, decisions, hand-offs, the brief, library files). The older text vault (`sync export/import/restore`), the cloud client (`sync init/status/push/pull/watch`) and its `/api/cloud/*` test routes were removed; disaster recovery is `backup` and `backup restore`.
- **Encryption:** client-side AES-256-GCM, key from a BIP-39 mnemonic (`sync pair`, `--join`): a 24-word phrase is the key itself, a 12-word phrase is expanded with HMAC-SHA256; PBKDF2 is for the human password and passphrase-style keys; key in OS keychain. IDs are blind HMACs.
- **Later:** Cloudflare managed mode (worker, D1/R2, Vectorize). Sync scope stays memories + decisions only. The worker verifies Stripe/LemonSqueezy webhook signatures, accepts the unsigned test payload only when `ENABLE_DEV_AUTH_BYPASS=true`, and compares the admin secret in constant time; set `STRIPE_WEBHOOK_SECRET` / `LEMONSQUEEZY_WEBHOOK_SECRET` before deploying.

| Threat | Defense |
|---|---|
| Poisoned memory | Trust tiers; quarantine tier 5; revoke by session/agent |
| Fake evidence | Evidence IDs must resolve on write |
| Prompt injection via retrieval | Retrieved text quoted as data; only `next_action` is instruction |
| Stale memory | Symbol/file-hash verify; commit-bound staleness |
| Runaway agents | Round caps, budgets, timeouts, `automemory stop` |
| Destructive commands | Guardrails + shadow mode first; password-gated policy edits |
| Plugin abuse | Capabilities, sandboxing, pinned versions |
| Secrets in ledger | Redact before write; raw ledger local only |

## 11. Install and agent surfaces

`install.sh` / `install.ps1` / `cargo install --git …`; build from source needs Rust 1.80+ (Node + pnpm only to rebuild `clients/web`, then copy `dist/*` into `crates/automemory-cli/src/dashboard/assets/`). `automemory doctor` verifies dirs, WAL, FTS, hooks.

| Surface | MCP | Hooks | Joins by |
|---|---|---|---|
| Claude Code | yes | SessionStart, UserPromptSubmit, PreToolUse, PostToolUse(+Failure), Stop via `~/.claude/settings.json` (global) or `.claude/settings.local.json` | hooks (+ optional headless later) |
| Codex CLI | yes (`config.toml`) | same events via `~/.codex/hooks.json` or `<repo>/.codex/hooks.json`; user must trust them once in `/hooks` | hooks or headless |
| Antigravity | yes | JSON hooks (`.agents/hooks.json`) | hooks + manager surface |
| Others (10 connectors, plus `connect other`) | yes | none | MCP + prompt nudges; unsupported until promoted |

Connector config paths live in `crates/automemory-cli/src/commands/connect.rs` (source of truth). Test with `cargo test --workspace`, `cargo bench -p automemory-core --bench perf`.
