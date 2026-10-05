# Interactive Web Dashboard & REST API

AutoMemory includes a built-in, local-first web console and REST API daemon that runs out of the box without external web server installations.

---

## 🚀 Starting the Dashboard

Launch the background daemon and dashboard:
```bash
automemory serve --port 4545
```

Open your browser to:
```text
http://127.0.0.1:4545
```

---

## 🎨 Visual Features

The dashboard provides visual oversight into your coding agents' cognitive activity:
- **Review Proposals Queue**: Real-time human-in-the-loop review interface for autopilot command halts, deduplication merges, and memory updates.
  - **One-Click Bulk Actions**: Single-click **"Approve All"** and **"Reject All"** buttons that execute zero-SQL atomic transactions.
  - **KPI Metric Cards**: Real-time distribution across Pending, Approved, Rejected, and Total lifetime proposals.
  - **Type Breakdown**: Instant view of `duplicate_merge` vs. `tool_execution_review` proposals.
- **Memory Explorer**: Search, filter, and inspect active vs. archived memories, confidence scores, and tags.
- **AST Knowledge Graph**: Visualize indexed repository files, functions, structs, and caller relationships.
- **Procedural Skills Catalog**: Inspect synthesized reusable step-by-step playbooks generated from past trajectories.
- **Failure Catalog**: Review unresolved error patterns, root-cause hypotheses, and verified fix playbooks.

---

## 📡 REST API Reference

The local daemon exposes high-speed JSON REST endpoints on port 4545:

### Proposals
- `GET /api/proposals/summary`: Returns proposal counts grouped by status and proposal type.
- `GET /api/proposals?status=pending&limit=20`: Lists review proposals with pagination.
- `POST /api/proposals/:id/decide`: Approves or rejects a specific proposal.
  - Body: `{"approved": true}`
- `POST /api/proposals/decide_all`: Atomically approves or rejects all pending proposals.
  - Body: `{"approved": true, "proposal_type": "duplicate_merge"}`

### Memories & Search
- `GET /api/memories?query=auth&limit=10`: Searches active memories.
- `POST /api/memories`: Saves a new memory record.
- `DELETE /api/memories/:id`: Archives a memory record.

### Guardrails & System
- `GET /api/stats`: Ingest metrics and memory storage statistics.
- `GET /api/guardrails/rules`: Lists active critical and high-risk safety rules.
- `POST /api/guardrails/evaluate`: Evaluates an arbitrary shell command against active safety policies.
- `GET /api/mode`: Gets current autopilot mode (`vibe`, `strict`, `review`).
- `POST /api/mode`: Switches active autopilot safety mode.

---

## ⚡ WebSocket Live Pulse (`/ws`)

The dashboard connects to a lightweight WebSocket gateway at `ws://127.0.0.1:4545/ws`:
- **Real-Time Stream**: Pushes live memory insertions, tool executions, and guardrail halts directly into the web UI with sub-millisecond propagation.
- **Zero Polling**: Keeps client CPU usage minimal.
