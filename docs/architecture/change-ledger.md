# Change ledger: many agents, one codebase

**Status: design, not built.** Everything here is a proposal to be checked by the experiments in section 9. Claims from outside sources are marked *(reported)*: they come from project READMEs, blogs or preprints we have not reproduced.

**Decision (Nov 2026): Tracewise is the recorder.** AutoMemory reads it through an optional link (`history.rs`, `automemory who <file>`); without `tw` the answer is empty and nothing else changes. Still open: lock ownership when both are installed (AutoMemory owns locks), shared actor and session ids, and showing "changed by" next to a stale memory.

**Overlap to settle before building:** Tracewise (a separate tool, `tw`) already records who changed every line, when and why, and undoes by line. The snapshotter and ledger below should use it as the recorder instead of a second one, which leaves this design with the worktree boundary, the integration order and the notifier. Open question for section 9: does Tracewise's record carry enough (author, reason, task) to be the ledger?

Goal: several agents (and several people) work on one repository at the same time, and
1. no agent's work silently overwrites another's,
2. nobody does the same work twice,
3. every agent knows what the others did and are doing, and
4. when an agent dies, another continues from where it stopped.

This replaces "file locks taken by hooks" as the main mechanism ([locking](locking.md) stays as the shared-folder fallback).

## 1. What goes wrong (failure modes)

| # | Failure | Example | Why locks and hooks alone do not stop it |
|---|---|---|---|
| F1 | **Textual conflict** | Both edit `src/auth.rs` lines 40-60 | A lock blocks the second agent for the whole turn, or is ignored |
| F2 | **False conflict** | A adds `fn a()` at the end of a file, B adds `fn b()` at the end | Line-based git conflicts although the changes are independent *(reported: conflicts on any file two agents touch, even different functions)* |
| F3 | **Semantic conflict** | A renames a function, B calls the old name in a new file. Each branch passes its tests alone | No textual overlap, so nothing flags it. *(Reported: a 2026 benchmark, "Passes Alone, Fails Together", studies exactly this; CodeCRDT estimates 5-10% of concurrent edits stay semantic conflicts even with zero merge failures.)* |
| F4 | **Duplicate work** | Both implement the same helper | *(Reported: 78% duplicate work with no coordination, 64% with advisory locks only, 0% with locks plus shared completion state, in a 32-agent experiment; one blog summary of a preprint.)* |
| F5 | **Stale view** | B works for an hour on a base that no longer has A's change or A's decision | Nothing tells B that the world moved |
| F6 | **Invisible writer** | An agent edits through a shell command or a CLI without hooks | Hooks see only their own agent's edit tools |
| F7 | **Lost work** | An agent crashes mid-task | The claim stays "claimed"; no hand-off is written |
| F8 | **Several people, several machines** | Four people, six agents | Local files and SQLite are not shared |

## 2. Principles

1. **Isolate, then integrate.** Each agent works in its own git worktree on its own branch. Collisions cannot happen while editing; they are handled once, in one place, at integration.
2. **Git stays the source of truth for code.** No new version-control system: every agent and tool already speaks git.
3. **Nobody "wins" an edit; the order of readiness decides.** The first change that is ready (checks green) integrates; the later one adapts to it. A human can override the order.
4. **The ledger records intent, not just diffs.** Every change carries the task, the agent, the user and a one-line reason, so others can judge relevance.
5. **Tests are the judge for semantic conflicts.** Only an executed check can reveal F3. We do not try to read code and guess.
6. **Hard guarantees live where tools cannot skip them:** the worktree boundary, git hooks and the integration step. Anything else is advice.

## 3. Components

```
 agent A worktree ─┐                      ┌─ prompt hook / MCP  → "what changed for you"
 agent B worktree ─┼─ Snapshotter ─▶ LEDGER ─▶ Notifier ────────┤
 agent C worktree ─┘  (private refs)   │  ▲                     └─ dashboard (people)
                                       ▼  │
                                  Integrator (merge queue)
                          trial merge → driver chain → checks → fast-forward main
```

| Component | Job |
|---|---|
| **Worktree manager** (`automemory run <agent> --task T`) | Creates or reuses `../wt/<task>` on branch `am/<task>`, records the base commit, claims the task, starts the agent there. Keeps the worktree of a dead agent. |
| **Snapshotter** | Saves the worktree state as a commit under a private ref `refs/automemory/<agent>/<session>` whenever files changed (a file watcher, with hook events as hints; debounced; changed paths only). Each snapshot has trailers: agent, session, task, user. Captures edits made by any CLI because it looks at files, not tools. Refs are local, never pushed by default, honour `.gitignore`, and pass the secret check. |
| **Ledger** | One authority per project, in SQLite locally and in a coordinator for teams (section 7). Stores tasks, claims, change records and their states. |
| **Integrator** | Runs the queue: trial merge, merge-driver chain, checks in a temporary worktree, fast-forward of the integration branch. |
| **Notifier** | Turns ledger changes into short, relevant messages for each agent (section 5). |
| **Attribution bridge** | Writes and reads line-level attribution in the git-ai notes format (`refs/notes/ai`) when present. Optional. |
| **Recovery** | Detects an orphaned claim (no heartbeat) and starts a new agent from the last snapshot (section 6). |

