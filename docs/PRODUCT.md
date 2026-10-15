# AutoMemory — Product Requirements & Technical Design

**Status:** v0.1.0 + hook pipeline (Oct 2026 — describes the code as built; supersedes the Oct 7 draft)
**Audience:** Product owner, coding agents, maintainers, contributors
**Product type:** Open-source, local-first persistent-memory platform for coding agents, with optional managed Cloudflare sync and user-owned GitHub sync.

---

## 1. Product Summary

AutoMemory is a persistent memory system for AI coding agents. It automatically captures useful task evidence, tool outcomes, decisions, errors, preferences, project documents, and successful procedures. In a new chat, it retrieves only the relevant context for the current project plus user-level preferences, so the agent can continue work without re-reading or re-discovering the entire codebase.

The product is a superset of typical coding-agent memory tools:

- Works with multiple coding agents through lifecycle hooks and MCP.
- Separates memories by project while retaining a global preference layer.
- Captures tool-call provenance and task outcomes automatically.
- Treats successful procedures and failure/root-cause records as first-class memory objects.
- Supports human-readable project documentation sync, including documents intentionally ignored by the project source repository.
- Offers two sync modes: managed Cloudflare SaaS or user-owned private GitHub repository.
- Uses local SQLite as the durable offline cache and source of operation on the developer machine.

## 2. Problem Statement

Coding agents start new chats with little or no knowledge of prior work. They often repeat expensive behavior:

- Re-scan the whole repository to understand architecture.
- Re-discover decisions already made.
- Repeat known deployment, test, migration, or debugging failures.
- Lose context when local agent files, chat history, or a workspace are deleted.
- Fail to carry useful context between Antigravity, Claude Code, Cursor, Codex, and other agent environments.
- Cannot easily synchronize private planning documents such as PRDs, architecture notes, and decision records that are intentionally `.gitignore`d in the source-code repository.

AutoMemory must make memory automatic and durable. The user should not have to repeatedly tell the agent to remember outcomes, and an agent running in auto-approve mode must still create an auditable memory trail.

## 3. Goals

### Primary goals

1. Automatically inject relevant global and project memory at the beginning of a new agent session.
2. Capture tool-call evidence, task outcomes, failures, and successful procedures without requiring the agent to remember to do it manually.
3. Keep project memories isolated: a project must not retrieve another project's technical context by default.
4. Maintain global user preferences independently of projects.
5. Support manual, agent-driven, and automatic create/update/archive operations for memories.
6. Preserve provenance: a stored memory must be traceable to its session, agent, task, tool call, and evidence where available.
7. Support durable multi-device sync through either managed Cloudflare SaaS or a user-controlled private GitHub repository.
8. Sync selected project documents, including documents ignored by the primary project repository.
9. Provide MCP in both local stdio and HTTP transports.
10. Remain useful without a cloud account or a paid model provider.

### Non-goals for MVP

- Fully autonomous production deployment control.
- Replacing source control for application code.
- Capturing every raw model token forever.
- Guaranteed semantic correctness of LLM-generated summaries.
- Team collaboration, RBAC, billing, or enterprise SSO in the first release.
- Building a custom foundation model or embedding model.

## 4. Principles

- **Local-first:** The CLI and local memory store must work offline.
- **Project-aware by default:** Retrieval should prioritize the current project and global preferences, never unrelated projects.
- **Evidence before assertion:** Memories derived from execution include provenance links and confidence.
- **Automatic, but reviewable:** Capture and update can be automatic; destructive deletion is never silent.
- **Portable data:** Export must be possible. Self-hosted GitHub sync uses readable Markdown/JSON artifacts alongside database snapshots.
- **Agent-neutral core:** Agent adapters translate vendor-specific hook payloads to one internal event schema.
- **No forced cloud dependency:** Cloudflare-managed sync is optional.
- **Low-friction:** Agents should receive memory via hooks/MCP automatically, rather than relying only on prompt instructions.

## 5. Users and Modes

### Individual developer

Uses one or more coding agents on local projects. Wants automatic continuity between chats and devices.

### Power user / self-hosted user

