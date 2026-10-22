# Automatic locking

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

**Status:** layers 1, 2, 3, 4 and 6 and the coverage metric are built (`tests/scenarios/locking_v2.py`); layer 5 (worktree launcher) is deliberately not planned. Measure the result with `automemory locks --coverage` after a week of real use; the target is ≥ 80% covered or reported.

**Order and gates**

| Step | Delivers | Verified by |
|---|---|---|
| 1 (layer 1 + 6) | Multi-turn leases, idle expiry, policy | `tests/scenarios/collision.py` gains a multi-turn case; unit tests for idle expiry |
| 2 (layers 2 + 3) | Shell writes and unknown writes covered | New guardrail-style table of shell commands with expected write targets; scenario where agent B's `sed -i` on A's file is reported |
| 3 (layer 4) | Alerts for agents without hooks | Dashboard scenario; manual check with Cursor or Cline |
| 4 (layer 5) | Optional hard isolation | End-to-end: two launched agents, no collision possible, overlap report at merge |

Decide before step 2: whether reconciliation may *deny* the next call of the offending session in Strict mode, or only warn. Decide before step 4: ship the launcher inside v1, or leave it to Phase 7 with the headless Runner.
