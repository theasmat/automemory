# Model Context Protocol (MCP) Tool Reference

AutoMemory implements the Model Context Protocol (MCP) over standard input/output (`automemory mcp`) and HTTP (`http://127.0.0.1:4545/mcp`). 

Below is the complete reference of all **26 official MCP tools**.

---

## 1. Memory Operations

### `memory.context`
Assembles a bounded, token-capped context package containing active preferences, recent architectural decisions, known gotchas, and distilled skills.
- **Parameters**:
  - `project_id` *(optional, string)*: Project slug or ID.
  - `token_budget` *(optional, integer)*: Maximum token limit for the context package (default: 2000).

### `memory.recall`
Queries memories relevant to a specific user task or prompt.
Returns trust tiers 1–3 only; Tier 4–5 agent claims are withheld unless asked for.
- **Parameters**:
  - `query` *(required, string)*: Search keywords or task prompt.
  - `project_id` *(optional, string)*: Project scope filter.
  - `limit` *(optional, integer)*: Max results to return (default: 5).
  - `include_untrusted` *(optional, boolean)*: Also return Tier 4–5 claims (default: false).

### `memory.search`
Full-text FTS5 and semantic search across all memory records.
- **Parameters**:
  - `query` *(required, string)*: Search query.
  - `limit` *(optional, integer)*: Maximum results.

### `memory.save`
Explicitly persists a durable memory record (architectural decision, user preference, or project fact).
- **Parameters**:
  - `summary` *(required, string)*: Concise one-line summary.
  - `content` *(required, string)*: Detailed explanation and rationale.
  - `type` *(optional, enum)*: `fact`, `decision`, `lesson`, `pattern`, `warning`, `preference`, `skill`.
  - `scope` *(optional, enum)*: `global`, `project`.
  - `confidence` *(optional, float)*: Confidence score between 0.0 and 1.0.
  - `tags` *(optional, array of strings)*: Searchable concept tags.

### `memory.update`
Updates an existing memory record, creating an audited revision entry with reason.
- **Parameters**:
  - `id` *(required, string)*: Target memory ID.
  - `summary` *(required, string)*: Updated summary.
  - `content` *(required, string)*: Updated content.
  - `reason` *(optional, string)*: Reason for modification.

### `memory.archive`
Soft-deletes a memory record so it is excluded from active retrieval.
- **Parameters**:
  - `id` *(required, string)*: Memory ID to archive.

### `memory.restore`
Restores an archived memory record back to active status.
- **Parameters**:
  - `id` *(required, string)*: Memory ID to restore.

### `memory.get`
Retrieves full details and revision history for a specific memory record.
- **Parameters**:
  - `id` *(required, string)*: Memory ID.

### `memory.list`
Lists active memories with optional project and type filters.
- **Parameters**:
  - `project_id` *(optional, string)*: Project filter.
  - `limit` *(optional, integer)*: Max results.

### `memory.stats`
Retrieves database metrics, active/archived counts, and memory storage health.
- **Parameters**: None.

### `memory.skills`
Lists auto-distilled procedural skill playbooks synthesized from verified success traces.
- **Parameters**:
  - `project_id` *(optional, string)*: Project filter.

### `memory.constitution`
Retrieves the token-capped Hot Memory Constitution containing core rules, architectural invariants, and active project preferences.
- **Parameters**:
  - `project_id` *(optional, string)*: Project filter.

### `memory.redact`
Tests secret redaction patterns on sensitive text, masking API keys, JWTs, and private tokens.
- **Parameters**:
  - `text` *(required, string)*: Input text to redact.

---

## 2. Traces & Failure Post-Mortems

### `trace.save`
Persists a verified step-by-step reproducible execution procedure.
- **Parameters**:
  - `title` *(required, string)*: Trace title.
  - `category` *(required, string)*: Functional category.
  - `steps` *(required, array of objects)*: Action, tool, command, position, verification.

### `trace.list`
Lists successful multi-step execution trajectories.
- **Parameters**:
  - `project_id` *(optional, string)*: Project filter.

### `trace.get`
Retrieves step-by-step details for a specific trace.
- **Parameters**:
  - `id` *(required, string)*: Trace ID.

### `failure.save`
Records an error pattern with root-cause hypothesis.
- **Parameters**:
  - `task_title` *(required, string)*: Context or task name.
  - `error_message` *(required, string)*: Error or stderr message.
  - `failed_tool` *(optional, string)*: Tool that triggered the error.
  - `root_cause_hypothesis` *(optional, string)*: Suspected root cause.

### `failure.list`
Lists known failures with optional resolution status filter.
- **Parameters**:
  - `status` *(optional, enum)*: `unresolved`, `resolved`.

### `failure.get`
Reads a failure record and its resolution details.
- **Parameters**:
  - `id` *(required, string)*: Failure ID.

### `failure.resolve`
Marks a failure as resolved with verified fix and post-mortem notes.
- **Parameters**:
  - `id` *(required, string)*: Failure ID.
  - `fix_applied` *(required, string)*: Description of fix.
  - `verification_result` *(required, string)*: Test or verification outcome.