Does not want AutoMemory account login. Uses a private GitHub repository as the sync remote and owns all stored data.

### Managed SaaS user

Signs in to AutoMemory. Uses encrypted cloud sync, a dashboard, and automatic multi-device continuity.

### Agent adapter targets

Shipped: 20 agent surfaces via `automemory connect` / `adapter list` (Antigravity with native hooks; all others MCP-only). Only Antigravity installs lifecycle hook files; the rest join over MCP plus prompt nudges where supported.

Future adapters: any MCP-compatible coding agent follows the same registry pattern.

## 6. Functional Requirements

### 6.1 Project registry

The system maintains a global project registry. Each project has a stable identifier, a display name, one or more local root paths, optional source Git remote, sync configuration, and selected document patterns.

Project matching order:

1. Exact canonical local root path.
2. Git repository root path.
3. Git remote origin fingerprint.
4. User-approved alias or path mapping.
5. Unregistered project: do not silently create cloud records; present an opt-in registration flow.

A project can be active on multiple devices with different local paths while using the same stable project ID.

### 6.2 Global preferences

Global preferences apply to all projects unless overridden at project scope. Examples:

- Preferred programming languages or package managers.
- Preferred code style.
- Preferred deployment platforms.
- Token/context budget.
- Preferred test strategy.
- LLM provider configuration for extraction and consolidation.
- Sync defaults and privacy policy.

Preferences can be set through the UI, CLI, and MCP tools. Preference changes require a source and timestamp. The user can lock a preference so agents cannot modify it.

### 6.3 Project memory

Every project uses a logical namespace. A memory record belongs either to:

- A specific project.
- The global namespace.

Default retrieval in a project includes:

1. Global locked preferences.
2. Global active preferences and reusable patterns, limited by relevance.
3. Current-project active memories.
4. Current-project relevant docs and document summaries.
5. Relevant success traces and known failure records.

Default retrieval must not include another project's private decisions or raw session evidence.

### 6.4 Automatic event capture

Agent adapters send normalized events to the local AutoMemory service. Capture must not depend on an agent remembering to call a memory tool.

Events include:

- Session started.
- User prompt submitted.
- Tool requested.
- Tool completed successfully.
- Tool failed.
- Agent response completed.
- Task completed successfully.
- Task ended unsuccessfully or was abandoned.
- Subagent started/stopped.
- Context compaction started/ended.
- Session stopped.

The adapter must gracefully degrade where an agent does not expose a particular hook. For example, if `TaskCompleted` is unavailable, use a combination of response completion, task markers, and session stop to generate a task candidate.

### 6.5 Tool-call ledger

Every captured tool event becomes an immutable event record. It is not automatically a memory.

The event ledger stores:

- Event ID, session ID, task ID, project ID, agent adapter ID.
- Tool name and normalized tool category.
- Sanitized input and output references.
- Input/output hashes.
- Exit status, duration, timestamp.
- File/artifact references where discoverable.
- Sensitivity classification.

Raw payload policy is configurable:

- `hash_only`: store hashes and selected metadata only.
- `redacted`: store sanitized input/output snippets.
- `full_local`: store full local encrypted event payloads.
- `full_synced`: only if explicitly enabled; never default.

Secrets, credentials, access tokens, private keys, `.env` values, and configured sensitive patterns must be redacted before persistence or sync.

### 6.6 Memory extraction

Memory extraction runs synchronously in the hook process on Stop/PostCommit and in the `serve` background worker loop. It must stay fast (PreToolUse pipeline measured ~1.26 ms mean).

The local deterministic distiller (`TrajectoryDistiller`) receives event evidence and returns a `DistillationReport`: pruned-event counts, an optional synthesized success trace, an optional synthesized memory, and resolved failures. There is no SKIP/ADD/UPDATE/LINK/FLAG verdict enum; ambiguity and low-confidence output flow to the review-proposal queue and candidate memory tier instead.

Initial automatic extraction candidates:

- New architecture or implementation decision.
- Stable configuration fact.
- A repeatable command/workflow that succeeded.
- A root cause and applied fix for a failure.
- A user preference explicitly stated or strongly repeated.
- A project convention.
- A known limitation, warning, or unresolved risk.
- Material change in project state, such as migration/version/deployment state.

