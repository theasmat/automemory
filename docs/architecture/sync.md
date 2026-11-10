# Sync: the same experience on another session or machine

Sync v2 (signed ops, device registry, revocation, scrub and purge, compaction) is described in [specs/sync-v2.md](../specs/sync-v2.md).

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
