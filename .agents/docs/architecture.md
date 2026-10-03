# AutoMemory — Technical Architecture

**Status:** Draft v0.1  
**Audience:** Coding agents, engineers, maintainers  
**Scope:** Local runtime, MCP server, hooks/adapters, event pipeline, sync, and data model.

---

## 1. System Overview

AutoMemory consists of:

1. **Local runtime daemon** — persistent memory store, event queue, extraction worker, retrieval engine, and sync coordinator.
2. **MCP server** — exposes memory, project, preference, trace, failure, document, and sync tools over stdio and HTTP.
3. **Agent adapters** — out-of-process hook handlers that normalize vendor-specific agent events into a common schema.
4. **Sync backends** — GitHub private repository (self-hosted) and Cloudflare-managed SaaS (optional).
5. **CLI and UI** — diagnostics, project management, memory browser, and configuration.

All components must work offline. Sync is optional and additive.

---

## 2. Local Runtime Architecture

### 2.1 Components

- **Event Ingestor:** receives normalized events from adapters, appends to append-only event ledger.
- **Event Ledger:** immutable, append-only record of all captured events.
- **Extraction Worker:** asynchronous worker that processes queued events and produces memory/trace/failure candidates.
- **Memory Store:** SQLite database with migrations, FTS5 indexes, and versioned records.
- **Retrieval Engine:** hybrid lexical + metadata retrieval, ranking, and context assembly.
- **Consolidation Worker:** background maintenance worker for deduplication, supersession, archiving, and review proposals.
- **Sync Coordinator:** manages push/pull operations, conflict detection, and sync metadata.
- **Document Vault:** local copy of tracked documents and their revisions.
- **Audit Log:** tamper-evident log of writes, updates, archives, deletes, and sync operations.

### 2.2 Data Flow

1. Agent adapter receives vendor event.
2. Adapter normalizes and sends to Event Ingestor.
3. Event Ingestor validates, enriches with project/session/task IDs, and appends to Event Ledger.
4. Extraction Worker processes events asynchronously, creates/updates memories, traces, failures, and document references.
5. Retrieval Engine serves MCP read/context/search requests.
6. Consolidation Worker runs periodically to maintain data quality.
7. Sync Coordinator pushes/pulls changes to remote vault.

---

## 3. MCP Server

### 3.1 Transport

- **Stdio:** primary for local coding agents.
- **HTTP:** local network/custom integrations and managed SaaS gateway.

### 3.2 Tool Categories

- **Read:** `memory.recall`, `memory.context`, `memory.get`, `memory.search`, `memory.list`, `memory.stats`, `trace.list`, `trace.get`, `failure.list`, `failure.get`, `project.list`, `project.get`, `doc.list`, `doc.get`, `preference.get`, `sync.status`.
- **Write:** `memory.save`, `memory.update`, `memory.upsert`, `memory.archive`, `memory.restore`, `memory.feedback`, `trace.save`, `trace.invalidate`, `failure.save`, `failure.resolve`, `preference.set`, `preference.lock`, `project.register`, `project.configure`, `doc.track`, `doc.untrack`, `sync.push`, `sync.pull`.
- **Governance:** `memory.proposals.list`, `memory.proposals.decide`, `memory.consolidate`, `memory.provenance`, `memory.export`, `memory.import`, `memory.redact`.

---

## 4. Agent Adapters and Hooks

### 4.1 Adapter Responsibilities

- Install vendor hook configuration.
- Receive vendor-specific payloads.
- Normalize to AutoMemory event schema.
- Determine current project identity.
- Send event to Event Ingestor.
- Never run expensive LLM extraction synchronously.

### 4.2 Hook Lifecycle Events

- `SessionStart` / `PreInvocation`
- `PreToolUse`
- `PostToolUse`
- `PostToolUseFailure`
- `TaskCompleted`
- `Stop` / `PostInvocation`
- `SubagentStart` / `SubagentStop`
- `PreCompact` / `PostCompact`
- `UserPromptSubmit`
- `Notification`

