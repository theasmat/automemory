w# Core Cognitive Engine & Features

AutoMemory is engineered from the ground up in native Rust to provide high-speed, local-first persistent memory for AI coding agents.

---

## 1. Native Rust Architecture (<5ms Latency)

Unlike traditional Python or Node.js memory wrappers that introduce hundreds of megabytes of dependencies and significant startup latency, AutoMemory runs as a single, compiled binary:
- **Zero Interpreter Overhead**: Hook pipeline measured ~1.26 ms mean (`cargo bench -p automemory-core --bench perf`; PRD budget is 5 ms p95).
- **Single-Writer SQLite**: One `Arc<Mutex<Connection>>` in WAL mode — no pool, no lock contention by construction.
- **Embedded Static Assets**: Built-in Axum HTTP server serves the pre-compiled Nuxt dashboard directly from memory without runtime filesystem dependencies.

---

## 2. Hybrid Retrieval with Temporal Exponential Decay

AutoMemory combines full-text BM25 search with mathematical time-decay scoring to balance historical relevance against freshness:

$$\text{FinalScore} = \text{BM25Rank} \times e^{-\lambda \cdot \Delta t} \times \text{ConfidenceScore}$$

- **Temporal Decay ($\lambda$)**: Older decisions gradually yield to recent revisions unless reinforced by explicit usage.
- **Contradiction Detection**: At recall time, memories naming a different version of the same tool than current context are flagged (generic `name version` matching; ports and years excluded).
- **Deterministic FTS5 Indexing**: ~0.56 ms mean recall over 500 memories (measured, see [OVERVIEW](OVERVIEW.md) §5).

---

## 3. Trajectory Distillation

Coding agents produce noisy interaction transcripts: exploratory directory listings, trial-and-error edits, and redundant status checks. AutoMemory applies multi-stage trajectory distillation (distilling a 40-event session measures ~0.11 ms; no fixed compression ratio is claimed):

```text
Raw Interaction Trajectory (100+ Events)
  ├── 1. Noise Filter (Strip read-only probes, idempotent status checks)
  ├── 2. Structural Diff Slicing (Isolate functional code edits)
  ├── 3. Verification Pinning (Identify test passes & build validations)
  └── 4. Procedural Synthesis
        ▼
Compact Reusable Skill Playbook (1 Concise Procedure)
```

The resulting skills are indexed into `memory.skills` and resurfaced when similar tasks are executed.

---

## 4. Self-Healing Failure Catalog

When an agent encounters a broken build, failing test, or runtime crash:
1. **Error Interception**: AutoMemory captures the tool invocation, CLI command, and stderr stream.
2. **Root-Cause Hypothesis**: Automatically tags error patterns with hypothesized causes.
3. **Playbook Synthesis**: Once the agent or human applies a successful fix, the solution is linked via `failure.resolve`.
4. **Proactive Warning**: Next time an agent attempts a command matching a known unresolved gotcha, AutoMemory warns the agent *before* the command executes.

---

## 5. Hot Memory Constitution & Token Budgets

LLM context windows are precious and expensive. AutoMemory never dumps unbounded historical transcripts into the agent prompt:
- **Token-Capped Synthesis**: Assembles a compact *Hot Memory Constitution* strictly respecting token limits (e.g., 2,000 tokens).
- **Prioritization Tiering**:
  1. Critical project architectural invariants and preferences.
  2. Unresolved gotchas and known failure patterns.
  3. High-confidence recent decisions.
  4. Most-relevant distilled skills.

---

## 6. File-Level Code Knowledge Graph

AutoMemory indexes codebase structure without language servers:
- **File & Doc Entities**: Indexes files and documentation with SHA-256 content hashes; symbol-level extraction arrives via the `CodeIntel` port.
- **Blast-Radius Impact Analysis (`graph.impact`)**: Given a modified file or symbol, traces indexed dependents. Results are only as deep as the indexed `imports` relations.
- **Zero-Language Server Protocol (LSP) Dependency**: Fast static graph generation without requiring active language server daemons.

---

## 7. Zero-SQL Governance Invariant

To eliminate database corruption, lock contention, and manual SQL hazards, AutoMemory enforces a strict **Zero-SQL Rule**:
- Agents and human operators interact strictly via high-level MCP tools (`memory.proposals.*`) or CLI subcommands (`automemory proposal ...`).
- Raw `sqlite3 ~/.automemory/automemory.db` shell commands are actively intercepted and blocked by the guardrail engine.
- Batch proposal decisions and duplicate-merge archival run atomically within single-transaction locks.