## 4. Question 1: two agents change the same file. Who wins?

Neither overwrites the other. Their work lives on separate branches. What happens depends on how the two changes relate, resolved by a ladder that stops at the first rung that works:

| Rung | Case | Action | Who acts |
|---|---|---|---|
| 0 | Different files | Merge, run checks | automatic |
| 1 | Same file, different regions | Plain git 3-way merge | automatic |
| 2 | Same file, git conflicts but the changes are independent (both add a function at the end, imports, reordered items) | A **structural merge driver** (tree-sitter based; Mergiraf-style or entity-level like Weave) resolves it | automatic. *(Reported: Weave 31/31 clean merges vs git 15-17/31, Mergiraf 28/31, all self-reported benchmarks.)* |
| 3 | Same function or entity edited by both | Conflict goes to the **later** agent, with the other change's task, intent and diff. The later agent resolves it in its own worktree and re-submits. The earlier change is never touched. | later agent |
| 4 | The later agent fails twice, or both changes are marked high-risk | Escalate: a human decides, or a short planning room if the human wants one | human |
| 5 | Merge is clean but checks fail after merge (F3) | Eject the later change from the queue, give its agent the failing check output and the earlier change | later agent |

**Order ("who is later").** The queue serves changes in the order they become *ready* (task submitted and its own check green). The human can set priority per task, which overrides readiness. This is the same idea as a gating merge queue: *(reported: Zuul tests queued changes speculatively assuming the ones ahead merge, and retests without a change that fails.)*

