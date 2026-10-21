# Discovery plan: what AutoMemory can become

Oct 2026. Stage: existing product, one real user (the maintainer), no outside users yet. Built from the repo (docs, trials T0-T11b, memory audit,
opportunity tree in [opportunity_solution_tree.md](opportunity_solution_tree.md)). Ideas, scores and thresholds are my judgement, not data.
**Discovery question:** which one sentence should AutoMemory be, and for whom, so a stranger installs it for that reason alone?

## Ideas explored (what the parts we already built could be)

| # | Idea | Who | Built today | Why it might matter |
|---|---|---|---|---|
| 1 | **Switch agents without re-explaining** (continuity brief + chat reading) | developers using 2+ agents | yes (C1-C8) | strongest pain, nothing cross-vendor found |
| 2 | **Shared project brain for a team** (decisions and brief travel by encrypted git; a teammate's agent inherits them) | small teams | sync, shared_by, team pins | memory is worth more when shared; teams pay |
| 3 | **Seatbelt for agents** (blocks destructive commands and secrets, human password, audit trail) | anyone letting agents run commands, security-minded teams | yes | fear is a strong buyer; easy to explain |
| 4 | **Agent traffic control** (locks, collision warnings, worktree helper for parallel agents) | people running 3+ agents at once | yes (locks), worktree helper planned | parallel agents are the trend; collisions are real |
| 5 | **Decision records that write themselves** (approved room or decision becomes an ADR in the repo) | teams, open source maintainers | rooms + decisions, export planned (R5) | durable artifact outside any tool, no lock-in |
| 6 | **Project hand-over for humans** (contractor or new hire gets "what happened, what was ruled out, what's next") | agencies, onboarding | resume pack + brief | same engine, non-agent buyer |
| 7 | **Flight recorder** (replay, cost and value report per task) | leads who pay for agent usage | replay, value report | budget owners need proof of what agents did |
| 8 | **Linter for agent instructions** (detect stale CLAUDE.md / AGENTS.md, verified vs changed notes) | any repo with rule files | export-rules, stale labels | tiny, instantly useful, viral inside repos |
| 9 | **Hosted team vault** (SaaS sync, SSO, audit) | companies | planned, not built | the revenue path if 2 works |
| 10 | **Agent-neutral "project context" standard** (open format other tools read) | tool makers | the brief schema | defends against platform risk if vendors adopt it |

Carried forward to validation: **1, 2, 3, 8** (cheap to test, built or nearly built). Parked: 9 (needs 2 proven), 10 (needs adoption), 5-7 (variants of 1-2).

## Critical assumptions

| # | Assumption | Category | Impact | Uncertainty | Priority |
|---|---|---|---|---|---|
| A1 | Enough developers use 2+ agents on one project and feel the switch pain weekly | Value | high | high | **leap of faith** |
| A2 | The brief makes the next agent act correctly without re-explaining (corrections per task falls 30%) | Value | high | high | **leap of faith** |
| A3 | People allow a tool to read their agents' chat files (opt-in, local, redacted) | Usability/Viability | high | medium | high |
| A4 | Drafted items are mostly right on real chats (eval chats were written with the heuristics, so 100% there means little) | Value | high | high | **leap of faith** |
| A5 | Private chat formats stay stable enough, or vendors tolerate readers | Feasibility | high | medium | high |
| A6 | Vendors do not ship native cross-agent hand-off in the next year | Viability | high | high | high |
| A7 | Setup takes under 10 minutes for a stranger | Usability | medium | medium | medium |
| A8 | Teams share a project vault through a private git repo (idea 2) | Viability | medium | high | medium |
| A9 | The memory write path can be cleaned so noise is under 20% (audit: 76% boilerplate) | Feasibility | high | low (we control it) | do now, no test needed |
| A10 | Someone would pay, and for what (seat, team vault, support) | Viability | high | high | later, after A1-A2 |

## Experiments

| # | Tests | Method | Success criteria | Effort | Timeline |
|---|---|---|---|---|---|
| X1 | A2, A4 | T12 on 3 real projects (Antigravity/Claude/Codex pairs): `tests/manual/t12_check.py`; count wrong or missing items | brief answers goal, ruled out, next step on 3 of 3; at most 1 wrong item each | 1 afternoon | week 1 |
| X2 | A2 | Eval harness, 10 real tasks, memory off vs on, 3 runs; corrections per task and tests pass | at least 30% fewer corrections, pass rate not lower | 3-4 days | week 1-2 |
| X3 | A1, A3, A7 | Concierge: 5 developers who use 2+ agents install it, no pitch; time install; call after a week | 3 of 5 still connected and used a hand-off unprompted; install under 10 min; 4 of 5 turn chat reading on | 1 week elapsed | week 1-2 |
| X4 | A1, A6 | 8 short interviews (script: last time you switched agents; what did you do; what did it cost) plus a public post "switch agents without re-explaining" with a waitlist link | 6 of 8 describe the pain unprompted; 50+ signups or 10 replies saying "I need this" | 3 days + posting | week 1-2 |
| X5 | A5 | Reader canary: run the testkit against the latest Claude Code and Antigravity builds weekly; note breakages | 0 silent breaks in 3 weeks (a break shows as "needs update") | 1 hour/week | weeks 1-3 |
| X6 | idea 3, idea 8 | Fake-door: add "Seatbelt" and "Instruction linter" as two lines on the install page/readme; count clicks and replies | either gets more interest than idea 1 per visitor, else drop | 1 day | week 2 |
| X7 | A8, A10 | Ask the 5 testers' teams: would 2 people share one vault? would you pay, how much, for what | 2 of 5 say yes to sharing; 1 names a price | in X3 calls | week 3 |

## Timeline
- **Week 1:** X1, start X2, recruit for X3, run X4 interviews, start X5. In parallel fix A9 (memory write path), it is not a hypothesis.
- **Week 2:** finish X2, X3 install week, X6 fake-door, post the X4 page.
- **Week 3:** X3 calls, X7, analysis, decision.

## Decision framework
- X1 and X2 pass and X3 shows 3 of 5 retained: **be "switch agents without re-explaining"**; everything else goes behind Advanced; plan outreach and a team vault (idea 2, then 9).
- X1 passes but X2 or X3 fails: it works but nobody needs it yet; narrow to a single agent pair, or fold in as a feature of idea 3 or 8 (whichever X6 favours).
- X1 fails (wrong or missing items): improve the drafter on the real chats you collected, re-run once; if still under 2 of 3, switch to the LLM drafter (measured) or drop chat reading and keep the agent-written brief.
- X4 shows no unprompted pain: the market is the maintainer alone; keep as a personal tool and stop adding surface.
- X6 favours seatbelt or linter: ship that as the front door (smallest install, clearest promise) and keep continuity as the second feature.
- X5 shows a vendor breaking readers monthly: do not promise chat reading; sell the agent-written brief and the standard format (idea 10).

## Next steps offered
Interview script for X4, a one-page landing text for X6, effort estimate and stories for the "front door" (install in 10 minutes, one-sentence promise), or a PRD for whichever idea wins.