### 4.3 Out-of-Process Hook Design

Hooks run as separate commands that:

- Validate payload.
- Find project.
- Append normalized event to durable queue.
- Return quickly.
- Background daemon handles extraction, indexing, consolidation, and sync.

---

## 5. Data Model

### 5.1 Core Entities

- `projects`
- `project_paths`
- `sessions`
- `tasks`
- `tool_events`
- `memories`
- `memory_versions`
- `memory_links`
- `provenance_links`
- `success_traces`
- `failure_records`
- `preferences`
- `tracked_documents`
- `document_revisions`
- `sync_operations`
- `review_proposals`
- `agent_adapters`

### 5.2 Memory Fields

- `id` (time-sortable UUID)
- `scope` (`global` or `project`)
- `project_id` (nullable)
- `type` (`fact`, `decision`, `lesson`, `pattern`, `warning`, `observation`, `preference`, `skill`)
- `content`
- `summary`
- `confidence`
- `importance`
- `freshness`
- `status` (`active`, `superseded`, `archived`, `invalid`, `review`)
- `source_kind` (`user`, `agent`, `tool_event`, `task_reflection`, `document`)
- `created_at`, `updated_at`, `last_recalled_at`
- `tags`, `entities`, `metadata`

### 5.3 Indexes

- FTS5 on `content` and `summary`.
- Metadata indexes on `project_id`, `type`, `status`, `confidence`, `freshness`.
- Compound indexes for common queries (project + type + status).

---

## 6. Retrieval and Context Assembly

### 6.1 Retrieval Pipeline

1. Candidate selection by project scope, type, status, tags, date.
2. Lexical scoring (FTS5).
3. Metadata scoring (confidence, freshness, importance).
4. Diversity constraints.
5. Token budget enforcement.

### 6.2 Context Assembly

1. Locked global preferences.
2. Project identity and stack summary.
3. High-confidence decisions and facts.
4. Relevant preferences and patterns.
5. Relevant warnings and failures.
6. Relevant success traces.
7. Document excerpts and references.
8. Memory instructions and provenance references.

---

## 7. Sync Architecture

### 7.1 Eventual Consistency

- Append-only events merge trivially.
- Versioned memories use optimistic concurrency.
- Conflicts create review records.
- Documents use text merge behavior.

### 7.2 GitHub Vault Format

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

### 7.3 Cloudflare Managed Mode

- Authentication and account/session management.
- Project registry and device registration.
- Encrypted object storage for event batches, document revisions, exports, and database snapshots.
- Metadata storage for sync versioning and conflict coordination.
- HTTP MCP gateway for remote access where enabled.

---

## 8. Security and Privacy

- Redaction of secrets before persistence/sync.
- Local credential storage in OS keychain.
- Explicit consent for raw payload sync.
- Per-project capture rules and sync exclusions.
- Local-only project mode.
- Audit log of all write/update/archive/delete/sync operations.

---

## 9. Tech Stack

| Component | Technology |
|---|---|
| Local CLI/Daemon | Rust |
| MCP Server | Rust (stdio + HTTP) |
| Agent Adapters | Rust (out-of-process hooks) |
| Cloudflare Worker | TypeScript |
| Frontend (Managed Mode) | Nuxt 4 (CF Pages) |
| Database (Local) | SQLite (WAL mode) |
| Database (Managed) | Cloudflare D1 + R2 |
| LLM (Local) | Ollama (Llama 3 8B) |
| LLM (Cloud) | Claude 3.5 Sonnet / GPT-4o |

---

## 10. Future Enhancements

- Semantic vector retrieval.
- Knowledge graph.
- Codebase map integration.
- Task planner with trace/failure checks.
- Agent performance dashboard.
- Team workspaces and roles.
- Browser/IDE extensions.
- Offline peer sync or self-hosted S3/MinIO backend.
- Automated PRD/ADR generation.
- Policy engine for read/write permissions.