**Prevention before the conflict.** A task declares its scope (files, and later symbols). At claim time:
- Overlapping scopes between two open tasks produce a warning to the planner (agent or human): split the work, serialize it, or accept the risk.
- Interface changes (a public symbol's signature or name) are flagged on the change record; the notifier tells every task whose scope references that symbol (callers from the codebase-memory server).

**Hot files** (lockfiles, schema files, generated files, `Cargo.toml`): marked in config. They are changed through a single queue slot at a time (an advisory reservation), because structural merge does not help there.

## 5. Question 2: how does each agent know what the others did?

Three channels, from most to least reliable. Every message is short, scoped to the agent's task, and has a token cap.

| Channel | When | Content |
|---|---|---|
| **Pull at known points** (hooks) | session start, before each prompt, before claiming, before submit | "Since you started: *merged:* B finished task T7 (added `parse_args`, changed `cli.rs`, 2 files, check green). *In flight:* C is changing `db.rs` (task T9). *Affects you:* your scope uses `open_db`, which T9 changes." |
| **Pull on demand** (MCP tools) | any time | `work.status` (tasks, claims, states), `work.changes {since, scope}`, `work.conflicts` (open conflicts for me) |
| **Human view** (dashboard) | any time | Board of tasks and claims, change timeline per agent, queue, conflicts |

**What counts as relevant to an agent:** the change touches a file in its scope, a file in its scope's directory, a symbol its scope calls, or a decision/convention the task references. Everything else is left out; a one-line count ("12 other changes merged") is the most we say about the rest.

**Change record** (the unit that is shown):

| Field | Meaning |
|---|---|
| id, task, agent, session, user | who and what for |
| base, head | the commits the change sits between |
| files, symbols | what it touched (symbols when the language is supported) |
| intent | one line, taken from the task title or the agent's submit message |
| state | in-flight, ready, queued, merged, ejected, orphaned |
| checks | command, result, time |
| conflicts | other change ids and the rung reached |

**Keeping the base fresh (F5).** When the integration branch moves, the ledger tells affected agents and offers `work.sync` (rebase or merge the worktree onto the new base). It is **never automatic mid-edit**: it runs only when the worktree has no uncommitted changes in affected files, or when the agent asks. This keeps the window of "working on an old base" short without rewriting files under an agent.

**Agents without hooks.** They still get the rules text telling them to call `work.status` at the start of a task and before submitting, and the dashboard shows the same state to the person. We accept that these agents are served less well and say so.

## 6. Failure and recovery (F7)

- A claim has a **heartbeat**: any tool event, snapshot or MCP call renews it. After the limit (default 10 minutes) with no sign of life the task becomes *orphaned*.
- The ledger has the progress at the last snapshot: files, branch, last check result, and the agent's last note. Another agent claims the orphaned task and **starts from that snapshot's worktree**, so the work is not redone.
- Attempts are counted. After two failed attempts the task waits for a human.
- The worktree and branch of a dead agent are kept until the task is done or cancelled.
- Test: start an agent on a task, `kill -9` it, check that a second agent receives the partial progress and finishes the task.

## 7. Several people and machines (F8)

Two kinds of state need two different guarantees:

| State | Needs | Mechanism |
|---|---|---|
| The integration branch (`main`) | Exactly one winner per update | **The git remote itself.** A fast-forward push is an atomic compare-and-swap: the first push wins, the second is rejected and rebases. No extra service is needed for this. |
| Claims, task states, change records, heartbeats | Strong consistency (two people must not claim one task) | A **coordinator** with a single writer per project: a small service (one Cloudflare Durable Object per project fits `workers/sync-gateway`) or the local SQLite when working alone. Git or a vault cannot do this: they are eventually consistent, so two claims made at the same moment would both succeed. |
| Notes, decisions, briefs | Eventual consistency is fine | The existing encrypted git-vault sync |

Rules: identity has three parts (user, agent, session). Only people approve; an agent never approves. Offline, an agent works from cached state and cannot claim shared tasks. Snapshot refs can be pushed to a team remote under `refs/automemory/...` when the team chooses, so a teammate's agent can see in-flight work; they are not pushed by default.

## 8. Options considered

| Option | Verdict |
|---|---|
| Keep file locks as the main mechanism | Rejected as the main one: blocks a whole turn, ignored by hook-less tools, cannot see shell writes |
| One shared directory with live merging (CRDT editing, as in CodeCRDT and AgentRoom) | Rejected for now: needs agents to write through our editor, not their own tools; *reported* 5-10% semantic conflicts remain; AgentRoom's authors conclude coordination matters more than merging |
| A new version-control system | Rejected: no agent or tool would adopt it; git already does what we need |
| Jujutsu as the engine (auto-snapshots, operation log, conflicts stored in commits, workspaces) | Attractive. Keep as an optional backend later; requiring it would cut the audience |
| Worktrees only (no ledger) | The baseline to beat in the benchmark. It stops F1-F2 while editing but not F3-F8 |
| git-ai or Entire for history | Use alongside (read their refs, write the same format); they record after the fact and do not coordinate live work |
| **Chosen:** worktrees + snapshots + ledger + merge queue with a merge-driver chain, git remote as the main-branch arbiter, coordinator for claims | Adopt, subject to the experiments below |

## 9. Experiments before building (each cheap, each can kill the design)

| # | Question | Method | Pass |
|---|---|---|---|
| X1 | Is a snapshot cheap enough? | Watcher plus private-ref snapshot on this repo and a large one | Under 150 ms per snapshot at the 95th percentile; no visible slowdown |
| X2 | How many conflicts does the structural driver remove? | Replay 20 real two-agent edits (or synthetic) through plain git, Mergiraf and Weave | At least 60% fewer conflicts than plain git, zero wrong merges in the sample |
| X3 | Do semantic conflicts (F3) get caught by checks after merge? | Plant 10 semantic conflicts (rename + new caller) | At least 8 of 10 ejected with the failing output |
| X4 | Does the notice change agent behaviour? | Two real agents, tasks with overlapping scope, with and without the prompt-hook notice | Duplicate work and conflicts fall; tokens per notice under 300 |
| X5 | Does recovery work? | Kill an agent mid-task, 10 times | At least 8 of 10 finished by the second agent without redoing the first agent's files |
| X6 | Do four people and six agents stay consistent? | Script: 4 simulated users, 6 agent processes, mock coordinator, random kills | No task claimed twice, `main` never in a broken state, every orphan recovered |
| E0 | Is any of this better than worktrees alone? | The benchmark arms: none, worktrees only, + ledger, + ledger and queue | Clear gain in verified tasks per agent-hour over "worktrees only" |

Metrics to track: conflict rate per merge, false-conflict reduction, time from conflict to resolution, duplicate-work rate, orphan recovery rate, notice tokens per session, snapshot cost.

## 10. Build order

| Step | Delivers |
|---|---|
| Spike (X1-X3) | Snapshotter, trial merge, driver chain on this repo |
| S2 | Task states open, claimed, orphaned, done; heartbeat; attempts |
| S3 | Ledger and snapshotter; change records; `work.changes` |
| S5 | Prompt-hook notice; `connect` for the three agents |
| S6 | `automemory run` (worktrees), integrator queue |
| E0 | Benchmark gate |
| S11 | Coordinator and multi-user test (X6) |

## Sources
[Weave (entity-level merge driver)](https://github.com/Ataraxy-Labs/weave), [Mergiraf](https://www.emergentmind.com/topics/mergiraf), [saddle: structural merge before bouncing conflicts to agents](https://github.com/brandonapol/saddle/issues/330), [CrewAI: semantic merge for parallel changes](https://github.com/crewAIInc/crewAI/issues/4562), [Zuul gating](https://zuul-ci.org/docs/zuul/9.4.0/gating.html), [Jujutsu conflicts](https://git.joshthomas.dev/mirrors/jj/raw/branch/main/docs/conflicts.md), [CodeCRDT](https://arxiv.org/pdf/2510.18893), [AgentRoom](https://www.alphaxiv.org/abs/2608.23740), [Passes Alone, Fails Together](https://arxiv.org/pdf/2609.25396), [duplicate-work study summary](https://codex.danielvaughan.com/2026/06/20/before-the-pull-request-multi-agent-coordination-codex-cli-grite-duplicate-work-prevention/), [three agents editing one file](https://codex.danielvaughan.com/2026/06/06/multi-agent-coordination-problem-concurrent-agents-semantic-conflicts-codex-cli/), [git-ai](https://github.com/git-ai-project/git-ai), [Entire CLI](https://github.com/entireio/cli).
