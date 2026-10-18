# Making AutoMemory's memory better: audit, research, plan

Oct 2026. Constraint kept: the daemon makes no LLM calls. Numbers about our own system come from the
maintainer's real `~/.automemory` database (read-only). Numbers about other systems are as reported
by their authors; vendor figures are marked.

## 1. What the data says about today's system

| Measure | Value | Meaning |
|---|---|---|
| Active memories | 51 | small |
| of which boilerplate (`Execution outcome for: …`, `Modified codebase files (1)`) | 39 (76%) | the distiller writes the tool call, not the lesson |
| Memories ever recalled (`last_recalled_at` set) | 0 of 77 | no feedback loop; nothing proves recall is used |
| Success traces | 4,790, only 37 distinct titles | 2,630 are the same "claude session execution trajectory" |
| Procedural skills | 7,283, only 11 distinct names, 29 distinct step lists | each Stop / commit / test pass inserts a copy instead of updating one |
| Failure records | 499, 483 with the same text "Non-zero exit or tool failure" | no error signature, so nothing to match or learn from (`grep` exiting 1 counts) |
| Decisions | 5, two pairs near-duplicates | the highest-value type is the rarest |
| Session checkpoint text | "38 edits across 0 files" | file paths are not reaching the digest |

Conclusion: the write path is the problem, not retrieval. The store grows fast (about 3 KB per memory,
thousands of duplicate traces and skills) while the part an agent needs (why a decision was made, what
broke and how it was fixed, what the user corrected) is almost absent. Better retrieval over this store
would retrieve noise faster.

## 2. What research says

