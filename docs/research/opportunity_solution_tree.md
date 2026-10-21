# Opportunity Solution Tree: does AutoMemory have a real opportunity?

Oct 2026. Method: Teresa Torres' tree (outcome, opportunities, solutions, experiments), opportunity score = importance x (1 - satisfaction).
**Honest limits:** the evidence is our own trials (T0-T11b, one maintainer, two agents), the memory audit and the papers in
[memory_improvement.md](memory_improvement.md) and [multi_agent_ecosystem_needs.md](multi_agent_ecosystem_needs.md). There are **no
outside user interviews yet**. Importance and satisfaction below are estimates to be replaced by interview data; the scores rank,
they do not prove.

## Desired outcome

> A person who uses two or more coding agents on one project stops re-explaining the project, and trusts what the agents know.

Measure (one number): **corrections per task** (times the person had to repeat or fix context), memory-off vs memory-on, on 10 real
tasks (the eval harness already records it). Target: at least 30% fewer corrections, and tests-pass no worse. Not measured yet (X3).

## Opportunities (customer words), ranked

| # | "I struggle to..." | Importance | Satisfaction today | Score | Evidence |
|---|---|---|---|---|---|
| O1 | switch agents mid-task without re-explaining the goal, what we decided and what we ruled out | 0.9 | 0.3 (each agent only resumes itself) | **0.63** | The maintainer's own case (long Antigravity chat, wants Claude to continue); T5 hand-off passed; no cross-agent tool found in the ecosystem study |
| O3 | know whether what an agent "remembers" is true and current | 0.8 | 0.2 (rule files and most memory tools do not check) | **0.64** | VibeMemBench: only verified experience helps; our audit: 76% of stored notes were boilerplate, 0 of 77 ever recalled |
| O7 | start in minutes, not a project (install, connect, trust it) | 0.8 | 0.4 | 0.48 | 63 tools and ~25 dashboard pages; setup passes in T0-T1 but adoption cost is our biggest self-made risk |
| O2 | stop two agents overwriting the same files | 0.7 | 0.4 (worktrees help some) | 0.42 | T4 passed; real but partly solved elsewhere |
| O4 | stop agents re-proposing ideas already rejected | 0.8 | 0.5 | 0.40 | Folded into O1: the brief's "ruled out and why" |
| O6 | keep secrets out of memory and dangerous commands out of agents | 0.7 | 0.5 | 0.35 | T6, redaction, human gate; table stakes more than a reason to buy |
| O5 | make agents agree a plan I approve before they code | 0.5 | 0.5 | 0.25 | T11 passed but costs tokens; no proof it beats one agent plus tests |

Focus: **O1 and O3 first** (O4 rides with O1). O7 is a constraint on everything. O2, O6 are supporting. O5 stays parked.

## Solutions

### O1 Switch agents without re-explaining
| Solution | Status | Perspective |
|---|---|---|
| A. Agent-written brief (goal, decided, ruled out with reasons, open, next) injected with provenance | built (C1) | PM: cheapest, works for every agent |
| B. Read the previous agent's chat files (opt-in, local, redacted), draft the brief, search the history | built for Claude Code and Antigravity (C2-C6) | Eng: private formats can break; modular readers |
| C. LLM-written summary of the old chat | optional (`llm run brief`), unmeasured | Eng: better prose, costs tokens, sends text out unless local |
| D. Export the whole chat into the new agent as a file | not built | Designer: simple, but huge and noisy; research says unfiltered context hurts |
| E. One shared "project instructions" file all agents read (what people do today) | partly built (`export-rules`) | no state, goes stale |

### O3 Trust what agents remember
| Solution | Status |
|---|---|
| A. Trust tiers and "verified / changed / stale" labels from git changes | built |
| B. Write-path cleanup: stop storing tool-call boilerplate, dedupe traces and skills (X1) | **not done; the audit says this is the real defect** |
| C. Validity and history (supersede instead of delete) (X6) | planned |
| D. A weekly "what changed in memory" report for the human | value report built |

## Experiments (cheapest first)

| # | Tests | Hypothesis, method, metric, threshold |
|---|---|---|
| E1 | O1 value | Run the Antigravity-to-Claude continuation (T12) on 3 real projects. Metric: questions "goal / ruled out and why / next step" answered correctly from the brief with no re-explaining. Pass: 3 of 3 projects, at most one wrong item each. Cost: an afternoon; checker exists (`tests/manual/t12_check.py`). |
| E2 | Outcome | Eval harness, 10 real tasks, off vs on, 3 runs each; metric corrections per task and tests pass. Pass: at least 30% fewer corrections, no drop in pass rate. If it fails, cut features before adding more. |
| E3 | Demand (skin in the game) | Give the install to 5 developers who use 2+ agents; no pitch. Metric: after one week, how many still have it connected and used the brief or hand-off unprompted. Pass: 3 of 5. Interview them for real importance and satisfaction numbers. |
| E4 | O3 | After X1, rerun the memory audit: boilerplate share under 20% and at least 1 in 5 injected notes later used (recalled and referenced). |
| E5 | O7 | Time a stranger from install to first useful hand-off. Pass: under 10 minutes, no help. |

## The tree

```
OUTCOME  Two-plus-agent users stop re-explaining and trust what agents know (corrections per task -30%)
├─ O1 Switch agents without re-explaining (0.63)         <- strongest, and unmet
│   ├─ brief with provenance (built) ........ E1, E2
│   ├─ read old chats, draft the brief (built) E1
│   ├─ LLM summary (optional) ................ compare to deterministic in E1
│   └─ dump the whole chat (rejected: noise)
├─ O3 Trust what is remembered (0.64)                    <- real, and we are weak here today
│   ├─ trust tiers, stale labels (built)
│   ├─ stop boilerplate writes, dedupe (X1, TODO) ... E4
│   └─ validity/history (X6, planned)
├─ O4 Do not redo rejected ideas (0.40) .............. part of O1
├─ O7 Minutes to value (0.48)                           <- adoption constraint
│   └─ fewer pages/tools by default, timed install ... E5
├─ O2 File collisions (0.42)  built, supporting
├─ O6 Secrets and dangerous commands (0.35)  built, table stakes
└─ O5 Agree a plan first (0.25)  parked until E2 shows value
```

## Verdict

**Yes, there is an opportunity, but only a narrow one is proven, and the proof is thin.**
- **Strongest:** cross-agent continuity (O1). It is a real, frequent pain, nothing in the ecosystem study solves it across vendors,
  and it is built end to end. It has been tested on fixtures and the maintainer's own chats, not on other people.
- **Real but we are behind:** trust (O3). The research agrees only verified memory helps, and our own audit shows the write path
  still stores mostly noise. Fixing X1 matters more than any new feature.
- **Not an opportunity yet:** planning rooms (O5) and more dashboard surface. Both add cost before value is shown.

**Biggest risks**
1. **Value unproven.** Most memory systems do not beat memory-off in studies. E2 decides whether this is a product or a hobby tool.
2. **Platform risk.** Claude Code, Antigravity or others could ship native cross-agent hand-off; their chat formats are private and
   can change. Mitigation: the agent-written brief works without reading chats; readers are small and isolated.
3. **Complexity.** 63 tools and a large dashboard are a hard sell for O7. The pitch should be one sentence: "switch agents without
   re-explaining", with everything else behind Advanced.
4. **n = 1.** Every pass so far is one person's setup. E3 is the cheapest way to find out if anyone else cares.

**Do next, in order:** E1 (T12, you can do it today) -> X1 (stop the noise) -> E2 on 10 tasks -> E3 with 5 people -> decide what to cut.