The extractor must avoid saving trivial noise such as routine formatting commands, repetitive test success without a notable finding, generic greetings, and unverified guesses.

### 6.7 Memory updates and supersession

AutoMemory must not only append. When new evidence conflicts with or refines an existing memory:

- Identify candidate memories using lexical, semantic, metadata, and entity similarity.
- Choose `UPDATE`, `SUPERSEDE`, `MERGE`, or `ADD`.
- Keep the prior version and provenance history.
- Mark previous content as `superseded`, not deleted.
- Record why the update happened and which evidence supports it.

Example:

- Old active decision: “Authentication uses 15-minute JWT expiry.”
- New verified evidence: configuration changed to 30 minutes.
- Result: old decision becomes superseded, new version becomes active, both link to the change event/session.

### 6.8 Success traces

A success trace is a structured, reusable procedure created after a task completes successfully. It answers: **what worked, how, in which environment, and how confident are we that it can be repeated?**

A trace includes:

- Task title and normalized task category.
- Preconditions and relevant environment facts.
- Ordered steps derived from actual tool events.
- Commands/tools used.
- Expected outputs or verification checks.
- Output artifacts and file references.
- Duration and timestamps.
- Reproducibility score.
- Known caveats and failure modes.
- Links to related memories, source task, and evidence events.
- Status: active, deprecated, invalidated.

When a similar task begins, the recall layer should retrieve relevant success traces before the agent performs costly exploration.

### 6.9 Failure records

A failure record is a structured record of what went wrong and, if known, how it was fixed.

It includes:

- Task title/category.
- Tool/action that failed.
- Error message or redacted error evidence.
- Root-cause hypothesis and confidence.
- Fix applied, if any.
- Verification result after fix.
- Links to events, files, and related success traces.
- Status: unresolved, resolved, obsolete.

Failure records should be retrieved when a new task resembles a prior failure or touches the same affected component.

### 6.10 Documents to sync

Users can specify selected documents and directories that contain durable context but may be ignored by the code repository.

Examples:

- `PRD.md`
- `docs/architecture.md`
- `docs/decisions/`
- `.cursor/rules/`
- `AGENTS.md`
- `CLAUDE.md`
- local design notes or handoff documents

The project manifest defines include paths and exclusions. Files are copied into AutoMemory’s project vault during sync; they do not need to be committed to the source repository.

The document system must:

- Preserve original relative paths.
- Hash contents and track revisions.
- Detect local deletion and retain previously synced version unless explicit deletion is requested.
- Generate optional document summaries and embeddings.
- Mark files containing secrets as blocked from sync unless explicitly allowed after warning.

### 6.11 Context assembly

At session start the system stays silent by default (pull principle); with `[defaults] boot_injection = true` it assembles a bounded context package for the current project. The package order is:

1. Locked global preferences.
2. Project identity, stack summary, and current state.
3. High-confidence project decisions and architectural facts.
4. Relevant user/project preferences.
5. Relevant warnings and unresolved failures.
6. Relevant successful procedures.
7. Relevant document excerpts and file references.
8. A compact “memory instructions” section explaining evidence confidence and source references.

The package must fit within a configurable token budget. Default target: 2,000 tokens for passive session boot, with optional task-aware expansion after the user prompt is received.

Retrieval modes:

- **Boot retrieval:** small, high-confidence context at session start.
- **Task retrieval:** after user prompt, search by task/query to add targeted context.
- **On-demand MCP retrieval:** agent explicitly requests context for a question, file, component, or procedure.

The agent must receive file references and memory IDs so it can inspect the source instead of trusting a compressed summary blindly.

### 6.12 Automatic sync

Automatic sync operates without requiring a user confirmation for ordinary memory writes. This is important for auto-approve coding workflows.

Recommended trigger policy:

- Queue local changes immediately.
- Debounce uploads for 30–90 seconds after tool/event activity.
- Sync at task completion when a high-value memory, trace, failure, preference, or selected document changed.
- Sync at session stop as a final checkpoint.
- Retry with exponential backoff if offline.
- Never block an agent task on sync failure.