---

## 3. Governance & Proposal Resolution (Zero-SQL)

### `memory.proposals.summary`
Returns aggregate counts of proposals grouped by status (`pending`, `approved`, `rejected`) and proposal type (`duplicate_merge`, `tool_execution_review`).
- **Parameters**:
  - `project_id` *(optional, string)*: Project filter.

### `memory.proposals.list`
Lists review proposals with optional filtering and pagination.
- **Parameters**:
  - `status` *(optional, enum)*: `pending`, `approved`, `rejected`, `all`.
  - `proposal_type` *(optional, string)*: Filter by type.
  - `limit` *(optional, integer)*: Max results (default: 20).
  - `offset` *(optional, integer)*: Pagination offset.

### `memory.proposals.decide`
Approves or rejects an individual proposal, automatically triggering required side-effects (e.g. archiving duplicate memory records).
- **Parameters**:
  - `id` *(required, string)*: Proposal ID.
  - `approved` *(required, boolean)*: `true` to approve, `false` to reject.

### `memory.proposals.decide_all`
Atomically approves or rejects all pending proposals matching optional filters within a single transaction.
- **Parameters**:
  - `approved` *(required, boolean)*: `true` to approve all, `false` to reject all.
  - `proposal_type` *(optional, string)*: Filter (e.g., `duplicate_merge`).
  - `project_id` *(optional, string)*: Project filter.

---

## 4. Codebase Knowledge Graph (Tree-Sitter)

### `graph.index`
Scans a project workspace and parses AST entities into the local graph database.
- **Parameters**:
  - `project_id` *(required, string)*: Project identifier.
  - `path` *(required, string)*: Local workspace root directory path.

### `graph.query`
Queries code structural entities (files, functions, structs, classes, modules, traits).
- **Parameters**:
  - `name_pattern` *(optional, string)*: Substring match on symbol name.
  - `entity_type` *(optional, enum)*: `file`, `function`, `struct`, `class`, `module`, `trait`, `doc`.
  - `path_prefix` *(optional, string)*: Path prefix filter.

### `graph.neighbors`
Retrieves relationships and neighboring AST entities for a specific symbol or file.
- **Parameters**:
  - `entity_id` *(required, string)*: Target entity ID.

### `graph.impact`
Performs blast-radius impact analysis for modifying a file or symbol, tracing dependent callers across the repository.
- **Parameters**:
  - `target` *(required, string)*: Symbol name or relative file path.
  - `depth` *(optional, integer)*: Max traversal depth (default: 3).

> **Graph depth note:** the built-in indexer records file-level entities with SHA-256 plus `imports` edges; symbol-level entities arrive via `graph.index` companions or `ingest_graph_data`. `callers()` returns indexed dependents — real, but only as deep as the indexed relations.

---

## Multi-Agent Coordination Tools

### `memory.ask`
Alias for `memory.recall` with PRD naming: ranked short entries with ids, status, trust tier, and provenance. Tiers 1–3 only unless `include_untrusted` is set.

### `memory.about`
Everything known about a file or symbol: graph entities, callers, and linked memories (tier-filtered like recall).
- **Parameters**: `target` *(required)*, `project_id`, `include_untrusted`.

### `memory.topics`
What is queryable: active decision topics plus distinct memory tags.

### `memory.propose`
Writes a **candidate**-tier memory. Rejected without provenance.
- **Parameters**: `content`, `summary`, `evidence` *(all required)*.

### `memory.constitution`
Token-capped Hot Memory Constitution: core rules, architecture invariants, gotchas.

### `memory.skills`
Auto-distilled procedural skill playbooks from verified success traces.

### `lock.acquire` / `lock.release` / `lock.status`
Cooperative leases on file, glob, or symbol scope with TTL and heartbeat. Required for acquire: `scope`, `goal`. Conflicts return holder, goal, and remaining TTL.

### `decision.record` / `decision.get`
Record a decision with rationale, alternatives, and dissent (supersedes prior on topic); fetch active decision plus history. Required for record: `topic`, `decision`, `reason`.

### `task.create` / `task.next` / `task.submit`
Enqueue work with spec and symbol scope; claim oldest-open FIFO (or resume); submit hands to review (owner session only).

### `review.submit` / `review.next` / `review.post`
Queue a diff; fetch the oldest pending diff by another session; post a verdict. `tests_passed` is required — failing tests reject regardless of vote.

### `room.pending` / `room.join` / `room.post` / `room.status`
Planning-room turns: list actionable rooms; join or create on a topic; post propose/critique/revise (turn holder only, evidence IDs must resolve); fetch room with posts.

### `baton.pass` / `baton.pickup`
Sub-400-token ledger-grounded handoff packets between sessions (direct or broadcast pickup).

Full machine-readable schemas: call MCP `tools/list` (e.g. `echo '{"jsonrpc":"2.0","id":1,"method":"tools/list"}' | automemory mcp`).
