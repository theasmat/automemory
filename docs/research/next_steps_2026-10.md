# What to add next (research snapshot, Oct 2026)

Snapshot used to rank work in October 2026. Open items now live in [../roadmap.md](../roadmap.md); X1 (skills write path), X7 (stale MCP servers) are done.

Inputs: our own database and trials, [memory_improvement.md](memory_improvement.md) (benchmarks), and a short web
check on four topics. Web sources are mostly blogs and aggregators; where a claim matters it is marked **verify** (check the primary
source before building on it).

## 1 What we saw
| Signal | Source | Meaning |
|---|---|---|
| The trial database holds **6,794 procedural skills with 4 names and 29 distinct step lists** ("Add format_price function" ×2500), 216 traces with 4 titles; 18 MB for 14 active notes | `~/.automemory` (today) | The distiller still inserts a copy per run instead of updating one. Wasteful, slows sync, and any skill list shown to agents is noise |
| Boilerplate notes fell from 76% to 29%; 12 of 14 notes have been recalled once | same, vs the Oct 18 audit | Write-path gate and recall tracking work; skills and traces were not fixed |
| Memory systems rarely beat memory-off; verified experience and **selection** do | VibeMemBench, SWE-ContextBench (see research file) | Measure value before adding recall features |
| MCP spec **2026-07-28** removes protocol sessions and the `initialize` handshake (stateless, per-request `_meta`), reworks Tasks, deprecates Roots, Sampling and Logging | [blog summary](https://blog.mcpservers.org/posts/mcp-spec-2026-07-28), [changelog mirror](https://modelcontextprotocol.net/specification/2026-07-28/changelog) (**verify** on modelcontextprotocol.io) | Hosts will move to it; our stdio and `/mcp` servers answer `initialize` today |
| Git **worktrees** are the standard isolation for parallel agents; coordination on top is a shared task list, a mailbox and claim locks (Claude Code "agent teams", experimental) | [Augment guide](https://augmentcode.com/guides/git-worktrees-parallel-ai-agent-execution), [Morph](https://www.morphllm.com/claude-code-git-worktree) | Our locks, tasks and rooms cover coordination; we do not help create or clean worktrees |
| **SKILL.md** (agentskills.io, Dec 2025) is read by 30+ tools; AGENTS.md is near universal | [noqta](https://www.noqta.tn/en/news/skill-md-open-standard-30-ai-coding-tools-adoption-2026) (**verify** counts) | We write skills only for Antigravity; verified knowledge could reach every agent as a skill |
| Trial T11/T11b: a day-old agent MCP server ignored new rules; a "test" post spent a turn; agents did unrequested housekeeping on a broad prompt | `tests/manual/TRIAL_LOG.md` | Upgrades, input floors and narrow prompts matter more than features |

## 2 Ranked proposals
| # | Proposal | Why now | Size | Done when |
|---|---|---|---|---|
| X1 | **Fix the write path for skills and traces**: one skill per (name, steps hash) updated in place (`success_count++`), traces deduped by signature, `automemory memory prune` merges the 6,794 into ~30 (reversible, backup first), distiller refuses empty or generic names | Biggest measured defect; makes sync, recall and the Brain view useful | M | Trial DB shrinks to tens of skills; a rerun adds none; before/after in the trial log |
| X2 | **MCP 2026-07-28 compatibility**: accept stateless requests (no `initialize`, protocol version in `_meta`), return `resultType`, keep answering the old handshake; test against both | Hosts will switch; breaking silently loses every agent | M | Contract tests for both protocol versions pass; `doctor` reports the version a host used |
| X3 | **Value proof**: run the eval harness (N1) on 10 real tasks, off vs on, publish medians; add "memory off" switch per project for fair runs | Research says most memory does not help; we must show ours does or cut it | M | A table in FEATURES.md with dates and agent versions |
| X4 | **Worktree helper**: `automemory worktree add <task>` (branch per task, copies env files, registers the path so locks and overlap checks know it), cleanup on task submit, claim-to-worktree in the Planning room task flow | Where practitioners already are; turns tasks into isolated work | M | Two agents claim two tasks and edit the same file in separate worktrees; overlap check warns before merge |
| X5 | **Skills export (SKILL.md)**: write verified decisions, conventions and good procedures as `.agents/skills/` / `.claude/skills/` folders, refreshed like instruction files | Reaches 30+ tools through one format; reuses `export-rules` machinery | S | A skill written for one agent is read by a second; disconnect removes only ours |
| X6 | **Memory validity and history**: `valid_from` / `invalid_at` on decisions and notes; Revise (room) and supersede set `invalid_at` instead of hiding; "what changed since" view | Forgetting and contradiction are where systems fail (research file §2) | M | Superseded decision stays readable with dates; recall never returns an invalid note |
| X7 | **Upgrade hygiene**: `connect`/`update` print which agents hold an old MCP server (doctor check exists); dashboard banner "Restart Antigravity (server 44 h old)" | T11b lost an hour to it | S | Banner appears after an install and clears after restart |
| X8 | **Narrow-task prompts in the rules text**: say "do only what you were asked; never commit, resolve failures or pick up batons unless told" in the connect guidelines | Trial: Antigravity committed 16 files unasked | S | Same neutral prompt in the trial no longer commits |
| X9 | **Hook-level adapters for Cursor, Windsurf, Gemini CLI, Copilot** from recorded payloads (I2 tooling) | Biggest reach gap; needs real captures, not guesses | M | One fixture per agent; agent leaves "experimental" |
| X10 | **Rooms: only if X3 shows value**: R2 (edit plan steps), then R3 (task state); park R4–R7 | Rooms worked in T11/T11b but cost tokens and need proof | M | A/B on 5 design tasks: room vs one agent plus tests |

Order: X1 and X7/X8 first (cheap, visible, fix real defects), X2 before the next MCP-host release, X3 in parallel (it decides X10), then X5, X4, X6, X9.

## 3 Not recommended
- A vector database or graph store: retrieval is not the bottleneck (research, our data).
- Agent-to-agent chat beyond rooms, voting, or LLM-judged consensus: cost and blurred responsibility.
- Auto-approving anything an agent proposes: the human verdict is the product's trust boundary.
- Cloud sync service before X1 (a bloated store would sync bloat).