The sync status must remain visible in CLI/UI. Local state is authoritative while offline; conflicts are resolved during sync.

### 6.13 Consolidation

Consolidation is a background maintenance process. It is not a normal task hook and should not run after every tool call.

Recommended default:

- Low-cost incremental deduplication after each task.
- Full consolidation once per day while the machine is idle, or when manually requested.
- Cloud-managed users may run consolidation server-side only if end-to-end encryption permits it; otherwise it runs on a trusted local device.

Consolidation actions:

- Detect and merge duplicates.
- Update freshness and confidence.
- Resolve contradictory memories through supersession.
- Link related memories and traces.
- Archive low-value, stale, or invalid records.
- Create review proposals for uncertain conflicts or potential deletion.
- Produce a maintenance report.

No hard deletion occurs automatically. Low-value memories are archived after a configurable retention period and can be restored.

## 7. MCP Product Specification

AutoMemory exposes MCP tools through both stdio and HTTP transports.

### 7.1 Transport modes

- **Stdio:** primary local integration for coding-agent clients.
- **HTTP:** local network/custom integrations and managed SaaS gateway.

The MCP API must have one common tool schema independent of transport.

### 7.2 Read tools

- `memory.recall` / `memory.ask`: Retrieve relevant project/global memories for a query (trust tiers 1–3 by default; `include_untrusted` opts into Tier 4–5).
- `memory.about`: Everything known about a file or symbol (graph entities, callers, linked memories).
- `memory.topics`: Active decision topics plus distinct tags (what is queryable).
- `memory.context`: Build context package for current project and task.
- `memory.get`: Read a memory, including versions and provenance.
- `memory.search`: Hybrid search over memories, traces, failures, and document summaries.
- `memory.list`: Filter memories by project, type, status, tag, date, or confidence.
- `memory.stats`: Return project/global memory health and counts.
- `memory.digest`, `memory.constitution`, `memory.skills`: Activity rollup, invariant set, distilled playbooks.
- `trace.list` / `trace.get`, `failure.list` / `failure.get`, `project.list` / `project.get`, `doc.list` / `doc.get`, `preference.get`, `sync.status`, `graph.query`, `graph.neighbors`, `graph.index`, `graph.impact`, `lock.status`, `decision.get`, `task.next`, `review.next`, `room.pending`, `room.status`, `baton.pickup` — as named.

### 7.3 Write tools

- `memory.save` (explicit save), `memory.update` (versioned), `memory.archive` / `memory.restore`.
- `memory.propose`: Candidate-tier write; rejected without provenance evidence.
- `decision.record` (supersedes prior decision on topic, keeps dissent), `lock.acquire` / `lock.release`, `task.create` / `task.submit`, `review.submit` / `review.post` (tests judge), `room.join` / `room.post` (turn-gated, evidence-checked), `baton.pass`.
- `trace.save`, `failure.save`, `failure.resolve`, `preference.set`, `preference.lock`, `project.register`.
- Not built: `memory.upsert`, `memory.feedback`, `trace.invalidate`, `project.configure`, `doc.track`, `doc.untrack`, `sync.push`, `sync.pull` (CLI `sync push/pull` and `doc track/list/compile/scan` cover these flows).

### 7.4 Governance tools

- `memory.proposals.list` / `memory.proposals.summary` / `memory.proposals.decide` / `memory.proposals.decide_all`, `memory.redact`.
- Not built: `memory.consolidate`, `memory.provenance`, `memory.export`, `memory.import` (covered by CLI `sync export/import` and `memory.digest`).

### 7.5 Agent access policy

MCP writes should be automatic when auto-capture is enabled, but explicit agent-originated writes must include a declared scope and confidence.

Policies include:

- Per-project read/write permission.
- Per-agent adapter permission.
- Allowed memory types for automatic writes.
- Whether raw evidence can leave the device.
- Whether the agent may set global preferences.
- Maximum context/token budget.
- Maximum retrieval results.

## 8. Hook and Adapter Design

### 8.1 Adapter responsibilities

Each agent adapter:

1. Installs the agent’s native hook configuration.
2. Receives vendor-specific event payloads.
3. Normalizes payloads into AutoMemory event format.
4. Determines current working directory/project identity.
5. Sends event to local AutoMemory daemon or queues it to disk.
6. Does not run expensive LLM extraction synchronously.

### 8.2 Out-of-process hooks

Hooks run as an out-of-process command. The command must be lightweight and resilient:

- Validate payload.
- Find project.
- Append normalized event to a local durable queue/ledger.
- Return quickly.
- Background daemon handles extraction, indexing, consolidation, and sync.

This prevents a slow LLM, database lock, or network outage from blocking the coding agent.

### 8.3 Antigravity adapter

`automemory connect agy` writes workspace `.agents/hooks.json` (PreInvocation/boot, PreToolUse, PostToolUse, Stop), root `.mcp.json`, and `.agents/plugins/automemory/`; `automemory init` writes a reference copy to `~/.automemory/hooks/`. Existing MCP configs are merged, never overwritten. There is no config backup and no `automemory disconnect` — removal means deleting the written files. Validate with `automemory doctor`.

Required minimum lifecycle behavior:

- Session start: identify project, queue sync pull, create session record, provide boot context.
- Post-tool success/failure: append evidence event.
- Stop/session end: finalize pending task candidates, enqueue final extraction and sync.

If a native `TaskCompleted` event is unavailable, AutoMemory uses an inferred task boundary and labels traces as `inferred` rather than `explicit`.

### 8.4 Future adapters

Claude Code, Cursor, Codex, Gemini CLI, OpenCode, and others must share the same normalized internal event schema. Adapter-specific capabilities are declared in metadata so the product can adapt capture behavior without falsely claiming unsupported hooks.

## 9. Internal Data Model

The initial local store is SQLite in WAL mode. Use migrations and a stable portable export format.

### Core entities

`projects`, `project_paths`, `sessions`, `tasks`, `tool_events`, `memories`, `memory_versions`, `memory_links`, `memory_embeddings`, `success_traces`, `failure_records`, `preferences`, `tracked_documents`, `document_revisions`, `review_proposals`, `audit_log`, `custom_guardrail_rules`, `procedural_skills`, `leases`, `decisions`, `batons`, `reviews`, `rooms`, `room_posts`, `graph_entities`, `graph_edges`. No `provenance_links`, `sync_operations`, or `agent_adapters` tables; provenance travels in memory metadata and version rows.

### Memory fields

Each memory includes at minimum:

- `id` (time-sortable UUID)
- `scope` (`global` or `project`)
- `project_id` (nullable for global)
- `type` (`fact`, `decision`, `lesson`, `pattern`, `warning`, `observation`, `preference`, `skill`)
- `content`
- `summary`
- `confidence`
- `importance`
- `freshness`
- `status` (`active`, `candidate`, `pinned`, `stale`, `changed`, `superseded`, `archived`, `invalid`, `review`)
- `source_kind` (`user`, `agent`, `tool_event`, `task_reflection`, `document`)
- `created_at`, `updated_at`, `last_recalled_at`
- `tags`, `entities`, `metadata`

### Memory versioning

A memory update creates a version record. Current active memory points to latest valid version. Versions are never overwritten in place.

### Retrieval indexes

MVP:

- SQLite full-text search (FTS5).
- Metadata filters by project/type/status/tags/date.
- Recency and confidence ranking.

Later:

- Local or remote embeddings.
- Hybrid lexical + semantic retrieval.
- Entity graph traversal.
- Reranking.

## 10. Retrieval and Ranking

The retrieval pipeline must balance relevance with safety and token budget.

### Candidate sources

- Current project memories.
- Global preferences and global reusable patterns.
- Success traces.
- Failure records.
- Tracked document chunks.
- Project state summary.

### Ranking signals

- Project scope match.
- Query lexical/semantic relevance.
- Type priority: locked preference, active decision, warning, success trace, fact, observation.
- Confidence and evidence strength.
- Freshness.
- User feedback on recall usefulness.
- Task category match.
- Diversity constraints to avoid repeated variants of one fact.