* **Memory help for coding agents is not proven.** VibeMemBench (111 targets, 90 repos, 3,634 trajectories,
  scored by executable tests): injecting *verified* experience directly raised resolution 1.1–4.5 points on 4
  of 5 solvers and cut steps on all five; but 11 of 12 pairings of four existing memory systems did not beat
  memory-off. Repository history holds useful experience; current systems do not deliver it
  ([arXiv 2609.23570](https://arxiv.org/abs/2609.23570)).
* **Selection beats volume.** SWE-ContextBench (1,100 base + 376 related tasks, 51 repos): accurately
  summarized, correctly retrieved prior experience improved resolution and cut runtime and tokens, most on hard
  tasks; unfiltered or wrongly selected context gave little or hurt
  ([arXiv 2602.08316](https://arxiv.org/abs/2602.08316)). CTIM-Rover (ExpeL for SWE agents) also warns episodic
  memory adds noise ([arXiv 2505.23422](https://arxiv.org/html/2505.23422v1)). Subtask-level memory aligned to
  structure gave about +4.7 points on SWE-bench Verified
  ([arXiv 2602.21611](https://arxiv.org/html/2602.21611v1)); 135K curated "experience cards" gave +4.65
  ([MemGovern](https://arxiv.org/html/2601.06789v2)).
* **Forgetting, not recall, is where systems fail.** Across 13 configurations on a 385-case adversarial set,
  deterministic code handles lexical/temporal cases but scores 0–5% on obfuscated or cross-lingual
  identity; an LLM at write time fixes canonicalization but not deletion; an LLM only on the *mutation* path
  (supersede/delete) reaches 91.7–93.2% overall for about $0.17 per 385 cases, leaving recall untouched
  ([arXiv 2606.15903](https://arxiv.org/abs/2606.15903)). The March 2026 survey names "continual
  consolidation" and "learned forgetting" as unsolved and lists write-path filtering, contradiction handling and
  latency budgets as the real engineering concerns ([arXiv 2603.07670](https://arxiv.org/abs/2603.07670)).
* **Keep history; mark validity.** Zep/Graphiti sets `invalid_at` on a contradicted fact instead of deleting it
  (bi-temporal: when true, when learned) ([Zep paper](https://arxiv.org/abs/2501.13956); DMR 94.8% vs 93.4%
  MemGPT, vendor-reported).
* **Update/delete by LLM is fragile.** Mem0 moved from ADD/UPDATE/DELETE/NOOP reconciliation to single-pass
  ADD-only extraction; an independent study found one model emitting malformed JSON in about 25% of
  extraction responses ([Mem0 research](https://mem0.ai/research-3), vendor figures 92.5 LoCoMo / 94.4
  LongMemEval; a practitioner comparison found memory systems 14–77× costlier and ~30% less accurate than full
  history on another benchmark).
* **Consolidate offline.** Letta's "sleep-time" agents rewrite and deduplicate memory between sessions, using
  cheaper models, with git-like versioned context repositories
  ([Letta guide](https://forum.letta.com/t/sleeptime-agents-for-memory-consolidation-best-practices-guide/154)).
  Secondary sources only; treat details as indicative.
* **Caveat on all of it.** Most leaderboard numbers are self-reported; MemoryArena reports models near-perfect
  on LoCoMo falling to 40–60% when memory must drive decisions.

## 3. Principles for AutoMemory

1. **Store what the code cannot show** (decisions with reasons, constraints, gotchas, user corrections,
   fix recipes). Never the tool call itself.
2. **One fact, one row, updated in place**, with history kept (supersede, never silently overwrite).
3. **Facts from the ledger, prose from the agent.** The daemon extracts what is verifiable (exit codes, diffs,
   commit messages, user text); the agent, which is already an LLM, supplies the interpretation.
4. **Every memory must earn its place:** measured recall use and eval results decide what stays.
5. **Precision over volume** at read time: few, path-relevant, labelled entries.

## 4. Plan (ordered by value per effort)

| # | Change | Why (evidence) | How, without LLM calls in the daemon | Measured by |
|---|---|---|---|---|
| 1 | **Stop writing junk; make distillation idempotent** | 76% boilerplate, 4,790 traces / 37 titles, 7,283 skills / 11 names | Stable key per (session, task segment) and content hash; upsert instead of insert; quality gate: skip traces with no edit + verification pair, no title beyond "execution trajectory", or no non-inferable signal | boilerplate ratio < 10%; rows per session ≤ 3 |
| 2 | **One-time cleanup** | existing noise | `automemory memory prune`: merge duplicate traces/skills by hash, archive boilerplate, keep counts as usage metadata; reversible | row counts before/after |
| 3 | **Capture the high-value signals deterministically** | decisions are 5 of 77 rows | (a) user corrections from prompts ("don't…", "use X instead", "no, …") as tier-3 candidates tied to the files of the next edits; (b) failure signatures: normalized error line + command class + the fix (next successful command/diff touching the same files), so `Non-zero exit` stops being the only text; (c) commit messages as decision candidates linked to changed files; (d) red→green test pairs with the diff | share of memories with a reason; failure records with a signature |
| 4 | **Agent-in-the-loop distillation** | the LLM is the agent; writes are fragile when a second model does them | At PreCompact (Claude Code/Codex) and Stop, return a short instruction: save ≤ 3 non-obvious lessons via `memory.propose` with evidence IDs. Results enter the candidate tier (tier 4) and promote on verification or human confirm. Optionally use MCP sampling where the client supports it | candidates promoted per week; proposals rejected for no evidence |
| 5 | **Path-triggered recall + usage feedback** | selection beats volume; 0 of 77 recalled | On PreToolUse for a file, attach ≤ 2 one-line memories linked to that file or its dependents (codebase-memory-mcp supplies dependents); count a hit when the agent opens the entry (`memory.get`) or the file edit follows; decay unused entries, reinforce used ones | recall use rate; retrieval precision@3 |
| 6 | **Temporal validity** | Zep `invalid_at`; forgetting is the failing area | Add `valid_from` / `invalid_at` to memories and decisions; supersede chains already exist for decisions, extend to memories; PostCommit and the contradiction detector set `invalid_at` rather than archive | stale-flag precision (target ≥ 90%) |
| 7 | **Offline consolidation in `serve`** | consolidation is unsolved; sleep-time agents | Deterministic pass: cluster near-duplicates (normalized text + shared files), promote patterns seen in ≥ 3 distinct sessions to one skill with a usage count, compress episode traces older than 30 days into one summary row keeping evidence IDs; versioned so it is revertible | memory count flat as sessions grow |
| 8 | **Evaluate forgetting and use, not just recall** | ForgetEval, VibeMemBench | Extend `tests/eval`: executable-task A/B (already built) plus forgetting cases (supersede, delete, renamed symbol, contradicted decision) | task success delta; forgetting pass rate |

Where an LLM would help but the daemon will not call one: canonicalization (same entity, different
spelling) and intent-aware delete. Route those through the agent (#4) or an opt-in plugin on the *mutation*
path only, as the control-plane study suggests, never on recall.

## 5. What not to do first

* Do not add a vector database or knowledge graph yet: retrieval is not the bottleneck at 51 memories, and
  VibeMemBench shows richer stores did not help by themselves.
* Do not make the daemon write prose summaries with a model: Mem0's retreat from LLM update/delete and the
  25% malformed-output finding argue against it.
* Do not trust any benchmark number above until reproduced on your own tasks (`tests/eval`).

## 6. First sprint (about a week)

1. #1 and #2: idempotent distillation, quality gate, `memory prune`. Expect memories, traces and skills to fall by
   more than 90% with no information loss.
2. #3(a)(b): user corrections and failure signatures.
3. Recall telemetry (#5, counters only) so the next sprint is chosen from data.
4. Re-run the audit queries in section 1 and record the new table here.
