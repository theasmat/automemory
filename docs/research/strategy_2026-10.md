# Product strategy: AutoMemory (Oct 2026)

**Stage:** working MVP, one user (the maintainer), no outside users yet. **Method:** 9-section Product Strategy Canvas, grounded in our own data, the [opportunity tree](opportunity_solution_tree.md), the [ecosystem study](multi_agent_ecosystem_needs.md) and a web scan on 2026-10-22. Labels: **fact** (measured or checked), **hypothesis** (to be tested).

## 0. Where we are (verified)

| Fact | Value |
|---|---|
| Surface | 49 MCP tools, 45 CLI commands, 13 dashboard pages, ~52k lines of Rust |
| Quality | 305 Rust tests and every script suite pass (`tests/run_all.sh`, 2026-10-22) |
| Real use (maintainer's database before the wipe) | 14 active notes, all recalled at least once; 1,230 tool events over 39 sessions; 12 hand-offs; 5 planning rooms; 0 locks held at that moment |
| Value proof | Not measured. The memory-off vs on eval (X3) and the live trials T10, T12, T13 have not run |
| Outside demand | None measured. No interviews, no external installs |

What the market looks like now (web scan, secondary sources, verify before quoting):
- **Shared memory over MCP is a commodity.** One directory lists 324 memory MCP servers. Several target exactly our first pitch: memorix, cairntir (local SQLite), mcp-memory-service, Mem0 and Supermemory (hosted).
- **A near-identical competitor exists.** [ai-memory](https://github.com/akitaonrails/ai-memory) (Aug 2026) is a self-hosted Rust service. It auto-installs hooks and MCP for each harness, hands work between Claude Code, Codex and Command Code, and has a run launcher and an event ledger.
- **Vendors are adding memory inside their own walls.** Anthropic curates memory between sessions in Managed Agents and runs several Claude Code sessions as Agent Teams. These features are single-vendor.
- **The pain that is measurable is parallel work, not memory.** One reported study (32 agents) found duplicate work at 78% with no coordination, 64% with advisory locks only, and 0% with locks plus shared completion state. Unique tasks per round went from 2.33 to 8.00. This is from a blog summary of arXiv 2606.19616, so check the figures before using them. Worktrees are the standard isolation, and coordination on top of them is the open problem.

**Conclusion:** "memory for coding agents" cannot win; it is crowded and vendors absorb it. The defensible product is the thing nobody else has yet: **cross-vendor coordination of parallel agents with verified shared state**. Most of its parts already exist in our code (locks, tasks with claims, hand-off brief, trust tiers, git-based staleness, the human gate).

## 1. Vision
**Run five agents on one codebase and get five agents' worth of work.** No duplicate work, no overwrites, no agent acting on something that is no longer true, whatever vendor each agent comes from.

## 2. Target segments

| Segment | Size (hypothesis) | Pain | Today's alternative | Priority |
|---|---|---|---|---|
| A. Power developers running 2+ agents in parallel (Claude Code plus Codex, Antigravity or Cursor), often in worktrees | Growing fast; the early majority of agent users by 2027 | High: lost hours to duplicate or conflicting work and re-explaining | Worktrees plus manual task splitting, AGENTS.md, copy-paste hand-offs | **Primary** |
| B. Small teams (2-10) where each person brings their own agent | Large | High at merge time: two people's agents redo or undo each other | PR review, Slack, per-vendor team features | Second (after A works) |
| C. Single-agent users | Largest | Low: vendor memory and CLAUDE.md are enough | Built-in memory | **Not served** |
| D. Enterprises needing SSO, audit export, policy | Large, slow | Medium | Vendor enterprise plans | Not now |

## 3. Pain and value

| Pain (segment A) | Cost today | Value we create |
|---|---|---|
| Two agents take the same task or file | Duplicate work (reported 64-78%), overwritten edits, merge conflicts | One claim per task and per file, visible to every vendor; collisions stopped before the edit |
| The next agent does not know what was decided, ruled out or already done | Re-explaining, re-proposing rejected ideas, redoing finished steps | The brief and the completion state are injected at session start with provenance |
| An agent acts on a stale note ("we use X" when X was removed last week) | Silent wrong work | Notes are checked against git and labelled verified / changed / stale; only trusted notes are told as fact |
| An agent marks work "done" that is not | Rework, broken main | Done means the task's check command passed, not the agent's claim |

## 4. Value propositions
- **Segment A:** when I run several agents at once, I want each to pick up a different piece and know what the others did and decided, so that I get more finished work per hour instead of babysitting collisions.
- **Segment B:** when my teammate's agent and mine work in the same repo, I want our agents to see each other's claims and decisions, so that we do not undo each other and only human-approved knowledge spreads.

## 5. Strategic trade-offs

| We choose | Over | Because |
|---|---|---|
| Coordination of parallel work (claims, completion, collisions) | Being a general memory store | Memory is commodity; coordination across vendors is unsolved and measurable |
| Cross-vendor neutrality | Deep integration with one vendor | Vendors will solve their own single-vendor case |
| Verified state (git checks, tests as judge, human gate) | More recall features (hybrid vectors, LLM summaries) | Research says unverified memory rarely helps; trust is the differentiator |
| A small surface (about 12 MCP tools, 4 views) | Feature breadth (49 tools, 13 pages) | Every tool costs context tokens and setup trust; O7 (start in minutes) is our largest self-made risk |
| Local-first single binary, optional hosted team sync | Cloud-first | Code and memory stay on the machine; the team tier is the business |
| Proof before features | Building the roadmap | Nothing is measured yet |

**Say no to (for 12 months):** the library (L6 and beyond), room types and roles (R5-R7), LLM helper expansion, embedding provider work, more `connect` targets without a live trial, a Cloudflare service before segment A retains.

## 6. Key metrics
- **North star:** *verified tasks per agent-hour*: unique tasks whose check passed, divided by agent time, across all agents on the project (goodput).
- **Inputs:**
  1. Duplicate-work rate (tasks or files done twice) — target under 5%.
  2. Collisions caught before the edit / all collisions — target over 90%.
  3. Hand-off accuracy: the next agent answers "goal, ruled out, next step" correctly from the brief (the T12 checker) — target 3 of 3.
  4. Stale-note precision — target over 90%.
  5. Time from install to the first coordinated two-agent task — target under 10 minutes.
- **Health:** hook p95 under 50 ms (release), false-block rate under 1%, tokens injected per session under 1,500, week-4 retention of outside users.

## 7. Growth engine
1. **The benchmark is the launch.** Reproduce the duplicate-work experiment with real agents and our tool (off / locks only / locks plus shared completion). Publish the harness and the numbers ("3x goodput with five agents"). This is content, proof and a recruiting tool for contributors at once. The harness exists (`tests/eval/`).
2. **One-line install, value in the first session.** `connect` already writes everything. Add `automemory run <agent> --task` (worktree plus claim) so the first thing a user does is the thing we are best at.
3. **Team loop.** The shared encrypted vault means one teammate's install invites the next: their agents see your claims only if they install. The paid tier is hosted team sync plus an audit view.
4. **Open source core.** The local binary stays free and open; trust in a tool that sees all agent activity requires it.

## 8. Core capabilities (architecture changes)

| Capability | Build / reuse | Investment | When |
|---|---|---|---|
| **Work ledger as the core primitive**: tasks, claims, completion with check results, decisions; notes become evidence attached to these, not a separate world | Reuse `tasks`, `leases`, `decisions`, `tool_events`; reshape the model around them | M | Now |
| **Worktree-native runs**: `automemory run <agent> [--task id]` creates or reuses a worktree, claims the task, starts the agent; claims and leases work across worktrees (project resolution already does) | Build (the deliberately unplanned "layer 5") | M | Now |
| **Completion by check**: a task is done only when its `check_cmd` exits 0; agents see what is done and by whom | Reuse room tasks (`check_cmd`, `check_status`) | S | Now |
| **Small agent surface**: about 12 MCP tools (`work.next/claim/done`, `lock.*`, `memory.recall/save`, `decision.record`, `brief`, `resume`, `status`), stateless MCP (spec 2026-07-28) | Fold the rest behind CLI and dashboard | M | Now |
| **Verifier**: git-based staleness on notes and decisions, symbol hash via codebase-memory | Reuse | S | Keep |
| **Cross-vendor readers**: Claude Code and Antigravity verified, others after a real sample | Reuse | S | Keep, do not extend |
| **Dashboard reduced to four views**: Work (board of tasks and claims across agents), Agents (who holds what), Inbox (approvals), Settings | Rework the 13 pages | M | Next |
| **Hosted team sync** (`workers/sync-gateway`) | Reuse | M | After segment A retains |

## 9. Defensibility (honest)
Today we have **none that a fast follower cannot copy**: ai-memory already ships most of the memory and hand-off surface. What can become a moat:
1. **Proof and brand:** the first published, reproducible benchmark of multi-vendor parallel agents, and being the reference for "how to run agents in parallel".
2. **Neutral position:** a vendor cannot credibly coordinate its competitors' agents.
3. **Accumulated verified state:** decisions and task history that are checked against the code become costly to leave (switching cost grows with use).
4. **Team network effect:** within a team, value rises with each member who installs.

## Strategic risks
1. **Value is not real or small.** If the benchmark with real agents shows little gain over worktrees plus a shared task file, the product is a nice-to-have. Test it first; it is cheap.
2. **Vendors converge on a shared protocol** (for example an A2A or MCP extension for task claims) and make the coordination layer a standard feature. Mitigation: implement the standard first and stay the best neutral implementation.
3. **Hook and MCP churn** across four vendors breaks the experience faster than one maintainer can fix. Mitigation: a small surface, contract tests, MCP-only fallback.

## Next steps (in order)
1. **E0 benchmark (1-2 weeks):** 10 real tasks, 3 agents in parallel, three arms (no coordination, locks only, locks plus tasks and completion), 3 runs each. Metrics: duplicate work, collisions, verified tasks per agent-hour, tokens. Decide the pitch from the result.
2. **E3 demand (parallel):** give the build to 5 developers who already run 2+ agents. After a week: who still uses it, and which part. Interview them.
3. **Cut to the core** (only after E0 confirms): the "say no" list above goes behind a feature flag or is removed; MCP tools down to about 12; dashboard down to four views.
4. **Build** `automemory run` with worktrees and completion by check.
5. Re-run this canvas with the E0 and E3 data.

Sources: [cross-agent-memory topic](https://github.com/topics/cross-agent-memory), [324 memory MCP servers](https://mcpservers.org/category/memory), [ai-memory](https://github.com/akitaonrails/ai-memory), [Code w/ Claude SF 2026 recap](https://www.claude.com/blog/code-w-claude-sf-2026-sf), [Agent Teams guide](https://timewell.jp/en/columns/claude-code-agent-teams-guide), [duplicate-work study summary](https://codex.danielvaughan.com/2026/06/20/before-the-pull-request-multi-agent-coordination-codex-cli-grite-duplicate-work-prevention/), [Tembo multi-agent workflows](https://www.tembo.io/blog/multi-agent-ai-coding-workflows), [Morph: worktrees](https://www.morphllm.com/claude-code-git-worktree), [survey on persistent agent memory](https://arxiv.org/pdf/2606.30306).