### Safety rules

- Do not inject raw secrets.
- Label uncertain or inferred memories.
- Prefer verified tool/document evidence over LLM-only inference.
- Include provenance IDs for critical claims.
- Avoid injecting an entire document if a short excerpt/reference is sufficient.

## 11. Sync and Data Portability

### 11.1 Managed Cloudflare mode

Managed mode provides encrypted multi-device sync.

Cloudflare responsibilities:

- Authentication and account/session management.
- Project registry and device registration.
- Encrypted object storage for event batches, document revisions, exports, and database snapshots.
- Metadata storage for sync versioning and conflict coordination.
- HTTP MCP gateway for remote access where enabled.

Cloudflare should not become the only durable data format. Users must be able to export all projects and preferences.

### 11.2 GitHub private repository mode

GitHub mode requires no AutoMemory login. The user grants their own token/credential and chooses a private repository.

Recommended vault layout:

```text
automemory-vault/
  global/
    preferences.json
    patterns/
  projects/
    <project-id-or-slug>/
      project.json
      docs/
        PRD.md
        architecture.md
        decisions/
      memories/
        active.jsonl
        archived.jsonl
      traces/
        success/
        failures/
      exports/
      snapshots/
  sync-log.jsonl
```

Do not rely solely on committing a live SQLite binary. SQLite can be included as an optional snapshot, but canonical Git sync artifacts should be text-based JSONL/Markdown where possible to allow diffing and conflict resolution.

### 11.3 Conflict strategy

- Event records are append-only and mergeable.
- Memory updates use version IDs and optimistic concurrency.
- Conflicting edits create a conflict record instead of silently choosing one for high-value memories/preferences.
- Automated low-risk fields such as `last_recalled_at` may use last-write-wins.
- Documents use normal text merge behavior; unresolved conflicts remain visible for user review.

## 12. Privacy and Security Requirements

- Never sync secrets by default.
- Redact known secret formats before local persistence where practical.
- Store local credentials in OS keychain/keyring, not plain project JSON.
- Use explicit consent before enabling full transcript sync.
- Provide per-project capture rules and sync exclusions.
- Support “local only” project mode.
- Support deleting local cached raw evidence while retaining sanitized derived memory.
- Managed mode must use encrypted transport and encrypted storage.
- If zero-knowledge encryption is offered, document recovery limitations clearly.
- All memory write/update/archive operations are recorded in an audit log.

## 13. User Experience

### First-run flow

1. Install CLI.
2. Select mode: Local only, GitHub vault, or Managed Cloudflare account.
3. Connect Antigravity (or another supported agent; see `adapter list`).
4. Register first project and select documents to track.
5. Confirm capture/privacy defaults (extraction is local deterministic code; there is no LLM provider to choose).
6. Run a diagnostic test that verifies hooks, event ingestion, and memory recall.

### Default capture policy

- Auto-capture evidence events: enabled.
- Auto-create low-confidence semantic memories from every tool event: disabled.
- Auto-create high-value memories/traces/failures after task/session analysis: enabled.
- Auto-update existing verified memory: enabled with version history.
- Auto-delete: disabled.
- Auto-archive stale low-value memory after retention period: enabled, recoverable.
- Auto-sync: enabled on task completion/session stop when remote configured.

### Dashboard sections

- Overview: active project, sync health, memory counts, recent activity.
- Projects: project map, paths, tracked docs, sync/capture settings.
- Memory browser: search/filter/edit/archive/restore/version history.
- Success traces: successful workflows and reproducibility details.
- Failures: root causes, fixes, unresolved issues.
- Preferences: global and project preferences, locks, overrides.
- Documents: tracked file revisions and sync health.
- Provenance: evidence timelines for memories/traces/failures.
- Review queue: conflicts, low-confidence records, archive/deletion proposals.
- Settings: providers, privacy, encryption, adapters, sync.

## 14. Recommended Defaults

