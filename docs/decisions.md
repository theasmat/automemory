# Decisions

Settled choices and why. Change one only on new evidence, and edit this file when you do. Open questions are in the [roadmap](roadmap.md).

## Scope
- v1 is Claude Code, Codex and Antigravity (Cursor has a transcript reader), the planning room and the Nuxt dashboard. More agents and the hosted service come later.
- `connect` supports those plus a generic `connect other --mcp-file <path> --name <agent>` for any tool that reads an MCP servers file. Targets without a connect/disconnect check in `tests/scenarios/agents_matrix.py` were removed.
- Code symbols belong to the external `codebase-memory` server. AutoMemory builds no code graph.

## Trust and safety
- The daemon makes no LLM call unless the user configures `[llm]` (off by default). An LLM only suggests (Inbox proposals, candidate notes, brief drafts); it never writes trusted memory, takes a lock or decides a guardrail. LLM text is untrusted input: strict JSON, length caps, proposals only.
- The author of a shell write is worked out, never typed: a person is proven by a terminal or the password; otherwise the nearest known agent in the process chain names it (`actor.rs`, one table line per agent), then agent-only environment variables, then `shell`. An agent can pose as another agent this way but never as a person. The same applies to `automemory note` and `memory save`: from an agent's shell they are unverified tier 4 notes labelled with its name, not trusted text exported into instruction files.
- "human" must be proven. `automemory decision record` defaults its author to human, and an agent's shell tool runs it too, so without a terminal on stdin or the human password in the environment the record is stored as written by `shell`: kept, never exported into AGENTS.md as a person's decision, never shown as decided by a human (found in the first live run, where every decision Claude recorded through the CLI read "Decided by: Human").
- Agents never approve: only the human verdict, behind the password, stores a decision. A human note is never evidence. Agents cannot overwrite a person's brief or library item; they propose.
- The human password is asked every time for protected actions and never stored. It is the only boundary that survives an agent with a shell, so keychain entries and config files never gate anything.
- Agents never touch the database with raw SQL (the Zero-SQL rule); they use MCP tools and CLI commands.
- Identity comes from the MCP entry written by `connect`, not from a caller-chosen name ([identity](architecture/identity.md)).
- Dashboard permissions are decided by effect, not by name: Safe (read-only or local and undoable), Protected (creates trusted knowledge, loosens safety or sends data off the machine; password every time), Terminal only ([dashboard control](architecture/dashboard-control.md)). A Safe endpoint must be harmless in an agent's hands, because an agent with a shell can call it.

## Memory and recall
- Keyword recall (FTS5 with trust tiers, recency and a token budget) is the default; hybrid vector recall is opt-in and adopted only if `embeddings eval` beats keyword. No vector database, no graph store.
- Embeddings never run in hooks. Cloud use is off by default (`allow_network`; a `private` project can forbid cloud profiles). Provider choices and keys are never synced.
- Keys live in the OS keychain through the `keyring` crate; the encrypted file is only the fallback where no keychain exists. A secret never appears in config, the database, logs, hand-offs, instruction files or the vault.
- Notes written by agents that are never recalled fade after 60 days (half rank, a quarter after 120) and are offered for archiving. A person's notes, pinned notes and decisions never fade.
- Skills from trajectories are off by default (`[skills] auto = false`); existing skills are still listed and synced.
- Memory quality follows the Nov 2026 evidence ([the report](research/reports/AutoMemory%20memory%20system%20improvements.md)): measure first (labelled recall set, then coding tasks), then change. A claim needs numbers; vendor benchmark scores are not evidence.
- What is stored is governed, not maximised: notes derivable from the repository are refused or demoted, and the managed instruction block stays near 100 lines. Most failures of memory in coding benchmarks are pollution and staleness, not ranking.
- Trust tiers cap how many result slots lower tiers may take; they are not score weights (additive provenance weights measured no effect). **Changed Nov 2026:** another agent's unchecked note (tier 4) used to be withheld from default recall, so agents on one long task could not read each other's findings. It now takes part in the capped slots (a third of the limit, at least one), labelled with who wrote it and that nobody checked it; it is never returned as a fact. Tier 5 stays quarantined, and `trust_policy = "strict"` restores the old withholding. Reason: shared memory that agents cannot read is not shared; the cap and the label are what the poisoning studies found effective.
- Time decay is not trusted as a score multiplier until an ablation on the eval set says so; staleness comes from verification against code first.
- No graph store, no reranker on the hook path, no LLM-driven add/update/delete. Vectors stay opt-in and are adopted only where the eval set shows a gain.

- **Locks and file awareness moved to Tracewise (Nov 2026, built in P1 to P3).** AutoMemory sees only agent hooks; Tracewise sees every writer, keeps data per repository (no cross-project locks) and releases locks at Stop. The two share one registered hook: AutoMemory's, which calls `tracewise hook --neutral` inside and merges the answers. Without Tracewise, AutoMemory has no locking. Command guardrails stay in AutoMemory. Plan: [specs/tracewise-companion.md](specs/tracewise-companion.md).

## Coordination
- Locks are taken automatically by the first edit and tied to the session; the worktree launcher (hard isolation) is not planned.
- Planning rooms: turns and evidence are the two rules that make them worth having. No free-form agent chat, no votes, no rooms that write code, at most three rounds. A review is not a one-round room because a review carries a test verdict a room does not.

## Continuity and library
- The brief (goal, rejected ideas, open questions, next steps) is the unit of hand-off; chat history syncs only when asked, and reading an agent's chat files is opt-in per agent, local, with secrets removed.
- Agent documents live in `.automemory/library/`, synced to all your computers by default through the encrypted vault; files can also be kept in place from the global config whether or not git tracks them.

## Sync
- Git-delta sync is the only path; the legacy text vault and the cloud client were removed. `sync pair` keeps the recovery phrase.
- Code travels by git, never by AutoMemory. Vectors are not synced.
- Each device signs its ops; the recovery phrase stays the only thing needed to enrol a computer. A lost computer is cut off by rotating the vault key (`sync devices revoke`), and the old phrase then stops opening new data. See [specs/sync-v2.md](specs/sync-v2.md).
- Sync never lets a copy that is not person-authored replace a person's note, and agents cannot change, archive or restore one over MCP. Locked preferences are not synced (they stay on their machine).
- No clock, CRDT or logical clock is needed: the merge compares fingerprints against the last synced version.

## Surface (after consolidation)
AutoMemory is seven things: Notes, Hand-off, Library, History, Coordination, Sync, Settings. Nothing a user relies on disappears silently: a removed MCP tool answers "was removed, use …". The evidence is in [research/consolidation_audit.md](research/consolidation_audit.md). Counts live in [architecture](architecture/overview.md) and are checked against the code.

## Not recommended
A vector or graph database, agent-to-agent chat beyond rooms, LLM-judged consensus, auto-approving anything an agent proposes, a hosted sync service before the local store stays small.
