# Evidence on what makes an AI agent memory system work (2025–2026)

Scope: peer-reviewed papers, preprints and reproducible benchmarks on extraction/consolidation, retrieval, temporal validity, contradiction, poisoning and evaluation, read for applicability to AutoMemory (local SQLite + FTS5, BM25 × time decay × confidence, optional vectors, deterministic distillation, trust tiers, code-hash verification). Labels: **[PR]** peer-reviewed, **[pre]** preprint, **[vendor]** vendor blog or self-reported.

## Key question 1: Which benchmarks exist, what do top systems score, which are saturated or flawed?

### Takeaway
LoCoMo is effectively saturated and partly broken (ground-truth errors, lenient judge, harness-dependent scores 38–96%); LongMemEval [PR, ICLR 2025] is the most-used credible conversational benchmark but vendor scores are not reproducible by third parties; MemoryAgentBench, BEAM and the coding-specific VibeMemBench / SWE-ContextBench are the newer, harder ones, and on all of them every commercial memory system scores at or below simple RAG or no-memory baselines.

### Cited Findings
- No official leaderboard exists for LoCoMo, LongMemEval or BEAM; authors publish datasets, not rankings. Self-reported LoCoMo: Mem0 92.5%, ByteRover 92.2%, Zep 94.7% (claimed), ZeroMemory 96.1% (claimed) [vendor] — [aiagentmemory.org](https://aiagentmemory.org/articles/agent-memory-leaderboard/)
- Third-party rerun (MemTensor OmniMemEval, fixed GPT-4.1-mini): Mem0 cloud 56.0 on LongMemEval vs its reported 94.4; Zep 63.8 on LoCoMo vs its reported 94.7 — [dev.to benchmark write-up](https://dev.to/everest_an/-i-benchmarked-ai-agent-memory-in-2026-and-the-numbers-tell-a-different-story-than-the-marketing-2ae4) (secondary; numbers not verified against OmniMemEval primary)
- LoCoMo audit (Penfield Labs): the LLM judge accepts ~63% of intentionally wrong answers; ~6.4% of ground truth is wrong (~99 wrong/ambiguous answers), implying an honest ceiling of ~93–94% — [dev.to Penfield](https://dev.to/penfieldlabs/proposal-a-real-benchmark-for-long-term-ai-memory-systems-57p5); [memoryplugin field guide](https://www.memoryplugin.com/wiki/evaluating.html)
- Same system scored 38%–92% on LoCoMo depending on harness; Gemini 2.5 Flash reading the raw transcript (no memory system) scores 72.8% — [memoryplugin field guide](https://www.memoryplugin.com/wiki/evaluating.html)
- Mem0's own ECAI 2025 paper [PR] reports LoCoMo J-score: full-context 72.90, Mem0-graph 68.44, Mem0 66.88, Zep 65.99, best RAG 60.97, LangMem 58.10, OpenAI 52.90, A-Mem 48.38 — i.e. full context beat every memory system; Mem0's win was latency (p95 1.44 s vs 17.1 s) and tokens (1,764 vs 26,031) — [arXiv 2504.19413](https://arxiv.org/html/2504.19413v1)
- Zep publicly disputed Mem0's SOTA claim, calling LoCoMo flawed and the Mem0 paper's Zep implementation incorrect [vendor, competitor] — [Zep blog](https://blog.getzep.com/lies-damn-lies-statistics-is-mem0-really-sota-in-agent-memory/)
- Letta's filesystem-only agent reaches 74.0% on LoCoMo, used to argue LoCoMo cannot separate trivial from sophisticated approaches [vendor] — [memoryplugin](https://www.memoryplugin.com/wiki/evaluating.html)
- LongMemEval [PR, ICLR 2025]: fact-augmented key expansion +9.4% recall@k and +5.4% QA accuracy; time-aware indexing + query expansion +6.8–11.3% recall on temporal-reasoning questions — [arXiv 2410.10813](https://arxiv.org/html/2410.10813v2); [ICLR 2025](https://iclr.cc/virtual/2025/poster/28290)
- MemoryAgentBench [pre, v4 June 2026, ICLR code of ethics cited]: 4 competencies (accurate retrieval AR, test-time learning TTL, long-range understanding LRU, selective forgetting SF). Overall: GPT-4o long-context 48.8; BM25 RAG 41.5; Qwen3-Embedding RAG 38.2; MemGPT 28.3; Zep 24.0; Mem0 21.1; Cognee 20.6. BM25 beat all embedding RAG and all memory products on AR (60.5). Selective-forgetting multi-hop: ≤5.0 for every system in Table 3 (Zep 3.0, Mem0 n/a, long-context 5.0) — [arXiv 2507.05257](https://arxiv.org/html/2507.05257v4)
- BEAM [pre; ICLR 2026 per Cognee page, unverified]: 100 conversations, 2,000 questions, up to 10M tokens; long-context models score 0.24–0.28 at the shortest tier, 0.10–0.13 at 10M; metric is rubric-nugget coverage, not % correct; vendor reports Cognee 0.79/0.67 (100K/10M), Mem0 0.641 avg on 1M [vendor] — [Cognee guide](https://www.cognee.ai/agent-memory-evaluation-datasets-benchmarks); [Mem0 how-to-read](https://mem0.ai/library/agent-memory/how-to-read-memory-benchmarks)
- ConvoMem [pre]: argues the first ~150 conversations fit in context and need no RAG — [arXiv 2511.10523](https://arxiv.org/html/2511.10523v1)
- EvoMemBench [pre]: long-context baselines remain highly competitive; memory helps most when current context is insufficient or the task is hard; no single memory form wins everywhere — [arXiv 2605.18421](https://arxiv.org/pdf/2605.18421)
- VibeMemBench [pre, Sept 2026]: 111 coding targets from 90 SWE-rebench V2 repos, 3,634 history trajectories; memory off vs best existing system (Mem0, SimpleMem, MemoryOS, A-MEM): 11 of 12 pairings at or below the no-memory baseline; oracle "frozen verified experience" +0.0 to +4.5 pp, all bootstrap CIs cross zero — [arXiv 2609.23570](https://arxiv.org/html/2609.23570v1)
- SWE-ContextBench [pre, Feb 2026]: Lite (Claude Sonnet 4.5, 99 tasks): baseline 26.26%, autonomous trajectory retrieval 26.26%, oracle trajectory 27.27%, autonomous summary 22.22% (below baseline), oracle summary 34.34%, Mem0 24.24%, Supermemory 30.30%, OpenViking 29.20% — [arXiv 2602.08316](https://arxiv.org/html/2602.08316v3)
- A 27.5-point gap between strict token-F1 and LLM-judge scores on identical outputs in a LoCoMo setup — [arXiv 2606.22030](https://arxiv.org/abs/2606.22030)

### Inferences
- Any vendor LoCoMo/LongMemEval number above ~90% should be treated as a harness artifact until a third party reproduces it; the only independent reruns put commercial systems at 20–65.
- The honest signal across benchmarks is consistent: BM25 is a strong baseline (beats dense RAG and all products on MemoryAgentBench AR), and the hard, unsolved competencies are knowledge update / selective forgetting and multi-session aggregation.
- For a coding-agent memory, VibeMemBench and SWE-ContextBench are the right yardsticks; both say retrieval precision and record form matter more than the store.

### Gaps
- Could not find MemBench (arXiv 2506.21605) headline numbers; only that it tests 7 mechanisms on Qwen2.5-7B.
- OmniMemEval primary report not fetched; the Mem0 56.0 / Zep 63.8 reruns are from a secondary write-up.
- BEAM's ICLR 2026 acceptance is claimed by a vendor page and not verified.

## Key question 2: Extraction strategies — LLM ADD/UPDATE/DELETE vs append-only bi-temporal vs offline consolidation

### Takeaway
LLM-driven ADD/UPDATE/DELETE reconciliation (Mem0) has no measured advantage over full context and loses facts; append-only with invalidation (Graphiti) has no published precision numbers; offline consolidation ("dreaming"/sleep-time) has the strongest measured effect (Anthropic wiki 13%→48%; sleep-time compute ~5× test-time compute reduction) but all of it is LLM-driven.

### Cited Findings
- Mem0 [PR, ECAI 2025] update phase: for each extracted fact retrieve top-10 similar memories, LLM picks ADD / UPDATE / DELETE / NOOP via tool call; DELETE is used when new info contradicts old. Result: 66.88 J vs 72.90 full-context — [arXiv 2504.19413](https://arxiv.org/html/2504.19413v1)
- Independent schema-aware extraction study [pre] measured Zep Cloud extraction at 77.44% precision, 83.06% recall, 80.16% F1 — [arXiv 2604.27906](https://arxiv.org/pdf/2604.27906)
- Graphiti/Zep [vendor docs]: old facts are marked invalid, not deleted; facts carry validity windows; conflicts found via semantic + keyword + graph search; new information takes priority by default; bulk ingestion skips edge invalidation (third-party tutorial) — [Graphiti repo](https://github.com/getzep/graphiti); [Graphiti tutorial](https://app-lab.ai/blog/graphiti-tutorial/)
- Zep paper [pre] reports 94.8% on DMR vs MemGPT 93.4% and 71.2% on LongMemEval with GPT-4o vs 60.2% full-context baseline [vendor-run] — [Zep arXiv 2501.13956 via secondary](https://www.digitalapplied.com/blog/open-source-agent-memory-mem0-letta-zep-compared)
- Sleep-time compute [pre, Letta/UC Berkeley]: cuts test-time compute needed for a given accuracy ~5× on Stateful GSM-Symbolic and Stateful AIME; benefit tracks how predictable the query is from the context — [arXiv 2504.13171](https://arxiv.org/html/2504.13171v1)
- Anthropic "components of agent memory" [vendor]: dreaming (offline pass that checks notes against transcripts, corrects, merges duplicates) raised a wiki over a ~350K-file corpus from 13% to 48% of graded criteria across 2→128 sessions; wiki+search 61% vs search-alone 53%; on a 24-question cross-document legal set wiki+search 87% vs search 73%; no gain when facts sit in one document — [Anthropic](https://www.anthropic.com/research/components-of-agent-memory)
- Same source: fixed salience rules hurt — telling models to "record only failures" cut memory gains by 0.234 (Opus 5), 0.150 (Opus 4.7), 0.020 (Sonnet 4.6); indiscriminate logging (413 files, 978K chars) earned 1 badge vs ~10 files/43K chars earning 4 — [Anthropic](https://www.anthropic.com/research/components-of-agent-memory)
- Same source: shared-table storage of contract facts kept 41/57 (72%); one page per entity with verbatim terms kept 57/57 and raised criteria met from 72% to 100%; concurrent writers overwrote each other's facts — [Anthropic](https://www.anthropic.com/research/components-of-agent-memory)
- VibeMemBench failure attribution: 69.3% of failing memory pairings are "form degradation" (relevant history reached the solver but was dropped or buried): instruction pollution 40%, overgeneralized fix patterns 18%, tool/shell noise 10%; raw-transcript systems (MemoryOS, A-MEM) cause the pollution cases; coverage misses 16.0%, ranking misses only 1.3% — [arXiv 2609.23570](https://arxiv.org/html/2609.23570v1)
- MEMTIER [pre]: consolidation daemon distils episodic JSONL into a semantic tier; adding the semantic tier lifted LongMemEval-S accuracy from 0.252 (BM25 over episodic only) to 0.382; knowledge-update 0.346→0.436, multi-session 0.060→0.180 — [arXiv 2605.03675](https://arxiv.org/html/2605.03675v3)

### Inferences
- Measured evidence favors: (a) small, curated, non-inferable records over raw logs; (b) one record per entity rather than merged tables; (c) an offline pass that re-verifies records against ground truth (transcripts, or for AutoMemory, code hashes). None of this requires an LLM at write time; the LLM-driven piece with evidence is only the offline verification.
- Mem0-style DELETE-on-contradiction is a lossy design with no measured accuracy benefit; append + invalidate (Graphiti) preserves history but its contradiction detection has no published precision.

### Gaps
- No Letta-published evaluation of sleep-time *memory reorganization* (leaderboard page says planned).
- No controlled head-to-head of ADD/UPDATE/DELETE vs append-only on the same data.
- Mem0's "later change" from LLM reconciliation was not found in any source.

## Key question 3: Which retrieval methods measurably help?

### Takeaway
BM25 is the strongest single baseline on memory benchmarks; hybrid RRF adds ~+1–3 pts overall at 3–4× latency and can hurt knowledge-update questions; query/key expansion with facts and time is the best-evidenced lift (+5–11 pts); time decay and access-frequency boosts have measured *negative* effects in two independent ablations; cross-encoder reranking is negative or untested in memory settings; tiered loading is architectural with no clean ablation.

### Cited Findings
- MemoryAgentBench: BM25 RAG 60.5 AR vs Text-Embed-3-Small 53.8, Qwen3-Embedding-4B 54.7, Contriever 33.9, Mem0 32.6, Zep 37.5 — [arXiv 2507.05257](https://arxiv.org/html/2507.05257v4)
- MEMTIER retrieval ablation (LongMemEval-S, N=100): BM25 0.360 / 25.7 ms; dense 0.380 / 92.6 ms; hybrid RRF 0.390 / 96.7 ms (P95 256 ms). Per category BM25→hybrid: multi-session 0.115→0.192, temporal 0.370→0.407, knowledge-update 0.500→0.375, single-session unchanged 0.714. Removing time decay, cognitive weight or tier boost each *raised* accuracy +0.012–0.014. Five BM25 score normalisations all gave identical accuracy (ceiling is what BM25 retrieves, not scaling). Reranking not evaluated — [arXiv 2605.03675](https://arxiv.org/html/2605.03675v3)
- vstash [pre; SQLite + FTS5 + sqlite-vec, bge-small, 20.9 ms median at 50K chunks]: RRF k=60 with per-query weights from mean IDF (rare-term queries → more keyword weight) beat fixed 0.6/0.4 on all five BEIR sets (+0.1% to +21.4% NDCG@10). Frequency+decay reranker lowered NDCG@10 vs pure RRF in every config (−1.6% adaptive, −9.0% fixed); cross-encoder reranking hurt −0.3% to −3.1%; hybrid beat vector-only on Wikipedia (0.758 vs 0.742) but not on the paper corpus at NDCG@10 (0.803 vs 0.832) — [arXiv 2604.15484](https://arxiv.org/html/2604.15484v1)
- LongMemEval [PR]: fact-augmented key expansion +9.4% recall@k, +5.4% QA; time-aware indexing + query expansion +6.8–11.3% temporal recall — [arXiv 2410.10813](https://arxiv.org/html/2410.10813v2)
- BioASQ 14B report [pre]: weighted-sum hybrid beat BM25 but RRF underperformed BM25 — fusion method matters — [arXiv 2609.04999](https://arxiv.org/pdf/2609.04999)
- Mem0 graph variant: +~2% over base Mem0 (68.44 vs 66.88) at 2× tokens and 3× search latency — [arXiv 2504.19413](https://arxiv.org/html/2504.19413v1)
- Anthropic: Claude Code loads only the first 200 lines / 25 KB of its memory index; long instruction files reduce adherence; "keep the injected layer small" — [Anthropic](https://www.anthropic.com/research/components-of-agent-memory)
- MEMTIER single-session-preference diagnostic (acc 0.067): failures from extraction discarding indirect preferences, BM25 lexical gap on paraphrases, and reader sensitivity — [arXiv 2605.03675](https://arxiv.org/html/2605.03675v3)

### Inferences
- For AutoMemory's BM25 × time decay × confidence formula: the time-decay factor has two independent negative ablations and should be re-tested or made opt-in; IDF-adaptive fusion weights are a cheap, measured improvement if vectors are enabled.
- The cheapest measured win that needs no LLM is indexing extra keys per record (dates, entity/file names, synonyms) rather than changing the ranker.
- Graph traversal has only a ~2-pt vendor-measured gain at large cost; not justified for a local daemon.

### Gaps
- No agent-memory paper evaluates progressive/tiered loading (abstract → outline → full) as a controlled ablation; MEMTIER's 600-token injection budget (0.412 vs 0.382) is the nearest datum.
- No reranking result in a memory benchmark other than vstash's negative one.

## Key question 4: Forgetting, staleness, contradiction

### Takeaway
Every evaluated system fails multi-hop knowledge update (≤5% on MemoryAgentBench SF multi-hop); no published precision for Graphiti-style invalidation; the only measured contradiction handler is Bayesian reliability-conditional updating, which beats last-write-wins on a synthetic contradiction benchmark but gives no gain on LoCoMo because conversational data rarely contradicts itself. Verification of notes against ground truth (transcripts/code) is the one approach with a measured positive effect.

### Cited Findings
- MemoryAgentBench selective forgetting (FactConsolidation): single-hop / multi-hop — GPT-4o long-context 60.0 / 5.0; BM25 48.0 / 3.0; MemGPT 28.0 / 3.0; Zep 7.0 / 3.0; authors: "all methods fail on the multi-hop situation" — [arXiv 2507.05257](https://arxiv.org/html/2507.05257v4)
- MEMTIER hybrid retrieval *reduced* knowledge-update accuracy (0.500→0.375) because dense similarity surfaces superseded facts — [arXiv 2605.03675](https://arxiv.org/html/2605.03675v3)
- Nous / belief-based memory [pre]: estimates each observation's reliability from epistemic language, feeds Bayesian updating over entity-attribute distributions; "substantially outperforms last-write-wins and raw-memory retrieval" on a controlled contradiction benchmark; little benefit over last-write-wins on LoCoMo — [arXiv 2606.22030](https://arxiv.org/abs/2606.22030)
- Anthropic: a single wrong landmark note made the agent repeat a mistake (loop 3× and never reach the goal); after dreaming corrected the note it reached the goal at action 1,935; memory of screen-readable state (money, map position) went stale and was redundant — [Anthropic](https://www.anthropic.com/research/components-of-agent-memory)
- Graphiti: invalidation, not deletion; design-level only, no accuracy numbers; Agent Memory Atlas says the repo's eval utilities "do not establish that temporal extraction or entity resolution is objectively correct" — [Agent Memory Atlas](https://neoneye.github.io/agent-memory-atlas/systems/graphiti/)
- A-MemGuard [PR, ICML 2026]: consensus validation flags a memory whose reasoning conflicts with related memories — a contradiction detector used for security, see Q5 — [PMLR v306](https://proceedings.mlr.press/v306/wei26ac.html)

### Inferences
- Content-based contradiction detection has no measured precision anywhere; AutoMemory's code-hash verification is a stronger signal than anything in the literature because it checks against ground truth rather than against other memories.
- Scoping decay to volatile record types (Anthropic's untested suggestion) matches the ablations: uniform time decay hurts, targeted staleness checks help.

### Gaps
- No paper reports precision/recall of "this stored fact is no longer true" detection on real data.
- Zep paper's own invalidation evaluation not fetched.

## Key question 5: Memory poisoning and defenses

### Takeaway
Attack success is high in idealized settings (MINJA 76.8–98.2%, MemPoison 0.95, GhostWriter ~98%) and ~34–67% in the systematic MPBench study, but drops sharply with pre-existing legitimate memories. Measured defenses: A-MemGuard [ICML 2026] >95% ASR reduction via consensus validation; provenance-capped trust resists volumetric poisoning (preprint, no number); prompt-injection detectors catch only 23–68% with up to 53% false positives. Trust tiers, provenance and quarantine are recommended everywhere but measured in only one preprint.

### Cited Findings
- MINJA [pre]: query-only attack, 98.2% injection success, 76.8% attack success on average — [arXiv 2503.03704](https://arxiv.org/html/2503.03704v2)
- Sunil et al. [pre, Jan 2026, EHR agents on MIMIC-III]: realistic conditions with pre-existing legitimate memories "dramatically reduce" MINJA effectiveness (no number given); Llama Guard and sanitization "largely ineffective"; proposes composite trust scoring and trust-aware retrieval with temporal decay + pattern filtering; warns thresholds must be calibrated to avoid blocking everything; no ASR-reduction or FPR numbers — [arXiv 2601.05504](https://arxiv.org/abs/2601.05504)
- MPBench / "From Untrusted Input to Trusted Memory" [pre, June 2026, GPT-OSS-120B only]: 6 attack classes; average ASR 34.25% (OpenClaw) and 66.67% (HERMES); conditional-command insertion 67.9–76.0%; compaction poisoning 45.1–85.2%; weak-signal attacks (policy-conformant fact injection, false precedent) evade detectors. Detectors TPR/FPR: PIGuard 38.3/0.3, DataFilter 23.0/53.3, CommandSans 52.3/45.0, PromptArmor 67.7/1.0; PromptArmor 84.4% on strong-signal vs 42.5% on weak-signal attacks. Provenance, source isolation and post-write monitoring listed only as future work — [arXiv 2606.04329](https://arxiv.org/html/2606.04329v1)
- A-MemGuard [PR, ICML 2026]: consensus-based validation (flag memory whose reasoning conflicts with related memories) + a separate "lesson" memory; >95% ASR reduction across benchmarks, >97% on EHRAgent, highest benign accuracy among defenses, 0.950 task success in multi-agent setting [author-reported] — [PMLR v306](https://proceedings.mlr.press/v306/wei26ac.html); [arXiv 2510.02373](https://arxiv.org/html/2510.02373v1)
- Provenance-capped defense [pre]: trust capped by source provenance rather than text confidence "resists volumetric poisoning attacks" with utility cost; content-derived reliability remains manipulable — [arXiv 2606.22030](https://arxiv.org/abs/2606.22030)
- MemPoison [pre]: backdoors that survive selective memory extraction, ASR up to 0.95 — [arXiv 2605.29960](https://arxiv.org/abs/2605.29960)
- Sleeper memory poisoning [pre]: goal-distant triggers succeed only 0–5%; success depends on query relatedness — [arXiv 2605.15338](https://arxiv.org/html/2605.15338)
- SMSR [pre]: certified defence against runtime memory poisoning (not fetched; title only) — [arXiv 2606.12703](https://arxiv.org/pdf/2606.12703)
- Eywa and Agent Zero Memory [pre] propose provenance-grounded memory; the latter self-reports 93.6 LoCoMo / 95.6 LongMemEval [vendor-grade] — [arXiv 2605.30771](https://arxiv.org/pdf/2605.30771); [arXiv 2608.29606](https://arxiv.org/pdf/2608.29606)

### Inferences
- The two defenses with any measured effect are both things a deterministic daemon can approximate: (1) cap trust by *source* (tool output / web / other agent < user < verified), never by content; (2) flag records that conflict with related records or with ground truth before they are injected. Prompt-injection classifiers on memory content are not worth running.
- The "~50% attack success ICML 2026 study" the request mentions most likely corresponds to MPBench's ~34–67% averages (preprint; ICML acceptance not confirmed) or Agent Security Bench's 84.3% (vendor-cited).

### Gaps
- Could not identify an "ICML 2026 memory-poisoning study with ~50% ASR" by that description; A-MemGuard is the confirmed ICML 2026 memory-security paper.
- "MemConflict" returned no results under that name.
- No measured effect for quarantine/staging of new memories or for trust tiers specifically, beyond the provenance-cap preprint.

## Key question 6: Does memory improve task success, and for which task types?

### Takeaway
Memory helps when it holds context that cannot be inferred from the environment (strategies, tested rules, cross-document facts): Anthropic CL-Bench 0.38→0.70 (Opus 4.7), legal cross-document 73→87%. It does not help, and costs 20%+ more, when it restates what the agent can read from the repo: AGENTS.md files give −0.5 to −2 pp on SWE-bench/CTXbench (p=0.87/0.37) and coding-memory systems land at or below no-memory baselines; only precise, verified, task-relevant experience gives +3–8 pp, and that is an oracle upper bound.

### Cited Findings
- Gloaguen et al., "Evaluating AGENTS.md" [ICLR 2026 workshop on Memory for LLM-Based Agentic Systems; pre]: 4 agent/model pairs (Claude Code+Sonnet 4.5, Codex+GPT-5.2, Codex+GPT-5.1 mini, Qwen Code+Qwen3-30B) on SWE-bench Lite (300) and CTXbench (138 instances, 12 repos with developer-committed files). LLM-generated files: −0.5 pp (SWE-bench, p=0.87), −2 pp (CTXbench, p=0.37); developer files +2.4 pp (p=0.21, n.s.); Dev beats LLM-generated by ~7 pp (p=0.038, the only significant result). Cost +20% / +23% (significant); +2.45–3.92 steps. Sonnet 4.5 on CTXbench: None 73.2, LLM 65.2, Dev 70.3. Instructions are followed (`uv` 1.6 uses/instance when mentioned vs <0.01). Repository overviews present in 95–100% of LLM files and not effective. In docs-stripped repos LLM files helped +2.7 pp. Limitations: Python only, one sample per instance, task resolution only — [arXiv 2602.11988](https://arxiv.org/html/2602.11988v2); [ICLR listing](https://iclr.cc/virtual/2026/10021235); [ETH SRI](https://www.sri.inf.ethz.ch/publications/gloaguen2026agentsmd)
- Two-agent ablation on real repos [pre]: 17 tasks, 288 runs, Claude Code and Codex; the real AGENTS.md never converted a failure to a pass — [arXiv 2607.27250](https://arxiv.org/html/2607.27250v1)
- SWE-ContextBench: oracle summary +8 pp over baseline on Lite (34.34 vs 26.26); per model on full benchmark baseline→oracle summary: Sonnet 4.5 19.68→23.40, GPT-5.3 Codex 22.60→23.94, Qwen3.5-plus 16.22→21.54, GLM-5 18.35→19.41, MiniMax 2.7 19.68→22.07; autonomous summary retrieval fell *below* baseline (22.22) because irrelevant summaries mislead; Mem0 24.24 (below baseline) — [arXiv 2602.08316](https://arxiv.org/html/2602.08316v3)
- VibeMemBench: irrelevant-text control lowers resolve rate by 0.7–3.4 pp; injection reduces agent steps for every solver; same record helps a weaker solver and hurts a stronger one; experience retained only if it improved pass rate in a reference run (192→349 of 444, 43.2%→78.6%, but selection guarantees positive uplift by construction) — [arXiv 2609.23570](https://arxiv.org/html/2609.23570v1)
- Anthropic CL-Bench database task, mean reward no-memory→memory: Opus 5.5 0.655→0.838; Opus 4.7 0.383→0.700; Sonnet 5 0.306→0.610; Sonnet 4.6 0.330→0.364 (rose to 0.641 only when prompted to read memory and test theories). Fable 5 stored strategies/plans/tested rules and skipped screen-readable state — [Anthropic](https://www.anthropic.com/research/components-of-agent-memory)
- "Codified Context" [pre] argues for infrastructure-level context in a complex codebase (not fetched) — [arXiv 2602.20478](https://arxiv.org/pdf/2602.20478)
- Factorial study of context-file structure variables [pre] (not fetched) — [arXiv 2605.10039](https://arxiv.org/pdf/2605.10039)

### Inferences
- The task-type split is consistent across three independent sources: non-inferable context (decisions, failed approaches, tested rules, cross-file facts, non-standard practices) helps; inferable context (repo overviews, file layout, standard tooling) is neutral to negative and costs tokens. AutoMemory's record types (decisions, lessons, failures) are on the right side of this line; any "project overview" record is on the wrong side.
- Retrieval precision is the gate: wrong or irrelevant records cost 1–4 pp, so a daemon should prefer returning nothing over returning a weak match.

### Gaps
- No study measures memory accumulated live across real sessions of one developer; all use pre-built pools.
- No study separates the effect of memory by record type (decision vs lesson vs failure).

## Ranked techniques: strength of evidence × applicability to a local, LLM-free daemon

1. **Store only non-inferable, verified records; keep the injected layer small** — strongest convergent evidence (AGENTS.md [ICLR-WS], Anthropic, VibeMemBench form-degradation 69%). Fully applicable; it is a write-policy and a size cap.
2. **BM25/FTS5 as the primary ranker** — BM25 beats dense RAG and every product on MemoryAgentBench AR; 25 ms vs 97 ms. Already AutoMemory's core; evidence says do not replace it.
3. **Key/field expansion at index time (dates, entity and file names, resolved paraphrases)** — LongMemEval [PR] +5–11 pp; deterministic, no LLM needed.
4. **Trust capped by source provenance, not content; flag records that conflict with ground truth or related records before injection** — A-MemGuard [ICML 2026] >95% ASR reduction, provenance cap [pre]. AutoMemory's trust tiers + code-hash verification map directly; add per-record source and a "verified against hash" gate on retrieval.
5. **Offline re-verification/consolidation pass** — Anthropic dreaming 13→48%, MEMTIER semantic tier 0.25→0.38. Applicable as a cron over code hashes and duplicate merging; the LLM part (rewriting notes) is optional.
6. **One record per entity, single writer per record** — Anthropic 72%→100% fact survival; cheap schema rule.
7. **Hybrid dense + IDF-adaptive RRF (optional vectors)** — vstash +0.1–21% over fixed weights, MEMTIER +0.03 overall but −0.125 on knowledge-update at 3.7× latency. Keep optional; disable for "latest state" queries.
8. **Time decay in the score** — two independent negative ablations (MEMTIER, vstash). Re-test AutoMemory's decay term; evidence favors decay only on volatile record types or as a staleness *filter* after verification, not a multiplier.
9. **Append-only with validity windows (bi-temporal)** — sound design, zero published precision; adopt the schema (valid_from/invalid_at) but do not expect it to detect contradictions on its own.
10. **Cross-encoder reranking, graph traversal, LLM ADD/UPDATE/DELETE reconciliation, prompt-injection classifiers over memory** — negative, marginal (+2 pp at 2–3× cost) or unmeasured; not justified for a local daemon.
11. **Progressive/tiered loading (abstract → outline → full)** — plausible and used (Claude Code 25 KB index, MEMTIER 600-token budget 0.41 vs 0.38) but no controlled ablation; implement for cost, do not claim accuracy gains.

## Gaps (overall)
- No neutral, third-party leaderboard for memory systems; all top scores are self-reported.
- No published precision for contradiction/invalidation detection in any system.
- No study of memory accumulated live over real developer sessions, nor by record type.
- The specific "ICML 2026 ~50% ASR" poisoning paper and "MemConflict" could not be identified.
- Zep paper, MemBench, SMSR, Codified Context and the factorial context-file study were not read in full.