| Area | Default |
|---|---|
| Storage | Local SQLite + append-only event queue |
| Agent integration | Antigravity adapter + MCP stdio |
| MCP transport | Both stdio and HTTP available; stdio enabled by default locally |
| Capture | All available tool/session events, sanitized |
| Extraction | Async and non-blocking |
| Provider | User-selectable; hybrid recommended when local model and API are available |
| Boot context budget | 2,000 tokens |
| Task-aware context budget | 4,000 tokens |
| Auto-sync | Debounced, task completion, and session stop |
| Full consolidation | Daily while idle plus manual command |
| Deletion | Never automatic hard delete |
| Stale data | Archive after configurable retention, default 30 days after review window |
| Project registration | Explicit opt-in for unknown projects |
| GitHub vault format | Text-first JSONL/Markdown plus optional DB snapshots |

## 15. MVP Scope

### Must-have

- Local SQLite store and project registry.
- Antigravity adapter with session start, post-tool, failure, and stop events where supported.
- Event queue/ledger and asynchronous worker.
- Project-scoped + global memory records.
- Preferences with global/project scope.
- Explicit MCP read/write/search/context tools over stdio and HTTP.
- FTS5 retrieval with metadata ranking.
- Memory versioning, supersession, archive/restore.
- Success traces and failure records.
- Selected document tracking and local vault copy.
- Local UI or minimal web dashboard.
- GitHub private-repository sync mode using text-first portable artifacts.
- CLI diagnostics and adapter installation.

### Deferred

- Cloudflare managed SaaS account system.
- End-to-end encrypted remote search.
- Neural/vector embeddings upgrade (current: local n-gram projection) and symbol-level code graph.
- Team workspaces and roles.
- Billing.
- Automatic test/deployment “breakage prediction.”

Shipped (no longer deferred): multi-agent adapters, knowledge graph (file-level), semantic vector retrieval (local), GitHub sync, local dashboard.

## 16. Acceptance Criteria

### New-chat continuity

Given a registered project with prior decisions, tracked docs, and success traces, when a user starts a new chat in that project, the agent receives no automatic context by default (pull principle); with `[defaults] boot_injection = true`, it receives a bounded project context without scanning the entire repository first.

### Project separation

Given two projects with distinct memories, when a session begins in Project A, Project B technical facts are not included in default context.

### Global preference

Given a global preference set in the UI, when a new session begins in any registered project, the preference appears in context unless a project override exists.

### Automatic capture

Given a successful task that includes tool events, when the task/session finishes, AutoMemory creates an event ledger and attempts asynchronous extraction without requiring an agent prompt such as “remember this.”

### Success trace

Given a repeatable successful workflow, AutoMemory produces a structured trace with ordered evidence-backed steps and makes it retrievable for a similar future task.

### Failure record

Given a failed tool call and later fix, AutoMemory links failure evidence, root-cause hypothesis, fix, and verification in a retrievable failure record.

### Memory update

Given new verified evidence contradicting an active memory, AutoMemory supersedes or updates the memory while preserving previous version and provenance.

### Offline resilience

Given a disconnected network, capture and retrieval continue locally; sync resumes after connectivity returns.

### GitHub ownership mode

Given self-hosted GitHub mode, user data is synchronized to the chosen private repository without requiring AutoMemory account credentials.

### Safe sync

Given a tracked document containing a detected secret, AutoMemory blocks sync by default and reports the reason.

## 17. Product Metrics

- Session-start context retrieval latency.
- Percentage of sessions receiving project context successfully.
- Hook event delivery success rate.
- Extraction queue latency.
- Memory precision feedback: useful vs stale/wrong recalls.
- Duplicate-memory rate before/after consolidation.
- Success-trace reuse rate.
- Failure recurrence rate after a resolved failure record exists.
- Sync success/failure and conflict rate.
- Percentage of active users enabling self-hosted vs managed sync.
- Local database size and archived-memory ratio.

## 18. Risks and Mitigations

| Risk | Mitigation |
|---|---|
| Too much noisy memory | Event ledger is separate from durable memory; extractor can SKIP; consolidation and feedback reduce noise |
| Incorrect LLM summaries | Preserve provenance, label confidence, expose source evidence, allow user correction |
| Hook support differs by agent | Capability-based adapters; degrade gracefully; never promise unsupported events |
| Hooks slow down agent workflows | Hooks only enqueue events; expensive work is asynchronous |
| Sync conflicts | Append-only events, versioned memory records, text-first vault artifacts, visible conflict queue |
| Secrets leak into memory/sync | Redaction pipeline, blocked file rules, local credential storage, explicit consent for raw payload sync |
| Vendor lock-in | Local store, MCP standard, portable export, GitHub self-hosting mode |
| SQLite binary conflicts in Git | Treat JSONL/Markdown as canonical sync format; use SQLite snapshots only as optional cache/backups |

## 19. Future Enhancements

- Neural embeddings upgrade; symbol-level AST extraction (tree-sitter or companion bridge).
- Codebase map integration to answer structural questions without full repository scan.
- Task planner that checks success traces and known failures before execution.
- Shared team/project memory with role-based access.
- Browser extension and IDE extension.
- Offline peer sync or self-hosted S3/MinIO backend.
- Automated PRD/ADR generation from validated memories and task traces.
- Policy engine for read/write permissions by agent, tool, project, and memory type.

---

# Appendix A — Normalized Event Contract

```json
{
  "event_id": "uuidv7",
  "event_type": "tool.completed",
  "timestamp": "ISO-8601",
  "agent": {
    "adapter_id": "antigravity",
    "agent_instance_id": "optional-agent-session-id"
  },
  "session_id": "uuidv7",
  "task_id": "optional-uuidv7",
  "project": {
    "id": "uuidv7",
    "root_path": "/local/path/to/project",
    "source_git_remote": "optional"
  },
  "tool": {
    "name": "shell",
    "category": "command",
    "input": "redacted-or-reference",
    "input_hash": "sha256",
    "output": "redacted-or-reference",
    "output_hash": "sha256",
    "status": "success",
    "exit_code": 0,
    "duration_ms": 832
  },
  "artifacts": [
    {"path": "docs/architecture.md", "kind": "file", "hash": "sha256"}
  ],
  "sensitivity": "redacted",
  "schema_version": 1
}
```

# Appendix B — Project Manifest Example

```json
{
  "schema_version": 1,
  "project_id": "018f0ef0-0000-7000-8000-000000000001",
  "slug": "my-saas",
  "display_name": "My SaaS",
  "paths": [
    "/Users/asmat/code/my-saas",
    "C:/Users/asmat/code/my-saas"
  ],
  "source_git_remotes": [
    "git@github.com:asmat/my-saas.git"
  ],
  "capture": {
    "enabled": true,
    "auto_approve_aware": true,
    "raw_event_policy": "redacted",
    "allow_agent_preference_writes": false
  },
  "context": {
    "boot_token_budget": 2000,
    "task_token_budget": 4000
  },
  "documents": {
    "include": [
      "PRD.md",
      "docs/architecture.md",
      "docs/decisions/**",
      ".cursor/rules/**",
      "AGENTS.md"
    ],
    "exclude": [
      "**/.env",
      "**/*.pem",
      "**/node_modules/**"
    ]
  },
  "sync": {
    "mode": "github",
    "enabled": true,
    "auto_push_on_task": true,
    "auto_push_on_stop": true,
    "vault_project_path": "projects/my-saas"
  }
}
```

# Appendix C — Success Trace Example

```json
{
  "id": "uuidv7",
  "project_id": "project-uuid",
  "title": "Deploy API release safely",
  "category": "deployment",
  "status": "active",
  "preconditions": [
    "Production environment variables are configured",
    "Main branch is clean"
  ],
  "steps": [
    {
      "position": 1,
      "action": "Run deployment script",
      "tool": "shell",
      "command": "./scripts/deploy.sh --environment production",
      "verification": "Command exits with code 0"
    },
    {
      "position": 2,
      "action": "Verify health endpoint",
      "tool": "http",
      "verification": "GET /health returns HTTP 200"
    }
  ],
  "reproducibility_score": 0.88,
  "caveats": [
    "Run database migration before traffic cutover when migration is required"
  ],
  "source_task_id": "task-uuid",
  "evidence_event_ids": ["event-1", "event-2"],
  "created_at": "ISO-8601"
}
```
