# Competitor teardown: memory systems for AI coding agents (state as of 2026-11-12)

Scope note: AutoMemory = local-first Rust daemon, SQLite WAL + FTS5, optional vector hybrid, MCP server (49 tools), hooks for Claude Code/Codex/Antigravity, trust tiers (pinned / verified / user statement / agent claim / untrusted), verification of notes against file/symbol hashes, automatic file locks, encrypted git-vault sync, backup/restore, no LLM call unless configured. Everything below is about what others do differently. Vendor-run benchmark numbers are marked "(vendor)". Where a fact is older than 2026 it is dated.

## Per-competitor teardown (architecture, extraction, retrieval, multi-agent, staleness, portability, pricing, benchmarks, adoption, weaknesses)

### Takeaway
The field split in 2026 into (a) LLM-extraction cloud memory APIs (Mem0, Zep/Graphiti, Supermemory, Hindsight), (b) file/Markdown-shaped memory inside the agent harness (Claude Code auto memory, Anthropic memory tool, Letta Code "git-backed memory", OpenViking `viking://`, Copilot memory), and (c) local single-binary SQLite+FTS5 sidecars for coding agents (claude-mem, engram). AutoMemory sits in (c) but with trust/verification/locking features none of the others in (c) have; what it lacks is mostly from (a)/(b): LLM-driven consolidation ("dreaming"), temporal validity on facts, published benchmark numbers, and citation-based re-validation à la Copilot.

### Cited Findings

**Mem0 (mem0ai/mem0, Apache-2.0 OSS + hosted platform)**
- Architecture: LLM extracts facts from conversation turns into a vector store (default embedded on-disk Qdrant), optional graph store; scoped by `user_id`, `agent_id`, `run_id`. — [Hindsight/Vectorize roundup, Aug 2026 (vendor-authored by a competitor)](https://hindsight.vectorize.io/blog/2026/08/11/open-source-agent-memory-systems); [rywalker research page, Jun 2026](https://rywalker.com/research/mem0)
- OSS default LLM is GPT-4.1-nano; every add/update requires an LLM call (latency + cost). — [rywalker](https://rywalker.com/research/mem0)
- SDK v2.0.0 (Python) / v3.0.0 (TS), 2026-04-14: "ADD-only extraction: memories accumulate and nothing is overwritten or deleted"; hybrid retrieval = semantic + BM25 + entity boost scored in parallel; built-in graph memory with entity linking (external graph stores removed from OSS, breaking). Vendor benchmark claims for the new algorithm: LoCoMo 91.6, LongMemEval 93.4, BEAM (1M tokens) 64.1 (vendor). — [Mem0 changelog highlights](https://docs.mem0.ai/changelog/highlights)
- Memory Decay, 2026-05-08 (Py 2.0.2 / TS 3.0.3): opt-in, per-project search-time ranking bias 0.3x–1.5x that boosts recently used memories and dampens stale ones; reorders only, never filters. — [Mem0 changelog](https://docs.mem0.ai/changelog/highlights)
- Temporal Reasoning (Platform v3), 2026-05-13: parses "last week", "as of <date>" against memory timestamps; `reference_date` param. — [Mem0 changelog](https://docs.mem0.ai/changelog/highlights)
- SDK memory expiration, 2026-06-27: `update()` accepts `expiration_date`; TS adds `expirationDate`, `showExpired`. — [Mem0 changelog](https://docs.mem0.ai/changelog/highlights)
- Reranking in TS SDK v3.1.0 (2026-07-13): Cohere, ZeroEntropy, cross-encoder, LLM-based via `rerank` option. — [Mem0 changelog](https://docs.mem0.ai/changelog/highlights)
- Mem0 CLI v0.2.2 (2026-04-06): commands add/search/list/get/update/delete/**import**/config/init/status/entity/event, `--json` output. — [Mem0 changelog](https://docs.mem0.ai/changelog/highlights)
- Mem0 Plugin v0.2.11 (2026-06-25) for Claude Code, Cursor, Codex, Antigravity: automatic context injection, project + global scopes, 17-category coding taxonomy, auto-capture; OpenCode plugin v0.2.0 adds "auto-dream consolidation" and secret redaction; Pi plugin adds "dream consolidation" and reranked search. — [Mem0 changelog](https://docs.mem0.ai/changelog/highlights)
- OpenMemory MCP (local-first MCP server, launched 2025-05-13) was sunset: tracked in issue #4923 (opened 2026-04-22); README sunset notice by late May 2026 ("OpenMemory is being sunset. For local self-hosted memory with a dashboard, please use the Mem0 self-hosted server instead"); directory removed by Aug 2026. Replacement self-hosted server lacks the dashboard and MCP integration. — [chatforest review, Aug 2026](https://chatforest.com/reviews/mem0-mcp-server/); original launch: [Mem0 blog](https://mem0.ai/blog/introducing-openmemory-mcp)
- Security: CVE-2026-31241 / CVE-2026-31244 (published 2026-05-12) — mem0 server 1.0.0 lacked auth on `DELETE /memories` and `DELETE /memories/{id}`. Issue #4875 (2026-04-17) disclosed 18 SQL injection points in PGVector, 11 in Azure MySQL, Cypher injection in Neptune Analytics; fixed by PR #4997 (merged 2026-04-28) and PR #4878 (merged 2026-07-07). — [Mend CVE-2026-31241](https://www.mend.io/vulnerability-database/CVE-2026-31241/); [chatforest](https://chatforest.com/reviews/mem0-mcp-server/)
- Pricing (managed): Hobby free (10,000 adds, 1,000 retrievals/mo), Starter $19, Growth $79 (one Aug-2026 audit says Growth was retired again), Pro $249 (500K adds), Enterprise custom; graph memory + "Dream consolidation" on Pro; on-prem/SSO/audit logs on Enterprise. — [rywalker](https://rywalker.com/research/mem0); [digitalapplied, Aug 2026](https://www.digitalapplied.com/blog/open-source-agent-memory-mem0-letta-zep-compared); [chatforest](https://chatforest.com/reviews/mem0-mcp-server/)
- Adoption: ~58.4K stars (Jun 2026), 62,736 stars / 7,315 forks (Aug 2026), ~63.0K (Aug 2026). — [rywalker](https://rywalker.com/research/mem0); [digitalapplied](https://www.digitalapplied.com/blog/open-source-agent-memory-mem0-letta-zep-compared); [Hindsight roundup](https://hindsight.vectorize.io/blog/2026/08/11/open-source-agent-memory-systems)
- Benchmarks: paper (ECAI 2025, arXiv 2504.19413) claims +26% over OpenAI Memory on LOCOMO, 90% fewer tokens, 91% faster (vendor). Zep disputes, claiming +10% over Mem0 on LOCOMO; Letta challenged methodology. Mem0 reports LongMemEval 94.4 but Vectorize's independent run (2026-03-15) got 49.0. — [rywalker](https://rywalker.com/research/mem0); [digitalapplied](https://www.digitalapplied.com/blog/open-source-agent-memory-mem0-letta-zep-compared)
- Complaints: fact extraction "too dull and rigid" (HN commenter, Apr 2026); $19→$249 price cliff; pre-v2 LLM reconciliation could "quietly issue a DELETE" of a still-needed memory (blog, recovery via `history()`); SDK changelog added guards "against LLM-hallucinated IDs" and against "hallucinated metadata". — [rywalker](https://rywalker.com/research/mem0); [dev.to post on silent deletes](https://dev.to/mukesh_13/mem0-auto-resolves-memory-conflicts-for-you-until-it-silently-deletes-one-you-still-need-4f4m); [Mem0 SDK changelog](https://docs.mem0.ai/changelog/sdk)

**claude-mem (thedotmack/claude-mem, Apache-2.0)**
- v15.18.1; ~116.4k stars, 10.1k forks (README badge, read 2026-11-12); third-party trackers reported 74.8K (May 2026), 89.3k (Aug 2026), 96,874 (Jul 2026) — treat all as approximate. — [GitHub README](https://github.com/thedotmack/claude-mem); [augmentcode](https://www.augmentcode.com/learn/claude-mem-74k-stars-agent-memory)
- Architecture: per-agent hook adapters (`claude-mem hook claude-code <event>`; Claude Code events Setup, SessionStart, UserPromptSubmit, PreToolUse, PostToolUse, Stop, SubagentStop, SessionEnd) → local Bun HTTP worker → "observer" (a second LLM agent) compresses tool activity into observations + end-of-session summary. SQLite at `~/.claude-mem/claude-mem.db` with FTS5; Chroma (needs uv/Python) for vectors. Observer providers: hosted claude-mem observer (30-day trial then subscription), OpenRouter/Gemini key, or the user's Anthropic plan (default = Claude subscription, Haiku). — [GitHub README](https://github.com/thedotmack/claude-mem); [ssdnodes guide, Oct 2026](https://www.ssdnodes.com/learn/lang/vi/claude-mem-and-your-claude-limits)
- Retrieval: progressive disclosure MCP triple `search` (~50–100 tokens/result) → `timeline` → `get_observations` (~500–1,000 tokens/result); plus `smart_search`, `smart_outline`, `smart_unfold`, `save_memory`; every observation has a citable ID. — [GitHub README](https://github.com/thedotmack/claude-mem)
- Multi-agent: Claude Code, Codex, Cursor, OpenCode + ~11 others record; Copilot CLI, Goose, Roo Code, Warp are search-only; session-start context scoped to the current harness unless `CLAUDE_MEM_SESSION_START_INCLUDE_ALL_SOURCES`. "TeamBrain" optional shared workspace with member attribution. Cloud Sync opt-in to cmem.ai. — [GitHub README](https://github.com/thedotmack/claude-mem)
- Portability/backup: no export/import tooling described; data is local SQLite. `<private>` tags exclude content. Anonymous telemetry on by default. — [GitHub README](https://github.com/thedotmack/claude-mem)
- Weaknesses: README itself says token-savings ratio "is not a measured recall or end-to-end saving" (#4138); one Jan-2026 comparison measured ~42k cache-creation tokens per session and 1.485M total tokens vs 20k for a transcript-grep alternative and disabled it; PR #2067 proposed a 100k tokens/day observer budget; issues #630 (SessionStart hook never fires), #623 (crash-recovery loop), #621 (ANSI instead of JSON); dev.to review "great idea, poor implementation" over cost-heavy design; no injection until the second session; requires Node 20.19+/22.12+, Bun, uv. — [GitHub README](https://github.com/thedotmack/claude-mem); [claude-historian COMPARISON.md](https://glama.ai/mcp/servers/@Vvkmnn/claude-historian/blob/5523c7a4febebd9cb3e54990eb6bc693ee206aa5/COMPARISON.md); [dev.to review](https://dev.to/veltrea_code/-reading-yc-backed-code-1-claude-mem-great-idea-poor-implementation-2mbi)

**Letta (letta-ai/letta, Apache-2.0; formerly MemGPT)**
- Architecture: memory "blocks" (strings pinned into the system prompt, edited by the agent via memory tools or by developers via API; one block attachable to several agents = "shared block"); all messages persisted and retrievable after compaction; archival memory = vector-backed long-term store queried by tool calls. Docs page is labelled "V1 SDK (legacy)"; a newer "Agent SDK" exists. — [Letta docs: memory](https://docs.letta.com/guides/agents/memory); [digitalapplied](https://www.digitalapplied.com/blog/open-source-agent-memory-mem0-letta-zep-compared)
- Sleep-time compute (2025-04-21, arXiv 2504.13171): a second asynchronous agent edits core memory while the primary is idle; paper claims 5x less test-time compute for same accuracy on Stateful GSM-Symbolic / AIME (vendor, 2025). — [Letta blog](https://letta.com/blog/sleep-time-compute); [arXiv](https://arxiv.org/html/2504.13171v1)
- 2026 direction: "Letta's Next Phase" (Mar 2026) makes Letta Code the flagship, "a runtime with git-backed memory, skills, subagents"; Conversations API (Jan 2026) = shared agent memory across concurrent user interactions; Letta Code App (Apr 2026) local; Mods (Jun 2026) self-modifying harness; Letta Agents SDK (Aug 2026). — [Letta product blog index](https://www.letta.com/blog-categories/product)
- FrontierMemory 1.0 (announced ~2026-10-22): agents read past trajectories and write a memory folder, scored by how much it helps a fresh agent; narrowed to 15 tasks (±6.7 pp per error); seven frontier models compared; code Apache-2.0, corpus gated on Hugging Face; submissions need three runs. Also "Evaluating Memory in Production Agents" (Jul 2026). — [The New Stack, Oct 2026](https://thenewstack.io/letta-frontiermemory-agent-harness-benchmark/); [Letta research](https://letta.com/research)
- Portability: Agent File (.af) export format exists (post from Apr 2025; not re-verified for 2026). — [Letta product blog index](https://www.letta.com/blog-categories/product)
- Pricing: Free (BYO keys), Pro $20/mo, Developer usage-based, Teams/Enterprise; tool execution $0.00015 per CPU-second. Stars 24,135 / forks 2,569 (Aug 2026). — [digitalapplied](https://www.digitalapplied.com/blog/open-source-agent-memory-mem0-letta-zep-compared)
- Weaknesses: "an agent runtime rather than a drop-in memory layer"; LoCoMo 83.2 is vendor-claimed; Letta Leaderboard uses synthetic facts + GPT-4.1 judge (compares models, not memory products); casual users ~$100/mo, power users ≥$200/mo. — [digitalapplied](https://www.digitalapplied.com/blog/open-source-agent-memory-mem0-letta-zep-compared); [Hindsight roundup](https://hindsight.vectorize.io/blog/2026/08/11/open-source-agent-memory-systems)

**Zep / Graphiti (getzep/graphiti Apache-2.0; Zep Cloud proprietary)**
- Architecture: LLM extracts entities/relationships/facts from "episodes" into a temporal knowledge graph; each edge carries valid-at / invalid-at plus ingestion time; contradicted facts get an invalid-at timestamp instead of deletion. Backends Neo4j, FalkorDB (embedded FalkorDB lite mode). — [aktagon summary of paper, Jul 2026](https://signals.aktagon.com/articles/2026/07/zep-a-temporal-knowledge-graph-architecture-for-agent-memory/); [rywalker Zep](https://rywalker.com/research/zep); [arXiv 2501.13956 (Jan 2025)](https://arxiv.org/abs/2501.13956v1)
- Retrieval: semantic + keyword + graph search over subgraphs, formatted for token efficiency. Zep site claims 94.7% on "memory benchmarks" and 161 ms p95 retrieval at 10M-graph scale (vendor). — [rywalker Zep](https://rywalker.com/research/zep)
- Releases 2026: v0.28.2 (Mar, Cypher-injection hardening in search filters), v0.29.0 (Apr), v0.29.2 (Jun 8, FalkorDB fixes). — [rywalker Zep](https://rywalker.com/research/zep)
- Benchmarks (vendor): DMR 94.8% vs MemGPT 93.4%; up to 18.5% aggregate gain on LongMemEval (~115k-token conversations). Supermemory's own LongMemEval-S table puts Zep at 71.2% overall (competitor-run). — [Zep blog](https://blog.getzep.com/zep-a-temporal-knowledge-graph-architecture-for-agent-memory); [callsphere](https://callsphere.ai/blog/vw3g-zep-memory-v2-temporal-knowledge-graph-graphiti-2026.md); [Supermemory research](https://supermemory.ai/research.md)
- Pricing: Free 1,000 credits/mo; Flex $1,250/yr (~$104/mo) 50,000 credits; Flex Plus $3,750/yr (~$312/mo) 200,000 credits; 1 credit per 350 bytes ingested; retrieval free; $25/mo tier eliminated; Zep Community Edition deprecated (Apr 2025), legacy getzep/zep server (4.7–4.8K stars) no longer developed; self-host = Graphiti only. — [rywalker Zep](https://rywalker.com/research/zep); [digitalapplied](https://www.digitalapplied.com/blog/open-source-agent-memory-mem0-letta-zep-compared); [agentsroom](https://agentsroom.dev/blog/ai-agent-memory-solutions)
- Adoption: Graphiti ~27.3K stars (Jun 2026), 29,642 / 2,998 forks (Aug 2026). — [rywalker Zep](https://rywalker.com/research/zep); [digitalapplied](https://www.digitalapplied.com/blog/open-source-agent-memory-mem0-letta-zep-compared)
- Complaints: credit math unpredictable; self-hosted edition deprecated; early Reddit production performance problems (Zep later documented a 30x scaling fix); Show HN questioned reinventing ontologies vs RDF; needs an external graph DB. — [rywalker Zep](https://rywalker.com/research/zep)

**OpenViking (volcengine/OpenViking, AGPL-3.0 core)**
- v0.3.22 benchmarked; desktop app 0.0.19 beta; 42.1k stars, 3.5k forks, 3,246 commits, 482 open issues (README read 2026-11-12). Papers: arXiv 2605.29640 (VikingMem, VLDB 2026), 2606.16903 (ICDE), 2609.11390. Listed as open-sourced 2026-01-15 by one directory; other coverage dates it Aug 2026. — [GitHub README](https://github.com/volcengine/OpenViking); [jimmysong landscape](https://landscape.jimmysong.io/projects/open-viking/)
- Architecture: context as a `viking://` virtual filesystem (`resources/`, `user/{user_id}/{memories,resources,skills,peers}`); tiered loading L0 `.abstract.md` (one sentence) / L1 `.overview.md` / L2 full; semantic search scoped to directories, TrieHI resolves directory scope before vector ranking; requires an embedding model and a VLM (Volcengine, OpenAI, Codex OAuth, Kimi, GLM, local Ollama). — [GitHub README](https://github.com/volcengine/OpenViking)
- Extraction: committing a session archives the conversation and extracts memories as Markdown in the background. — [GitHub README](https://github.com/volcengine/OpenViking)
- Multi-user: accounts, user isolation, opt-in resource ACLs; user keys for memory access; root key cannot read/write memories. — [GitHub README](https://github.com/volcengine/OpenViking)
- Portability: memories and sessions are plain Markdown files; `ov add-resource` imports URLs/repos; no explicit export command; migration tooling to managed service. — [GitHub README](https://github.com/volcengine/OpenViking)
- Benchmarks (vendor, Doubao models): LoCoMo accuracy OpenClaw 24.20→82.08%, Hermes 33.38→82.86%, Claude Code 57.21→80.32%; input tokens −34.3 to −91.0%; tau2-bench retail 70.94→77.81%, airline 54.38→66.25%; scripts in `./benchmark`. — [GitHub README](https://github.com/volcengine/OpenViking)
- Licence/limits: AGPL-3.0 core (CLI crates Apache-2.0); README forbids wholesale rewrite/re-open-sourcing and requires written authorization for translations; distributed deployment only in paid Self-Managed edition; hosting outside China planned; free tier 50 files; Windows via beta desktop app only. — [GitHub README](https://github.com/volcengine/OpenViking)

**Supermemory (supermemoryai; MIT public repo, hosted API)**
- Benchmarks (vendor, May 2026): LongMemEval-S Recall@20 97% overall vs Zep 71.2% vs full-context 60.2% (per category: knowledge update 100/83.3/78.2; temporal 95/62.4/45.1; multi-session 96/57.9/44.3); claims "#1" on LoCoMo and ConvoMem without scores; SMFS/xAFS: 3x fewer cumulative tokens on Claude, 1.75x on Codex over 110 questions. Founder-announced ~99% LongMemEval via "ASMR" agent-swarm variant (not standard setup); earlier 85.86% / 81.6%. Latency: 187 ms server, 356 ms end-to-end per query (vendor). Released MemoryBench (open-source benchmark suite). — [Supermemory research](https://supermemory.ai/research.md); [dev.to review](https://dev.to/andrew-ooo/supermemory-review-1-ai-memory-engine-on-longmemeval-3k0e); [Supermemory changelog 2026-01-05](https://supermemory.ai/changelog/2026-01-05-improvements-and-fixes.md)
- Pricing: ~$19 / $100 / $399 per month tiers (Oct 2026); self-hosting "free within lite license limit of 10,000 stored documents"; managed self-hosting is Scale/Enterprise. — [rywalker Supermemory](https://rywalker.com/research/supermemory); [Supermemory self-hosting docs](https://supermemory.ai/docs/self-hosting)
- Licence conflict: public repo MIT (~26.8K stars, Jun 2026), SDKs Apache-2.0, but the self-hosted server binary is "built from a separate, non-public codebase" per vendor statement cited by Developers Digest; Vectorize calls it closed-source/enterprise-only; Supermemory blog claims "local mode runs fully offline as a single binary under MIT". — [developersdigest](https://www.developersdigest.tech/agent-memory/supermemory); [vectorize](https://vectorize.io/articles/supermemory-alternatives); [Supermemory blog](https://supermemory.ai/blog/should-you-build-your-own-ai-memory-system.md)
- Weakness: Sept 2026 independent review notes claims are documentation-based, no hands-on reproduction; API compatibility ≠ cloud benchmark results. — [dev.to review](https://dev.to/andrew-ooo/supermemory-review-1-ai-memory-engine-on-longmemeval-3k0e)

**engram (Gentleman-Programming/engram, MIT)**
- Single Go binary, SQLite + FTS5 at `~/.engram/engram.db`; CLI, HTTP API, MCP over stdio, TUI; no Node/Python/Docker; no vector search mentioned in README. Homebrew install; v1.0.1 replaced the cask. — [README](https://cdn.jsdelivr.net/gh/Gentleman-Programming/engram@main/README.md); [themenonlab](https://themenonlab.blog/blog/engram-persistent-memory-ai-coding-agents)
- MCP: link says "27 tools" (other sources say 13–14, probably older); named: `mem_current_project, mem_context, mem_search, mem_timeline, mem_get_observation, mem_save, mem_update, mem_suggest_topic_key, mem_consolidate, mem_save_prompt, mem_session_summary, mem_session_start/end/finalize, mem_review, mem_judge, mem_compare, mem_judge_many, mem_doctor`; tool set varies by "MCP profile". — [README](https://cdn.jsdelivr.net/gh/Gentleman-Programming/engram@main/README.md)
- Writing: agent-written via `mem_save` under a "memory protocol" (decisions, bug fixes, discoveries; not raw tool output); session summaries recover after compaction; "conflict surfacing" flags related/conflicting memories for the agent to judge (`mem_judge`, `mem_compare`). — [README](https://cdn.jsdelivr.net/gh/Gentleman-Programming/engram@main/README.md)
- Scoping: project from explicit selection → `ENGRAM_PROJECT` → cwd detection; `--all`/`all_projects=true` for global reads; refuses with `project_transition_conflict` instead of switching silently; 14 setup targets (Claude Code, Codex, Cursor, Gemini CLI, OpenCode...). — [README](https://cdn.jsdelivr.net/gh/Gentleman-Programming/engram@main/README.md)
- Sync/portability: optional Engram Cloud (project-scoped replication, shared access, browser view); "Git Sync exports portable compressed chunks"; local SQLite stays authoritative; Obsidian Brain export (beta); `migrate-copy`. — [README](https://cdn.jsdelivr.net/gh/Gentleman-Programming/engram@main/README.md)
- Adoption: ~2,484 stars (one directory). Weakness: API logs and TUI copy actions can expose memory content (review). — [meshkore directory](https://meshkore-web.pages.dev/agent/gentleman-programming-engram); [gearscope review, May 2026](https://gearscope.pages.dev/reviews/engram)

**Anthropic: Claude Code auto memory + API memory tool**
- Auto memory: Claude writes notes itself; four types in frontmatter `type`: `user`, `feedback`, plus two more (project/reference). Storage `~/.claude/projects/<project>/memory/` with `MEMORY.md` index + one topic file per memory; `<project>` derived from the git repo so all worktrees share one directory; machine-local, not shared across machines. First 200 lines or 25 KB of MEMORY.md loaded each session; on over-limit write Claude Code returns an error telling Claude to rewrite the index; topic files loaded on demand. Claude Code records a `modified` ISO-8601 frontmatter timestamp on write "to show how current the fact is". Subagents can have their own auto memory (`memory` field). Toggle via `/memory`, `autoMemoryEnabled`, `CLAUDE_CODE_DISABLE_AUTO_MEMORY=1`, `autoMemoryDirectory`; off by default in self-hosted/background sessions. Memory files excluded from `cleanupPeriodDays` sweep. — [Claude Code docs: memory](https://code.claude.com/docs/en/memory)
- Dates (secondary): first version v2.1.32 (2026-02-05), `/memory` management in v2.1.59 (Feb 2026), 25 KB cap added v2.1.83 (2026-03-24); "auto dream" consolidation reported under test Mar 2026 (unconfirmed). — [issoh.co.jp](https://www.issoh.co.jp/md/tech/10919); [tessl](https://tessl.io/blog/anthropic-tests-auto-dream-to-clean-up-claudes-memory/)
- API memory tool `memory_20250818`: client-side; commands view/create/str_replace/insert/delete/rename under `/memories`; developer implements storage; API auto-injects a "MEMORY PROTOCOL ... ASSUME INTERRUPTION" system instruction; SDK helpers `BetaLocalFilesystemMemoryTool`; security guidance: strip sensitive data, cap file sizes, expire unaccessed files, path-traversal checks; pairs with context editing and compaction. — [Memory tool docs](https://platform.claude.com/docs/en/agents-and-tools/tool-use/memory-tool)
- Complaints: "Auto-memory not persisted across sessions despite standing instruction — causes significant token waste" (issue #65173); background classification agents consuming ~10% of 5h budget per minute (#102236). — [claudeissues #65173](https://claudeissues.com/issue/65173-auto-memory-not-persisted-across-sessions-despite-standing-instruction-causes-si); [claudeissues #102236](https://claudeissues.com/issue/102236-bug-background-classification-agents-consume-10-of-5h-token-usage-per-minute)

**GitHub Copilot memory + Dreaming**
- Public preview 2026-01-15 (all paid plans); GA 2026-10-28 at GitHub Universe. — [byteiota](https://byteiota.com/github-copilot-memory-28-day-context-goes-live/); [Unblocked blog](https://getunblocked.com/blog/github-agent-memory-only-knows-github/)
- Two kinds: repository-level facts (shared by collaborators, not pooled across org, not copied on fork) and user-level preferences (across all repos). Copilot creates memories itself; bulk import via Copilot CLI. Repository facts cite supporting code; before use, Copilot checks citations against the current branch ("a judgment, not a guarantee"); it upvotes validated memories, downvotes wrong/outdated ones; heavily downvoted memories stop being used and are deleted after disuse. Expiry: 90 days default without reinforcement (upvote resets, retrieval does not); org/enterprise can set 7–180 days. Write access needed to create repo facts; admins on Business/Enterprise can view/export/delete user preferences. — [GitHub docs: Copilot memory](https://docs.github.com/en/copilot/concepts/agents/copilot-memory)
- Dreaming (public preview, opt-in, consumes AI credits): reads synced sessions, memories, repo content, PR review feedback; edits memories directly; changes to `.github/copilot-instructions.md`, `AGENTS.md`, skill files go through a pull request. GitHub-reported preview results: 93% of memory-using CLI sessions showed better preference adherence, 42% more efficient workflows (vendor). — [Unblocked blog](https://getunblocked.com/blog/github-agent-memory-only-knows-github/)
- Criticisms: only serves Copilot; facts can come from closed, unmerged PRs; repo-wide sharing raises privacy concerns; earlier reporting said 28-day expiry (superseded by 90-day docs). — [Unblocked blog](https://getunblocked.com/blog/github-agent-memory-only-knows-github/); [byteiota](https://byteiota.com/github-copilot-memory-28-day-context-goes-live/)

**Cursor Memories**
- Shipped in Cursor 1.0 beta 2025-06-04 ("stored per project on an individual level"); removed from 2.1.17 (2025-11-24), confirmed intentional by staff 2026-01-07; docs URL redirects to Rules. Replacements: `.cursor/rules`, AGENTS.md, Team Rules; Automations memory (Mar 2026, per-automation notes); Projects beta (Sep 2026, Cloud Agents only); MCP memory servers. Complaint: a 2.2 user's agent still reported "32 memories" with no UI to remove them; workaround rollback to 2.0.77. — [memoryplugin blog (vendor of an MCP memory product)](https://blog.memoryplugin.com/cursor-memory/)

**Other notable 2026 entrants**
- Hindsight (Vectorize, MIT, ~19.6K stars Aug 2026; one source says 50K in 350 days): single embedded PostgreSQL; four parallel retrievers (semantic, BM25, graph traversal, temporal) + cross-encoder rerank; vendor claims LongMemEval-s 94.6%, LoCoMo 92.0%, BEAM 73.9%, LifeBench 71.5%, PersonaMem 86.6%; lacks native temporal-validity modelling. — [Hindsight roundup (vendor)](https://hindsight.vectorize.io/blog/2026/08/11/open-source-agent-memory-systems); [Hindsight blog tags](https://hindsight.vectorize.io/blog/tags/open-source)
- Cognee (Apache-2.0, ~30.0K stars): ECL (extract-cognify-load) graph pipeline, embedded SQLite + LanceDB + Kuzu, MCP server, code-graph indexing; Python-only; rankings are self-published. — [Hindsight roundup](https://hindsight.vectorize.io/blog/2026/08/11/open-source-agent-memory-systems); [Cognee blog](https://www.cognee.ai/memory-agent-for-ai-coding-tools)
- AgentMemory (rohitg00, SQLite, for Claude/Cursor/Codex): background capture + auto-inject; "95.2% retrieval accuracy" on its own test suite. — [shiporskip](https://shiporskip.io/compare/agentmemory-persistent-memory-ai-coding-agents-sqlite-2026-vs-opencode-terminal-ai-coding-agent-75-models-2026); [agentmemory COMPARISON.md](https://cdn.jsdelivr.net/gh/rohitg00/agentmemory@main/benchmark/COMPARISON.md)
- HaluMem benchmark (memtensor): evaluates hallucination in memory systems incl. a dynamic-update column; Mem0 scored 40.18% on one column. — [besthub HaluMem](https://www.besthub.dev/articles/halumem-evaluating-hallucinations-in-ai-agent-memory-systems-29392bd5f14e)
- STALE paper (May 2026): tests whether agents notice a newer fact superseding an older memory; most fail. — [hackernoon](https://hackernoon.com/the-most-dangerous-memory-in-an-ai-agent-is-the-one-that-used-to-be-true)

### Inferences
- The two biggest coding-agent-specific competitors by stars (claude-mem ~116K, OpenViking ~42K) both depend on an LLM/VLM for the write path; AutoMemory's "no LLM unless configured" is a real differentiator but means it has no built-in summarisation of raw tool activity, which is what users of claude-mem actually get.
- Mem0 reversed course in April 2026 from LLM reconciliation (ADD/UPDATE/DELETE) to ADD-only plus search-time decay and expiration dates — evidence that LLM-driven deletion was too dangerous. AutoMemory's deterministic approach is aligned with where the market went.
- Copilot is the only system doing deterministic-ish re-validation of memories against code (citations checked on the current branch) — the closest analogue to AutoMemory's hash verification, but with vote counts and expiry on top.

### Gaps
- Letta's 2026 "git-backed memory" / "context repositories" architecture: the announcement blog and docs pages returned 404/403; only the one-line description from the blog index is sourced.
- Letta `.af` Agent File: 2026 status not verified (only an Apr-2025 post is indexed).
- Supermemory pricing page redirected and then 404'd; tiers come from third-party pages dated Jun–Oct 2026.
- Exact claude-mem observer token costs: only one user's measurement; no official figure.
- MemOS: no 2026 sources found.
- Anthropic "auto dream": only secondary reports; not in official docs fetched.

## Q1. How does each system decide what to store and how to update or delete existing memories?

### Takeaway
Three patterns: LLM reconciliation at write time (Zep/Graphiti edge invalidation, pre-v2 Mem0, OpenViking consolidation, claude-mem observer), agent-written files with LLM-judged edits (Claude Code auto memory, Anthropic memory tool, Letta blocks, Letta Code git memory, engram `mem_save`), and deterministic-plus-vote systems (Copilot upvote/downvote/expiry; Mem0 v2 ADD-only + decay + expiration). Nobody except AutoMemory uses hash-based verification as the primary update signal.

### Cited Findings
- Mem0 v2.0.0 (2026-04-14): "ADD-only extraction: memories accumulate and nothing is overwritten or deleted"; decay (2026-05-08) only reorders; `expiration_date` (2026-06-27). — [Mem0 changelog](https://docs.mem0.ai/changelog/highlights)
- Pre-v2 Mem0: each `add()` ran an LLM pipeline that could issue DELETE; "when the model gets a distinction wrong, it quietly issues a DELETE". — [dev.to](https://dev.to/mukesh_13/mem0-auto-resolves-memory-conflicts-for-you-until-it-silently-deletes-one-you-still-need-4f4m)
- Graphiti: contradicted facts get `invalid_at` set, never deleted. — [aktagon](https://signals.aktagon.com/articles/2026/07/zep-a-temporal-knowledge-graph-architecture-for-agent-memory/)
- Claude Code auto memory: Claude decides; types `user`, `feedback`...; Claude Code enforces index size limits and reminds Claude to "merge or drop stale entries". — [Claude Code docs](https://code.claude.com/docs/en/memory)
- Anthropic memory tool: `create` may overwrite; `str_replace` requires unique match; `delete`/`rename` of root rejected; docs tell developers to expire unaccessed files themselves. — [Memory tool docs](https://platform.claude.com/docs/en/agents-and-tools/tool-use/memory-tool)
- Copilot: Copilot creates memories; upvotes validated ones, downvotes wrong ones; heavily downvoted stop being used and are deleted; 90-day expiry unless reinforced. — [GitHub docs](https://docs.github.com/en/copilot/concepts/agents/copilot-memory)
- Copilot Dreaming edits memories directly but routes instruction-file changes through a PR. — [Unblocked](https://getunblocked.com/blog/github-agent-memory-only-knows-github/)
- claude-mem: observer agent compresses tool activity into observations automatically; `<private>` opt-out. — [README](https://github.com/thedotmack/claude-mem)
- engram: agent-written under a protocol; `mem_consolidate`, `mem_judge`, `mem_compare` for conflicts. — [README](https://cdn.jsdelivr.net/gh/Gentleman-Programming/engram@main/README.md)
- OpenViking: post-session background extraction to Markdown; VikingMem paper describes event-driven extraction, update and consolidation. — [README](https://github.com/volcengine/OpenViking)
- Letta: agent edits memory blocks via tools; sleep-time agent rewrites core memory offline. — [Letta docs](https://docs.letta.com/guides/agents/memory); [Letta blog](https://letta.com/blog/sleep-time-compute)
- Mem0 editor plugins (Pi, OpenCode) and Copilot both ship "dream" consolidation passes; Anthropic reportedly testing "auto dream". — [Mem0 changelog](https://docs.mem0.ai/changelog/highlights); [tessl](https://tessl.io/blog/anthropic-tests-auto-dream-to-clean-up-claudes-memory/)

### Inferences
- "Dreaming"/offline consolidation is now table stakes across Mem0, Copilot, Letta and (reportedly) Claude Code. AutoMemory has no scheduled consolidation pass; a deterministic one (merge duplicates by hash/FTS similarity, flag contradictions, demote unverified claims) would match the pattern without an LLM.
- Vote-plus-expiry (Copilot) is a cheap, deterministic staleness signal AutoMemory could add alongside hash verification.

### Gaps
- No primary source describes Supermemory's or Hindsight's write-time update/delete policy.

## Q2. What retrieval stack do they use and what numbers do they publish?

### Takeaway
Hybrid (vector + BM25) with optional rerank is the 2026 default; graph traversal (Graphiti, Hindsight, Cognee, Mem0 built-in graph) and temporal parsing (Mem0, Graphiti, Hindsight) are the differentiators. All published numbers are vendor-run on LoCoMo / LongMemEval / BEAM; the one independent re-run found Mem0 at 49.0 vs its claimed 94.4 on LongMemEval.

### Cited Findings
- Mem0 v2: semantic + BM25 + entity boost in parallel; rerankers (Cohere, ZeroEntropy, cross-encoder, LLM); vendor LoCoMo 91.6, LongMemEval 93.4, BEAM 64.1; temporal query parsing adds ~1 ms median / ~198 ms p95. — [Mem0 changelog](https://docs.mem0.ai/changelog/highlights); [chatforest](https://chatforest.com/reviews/mem0-mcp-server/)
- Independent: Vectorize measured Mem0 LongMemEval 49.0 (2026-03-15) vs Mem0's 94.4. — [digitalapplied](https://www.digitalapplied.com/blog/open-source-agent-memory-mem0-letta-zep-compared)
- Graphiti: semantic + keyword + graph; Zep claims 161 ms p95 at 10M-graph; DMR 94.8 vs MemGPT 93.4; +18.5% LongMemEval (vendor). — [rywalker Zep](https://rywalker.com/research/zep)
- Supermemory: Recall@20 with aggregation; LongMemEval-S 97% (vendor, May 2026); 187/356 ms latency. — [Supermemory research](https://supermemory.ai/research.md)
- Hindsight: four parallel retrievers + cross-encoder; LongMemEval-s 94.6, LoCoMo 92.0, BEAM 73.9 (vendor). — [Hindsight roundup](https://hindsight.vectorize.io/blog/2026/08/11/open-source-agent-memory-systems)
- OpenViking: directory-scoped vector search (TrieHI), L0/L1/L2 tiered loading; LoCoMo 80–83% with coding agents vs 24–57% native (vendor, Doubao). — [README](https://github.com/volcengine/OpenViking)
- claude-mem: FTS5 + Chroma; progressive disclosure 50–100 → 500–1,000 tokens; token-savings claim explicitly not measured (#4138). — [README](https://github.com/thedotmack/claude-mem)
- engram: FTS5 only; progressive `mem_search` → `mem_timeline` → `mem_get_observation`. — [README](https://cdn.jsdelivr.net/gh/Gentleman-Programming/engram@main/README.md)
- Claude Code auto memory: no search; index (≤200 lines/25 KB) loaded, topic files read on demand. — [Claude Code docs](https://code.claude.com/docs/en/memory)
- Letta FrontierMemory: scores memory by downstream task gain, 15 tasks, ±6.7 pp. — [The New Stack](https://thenewstack.io/letta-frontiermemory-agent-harness-benchmark/)
- Letta Leaderboard criticised: synthetic facts, GPT-4.1 judge. — [digitalapplied](https://www.digitalapplied.com/blog/open-source-agent-memory-mem0-letta-zep-compared)

### Inferences
- AutoMemory publishes no benchmark; running LongMemEval-S or MemoryBench locally (both open) would be the cheapest credibility gain, and the Vectorize-vs-Mem0 gap shows independent numbers are scarce and valued.
- Temporal query parsing ("as of", "last week") is deterministic and cheap to add on top of existing timestamps.

### Gaps
- No independent coding-specific memory benchmark exists; FrontierMemory is the closest and is tiny.

## Q3. Which support several agents or users sharing one memory, and how do they handle provenance and trust?

### Takeaway
Multi-agent sharing is common (claude-mem TeamBrain, engram Cloud, Letta shared blocks + Conversations, Copilot repo facts, Mem0 user/agent/run scopes, OpenViking per-user ACLs), but provenance is thin: claude-mem has member attribution, Copilot has code citations and upvotes, Graphiti has ingestion timestamps. No competitor has explicit trust tiers like AutoMemory's five.

### Cited Findings
- claude-mem: ~15 agents write, 4 search-only; per-harness context scoping; TeamBrain "shared workspace with member attribution". — [README](https://github.com/thedotmack/claude-mem)
- engram: 14 agent targets; Cloud gives project-scoped replication and shared access; refuses silent project switches. — [README](https://cdn.jsdelivr.net/gh/Gentleman-Programming/engram@main/README.md)
- Letta: shared memory blocks across agents; Conversations API (Jan 2026) for shared memory across concurrent interactions. — [Letta docs](https://docs.letta.com/guides/agents/memory); [Letta blog index](https://www.letta.com/blog-categories/product)
- Mem0: scopes user_id / agent_id / run_id / app; Strands integration "entity scoping". — [Hindsight roundup](https://hindsight.vectorize.io/blog/2026/08/11/open-source-agent-memory-systems); [Mem0 changelog](https://docs.mem0.ai/changelog/highlights)
- Copilot: repo facts shared by collaborators (write access to create, read to use), cite code; user prefs cite the user's words; admins can export/delete. — [GitHub docs](https://docs.github.com/en/copilot/concepts/agents/copilot-memory)
- OpenViking: accounts, user isolation, opt-in resource ACLs, root key cannot read memories. — [README](https://github.com/volcengine/OpenViking)
- Claude Code auto memory: per-machine, per-repo; subagent memory separate; not shared across machines. — [Claude Code docs](https://code.claude.com/docs/en/memory)
- Copilot criticised for siloed memory unreadable by Claude Code/Cursor. — [Unblocked](https://getunblocked.com/blog/github-agent-memory-only-knows-github/)

### Inferences
- AutoMemory's cross-agent sharing plus trust tiers is ahead of all local competitors; the gap is user-facing attribution UI (claude-mem viewer, engram TUI/browser) and admin export of a user's memories (Copilot).

### Gaps
- No source describes how claude-mem TeamBrain or engram Cloud resolve conflicting writes from two agents.

## Q4. Which detect that a memory went stale?

### Takeaway
Only Copilot (citation re-check against current branch + votes + expiry) and Graphiti (bi-temporal invalidation on contradiction) have explicit staleness mechanisms; Mem0 added decay and expiration dates in 2026; Claude Code adds a `modified` timestamp only. AutoMemory's hash-based verification is unique among local tools.

### Cited Findings
- Copilot: citations validated against current branch before use; "does not guarantee"; facts may come from closed PRs; 90-day decay. — [GitHub docs](https://docs.github.com/en/copilot/concepts/agents/copilot-memory); [Unblocked](https://getunblocked.com/blog/github-agent-memory-only-knows-github/)
- Graphiti: valid_at / invalid_at edges. — [aktagon](https://signals.aktagon.com/articles/2026/07/zep-a-temporal-knowledge-graph-architecture-for-agent-memory/)
- Mem0: decay 0.3x–1.5x ranking bias; `expiration_date`; temporal reasoning. — [Mem0 changelog](https://docs.mem0.ai/changelog/highlights)
- Claude Code: `modified` frontmatter timestamp "shows how current the fact is". — [Claude Code docs](https://code.claude.com/docs/en/memory)
- Hindsight "lacks native temporal-validity modelling". — [Hindsight roundup](https://hindsight.vectorize.io/blog/2026/08/11/open-source-agent-memory-systems)
- Problem framing: STALE paper (May 2026) — most agents fail to notice supersession; blogs call agent memory "an append-only log in a trench coat" and "a cache with no invalidation policy". — [hackernoon](https://hackernoon.com/the-most-dangerous-memory-in-an-ai-agent-is-the-one-that-used-to-be-true); [melodykoh](https://melodykoh.substack.com/p/the-memory-problem-isnt-retrieval); [tianpan](https://tianpan.co/blog/2026/05/16/agent-memory-cache-without-invalidation-policy)
- HaluMem benchmark measures dynamic-update hallucination. — [besthub](https://www.besthub.dev/articles/halumem-evaluating-hallucinations-in-ai-agent-memory-systems-29392bd5f14e)

### Inferences
- Missing in AutoMemory relative to Copilot/Graphiti: a "superseded_by" link and valid-from/valid-to on notes, so a contradicted note stays queryable as history instead of being archived; and usage-based reinforcement (last-used counter) to drive decay.

### Gaps
- No competitor source documents detection of *contradiction between two memories* without an LLM.

## Q5. What export/import formats exist; can a user leave with their data?

### Takeaway
Portability is weakest in the hosted APIs (Mem0, Zep, Supermemory: no export documented) and strongest in file-shaped systems (Claude Code Markdown, OpenViking Markdown, Letta .af, engram Git Sync chunks + Obsidian export, Copilot admin export). AutoMemory's backup/restore + encrypted git vault is competitive; a plain-Markdown/JSON export would close the remaining gap.

### Cited Findings
- Mem0 CLI has `import`; no export documented in changelog or reviews. — [Mem0 changelog](https://docs.mem0.ai/changelog/highlights); [rywalker](https://rywalker.com/research/mem0)
- Zep/Graphiti, Supermemory: export "not covered" in all sources read. — [rywalker Zep](https://rywalker.com/research/zep); [Supermemory research](https://supermemory.ai/research.md)
- claude-mem: no export/import tooling; local SQLite; opt-in cloud sync. — [README](https://github.com/thedotmack/claude-mem)
- engram: Git Sync portable compressed chunks; Obsidian Brain export (beta); `migrate-copy`. — [README](https://cdn.jsdelivr.net/gh/Gentleman-Programming/engram@main/README.md)
- OpenViking: memories are plain Markdown; `add-resource` import; no export command. — [README](https://github.com/volcengine/OpenViking)
- Claude Code: plain Markdown files, editable/deletable. — [Claude Code docs](https://code.claude.com/docs/en/memory)
- Copilot: bulk import via CLI; admins can export user preferences. — [GitHub docs](https://docs.github.com/en/copilot/concepts/agents/copilot-memory)
- Letta: Agent File (.af) export format (Apr 2025 post). — [Letta blog index](https://www.letta.com/blog-categories/product)

### Inferences
- An "import from Claude Code auto memory / claude-mem SQLite / Mem0 JSON" command would be a cheap acquisition feature; Copilot's bulk import and Mem0 CLI import show demand.

### Gaps
- Could not verify any hosted vendor's data-export API in 2026 docs.

## Q6. What do users complain about?

### Takeaway
Cost of LLM-driven memory (claude-mem observer, Mem0 per-add LLM calls, Zep credits, Letta usage), silent/incorrect deletions, unverifiable vendor benchmarks, deprecated self-host options (OpenMemory, Zep CE, Cursor Memories), security holes in servers (Mem0 CVEs, Graphiti Cypher injection), and stale memories nobody notices.

### Cited Findings
- claude-mem: 1.485M tokens vs 20k alternative; "cost-heavy design"; hooks not firing (#630), crash loop (#623). — [claude-historian comparison](https://glama.ai/mcp/servers/@Vvkmnn/claude-historian/blob/5523c7a4febebd9cb3e54990eb6bc693ee206aa5/COMPARISON.md); [dev.to](https://dev.to/veltrea_code/-reading-yc-backed-code-1-claude-mem-great-idea-poor-implementation-2mbi)
- Mem0: rigid facts; pricing cliff; silent deletes; CVE-2026-31241/31244; OpenMemory sunset; breaking v2. — [rywalker](https://rywalker.com/research/mem0); [chatforest](https://chatforest.com/reviews/mem0-mcp-server/); [Mend](https://www.mend.io/vulnerability-database/CVE-2026-31241/)
- Zep: credit math, deprecated self-host, early perf issues, ontology reinvention. — [rywalker Zep](https://rywalker.com/research/zep)
- Letta: runtime not memory layer; $100–200+/mo; leaderboard methodology. — [digitalapplied](https://www.digitalapplied.com/blog/open-source-agent-memory-mem0-letta-zep-compared)
- OpenViking: AGPL + translation clause; 482 open issues; China-only hosting; VLM required. — [README](https://github.com/volcengine/OpenViking)
- Supermemory: licence ambiguity on self-host binary; unreproduced benchmarks. — [developersdigest](https://www.developersdigest.tech/agent-memory/supermemory); [dev.to](https://dev.to/andrew-ooo/supermemory-review-1-ai-memory-engine-on-longmemeval-3k0e)
- Cursor: feature removed; orphaned memories still injected. — [memoryplugin](https://blog.memoryplugin.com/cursor-memory/)
- Copilot: siloed; privacy of repo-wide memories; facts from unmerged PRs. — [Unblocked](https://getunblocked.com/blog/github-agent-memory-only-knows-github/)
- Claude Code: auto memory not persisting (#65173); background agents burning budget (#102236). — [claudeissues](https://claudeissues.com/issue/65173-auto-memory-not-persisted-across-sessions-despite-standing-instruction-causes-si)
- engram: logs/TUI can leak memory content. — [gearscope](https://gearscope.pages.dev/reviews/engram)
- CLAUDE.md-style memory "goes stale silently", doesn't scale past a few hundred facts. — [agentsroom](https://agentsroom.dev/blog/ai-agent-solutions)

### Inferences
- AutoMemory's positioning ("no LLM unless configured", local, deterministic) directly answers the top three complaint classes (cost, silent deletes, lock-in); the marketing gap is that nobody can see numbers proving it.

### Gaps
- No Reddit/HN thread specific to engram or OpenViking complaints was found.

## Feature gap table

### Takeaway
AutoMemory lacks mainly consolidation, temporal/supersession modelling, benchmarks, a viewer, and import paths; it is ahead on trust tiers, hash verification, locks, zero-LLM operation, encrypted sync and backup.

### Cited Findings

| Feature AutoMemory lacks | Who has it | Source |
|---|---|---|
| Offline consolidation / "dream" pass that merges, dedupes, demotes | Mem0 plugins (auto-dream), Copilot Dreaming (PR for instruction files), Letta sleep-time, Claude Code (reported) | [Mem0 changelog](https://docs.mem0.ai/changelog/highlights); [Unblocked](https://getunblocked.com/blog/github-agent-memory-only-knows-github/); [Letta](https://letta.com/blog/sleep-time-compute) |
| Bi-temporal validity (valid_at / invalid_at, superseded-not-deleted) | Graphiti | [aktagon](https://signals.aktagon.com/articles/2026/07/zep-a-temporal-knowledge-graph-architecture-for-agent-memory/) |
| Usage-based reinforcement + expiry (upvote/downvote, 7–180 day TTL) | Copilot; Mem0 decay + expiration_date | [GitHub docs](https://docs.github.com/en/copilot/concepts/agents/copilot-memory); [Mem0 changelog](https://docs.mem0.ai/changelog/highlights) |
| Temporal query parsing ("as of", "last week") | Mem0 Platform v3, Hindsight temporal retriever | [Mem0 changelog](https://docs.mem0.ai/changelog/highlights) |
| Automatic capture of tool activity into observations (no agent discipline needed) | claude-mem observer, AgentMemory, OpenViking post-session extraction | [claude-mem README](https://github.com/thedotmack/claude-mem); [OpenViking README](https://github.com/volcengine/OpenViking) |
| Progressive-disclosure retrieval with per-step token budgets and citable IDs | claude-mem, engram | [claude-mem README](https://github.com/thedotmack/claude-mem); [engram README](https://cdn.jsdelivr.net/gh/Gentleman-Programming/engram@main/README.md) |
| Tiered summaries (L0 abstract / L1 overview / L2 full) per memory | OpenViking | [OpenViking README](https://github.com/volcengine/OpenViking) |
| Web viewer / TUI for browsing memory and attribution | claude-mem viewer, engram TUI, Copilot settings UI | [claude-mem README](https://github.com/thedotmack/claude-mem); [engram README](https://cdn.jsdelivr.net/gh/Gentleman-Programming/engram@main/README.md) |
| Published benchmark numbers (LongMemEval-S, LoCoMo, MemoryBench) | Mem0, Zep, Supermemory, Hindsight, OpenViking (all vendor) | see Q2 |
| Import from other memory stores | Mem0 CLI `import`, Copilot bulk import | [Mem0 changelog](https://docs.mem0.ai/changelog/highlights); [GitHub docs](https://docs.github.com/en/copilot/concepts/agents/copilot-memory) |
| Plain-Markdown export / Obsidian export | Claude Code, OpenViking, engram | [Claude Code docs](https://code.claude.com/docs/en/memory); [engram README](https://cdn.jsdelivr.net/gh/Gentleman-Programming/engram@main/README.md) |
| Built-in graph / entity linking without external DB | Mem0 v2, Cognee (Kuzu), Hindsight | [Mem0 changelog](https://docs.mem0.ai/changelog/highlights); [Hindsight roundup](https://hindsight.vectorize.io/blog/2026/08/11/open-source-agent-memory-systems) |
| Reranking option (cross-encoder/LLM) | Mem0 TS v3.1, Hindsight | [Mem0 changelog](https://docs.mem0.ai/changelog/highlights) |
| Secret redaction on capture; `<private>` opt-out | Mem0 OpenCode plugin, claude-mem | [Mem0 changelog](https://docs.mem0.ai/changelog/highlights); [claude-mem README](https://github.com/thedotmack/claude-mem) |
| Per-subagent separate memory | Claude Code | [Claude Code docs](https://code.claude.com/docs/en/memory) |
| Wider agent coverage (~15 agents incl. OpenCode, Gemini CLI, Cursor, Hermes, OpenClaw) | claude-mem, engram (14), OpenViking (11+) | READMEs above |

| Feature where AutoMemory is ahead | Closest competitor | Source |
|---|---|---|
| Five trust tiers (pinned / verified / user / agent claim / untrusted) | None explicit; Copilot votes, claude-mem member attribution | [GitHub docs](https://docs.github.com/en/copilot/concepts/agents/copilot-memory); [claude-mem README](https://github.com/thedotmack/claude-mem) |
| Verification against file/symbol hashes | Copilot citation re-check (LLM judgment, "not a guarantee") | [GitHub docs](https://docs.github.com/en/copilot/concepts/agents/copilot-memory) |
| Automatic file locks across agents | None found | — |
| No LLM call unless configured (zero write-path cost) | engram (FTS only), Claude Code (uses the session model) | [engram README](https://cdn.jsdelivr.net/gh/Gentleman-Programming/engram@main/README.md) |
| Encrypted git-vault sync, local-first | engram Git Sync (unencrypted chunks per README), claude-mem cloud sync (hosted) | READMEs above |
| Backup/restore built in | None documents it | — |
| Single binary, hooks for Claude Code/Codex/Antigravity | engram (Go binary), Mem0 plugin v0.2.11 covers Antigravity too | [Mem0 changelog](https://docs.mem0.ai/changelog/highlights) |
| Still-maintained local MCP memory (OpenMemory sunset, Cursor Memories removed, Zep CE deprecated) | — | [chatforest](https://chatforest.com/reviews/mem0-mcp-server/); [memoryplugin](https://blog.memoryplugin.com/cursor-memory/) |

### Inferences
- Highest-leverage additions for AutoMemory, in order of effort: (1) `superseded_by` + valid_from/valid_to fields and a deterministic nightly consolidation report; (2) last-used counters feeding a decay rank; (3) run LongMemEval-S/MemoryBench and publish; (4) Markdown export + importers for Claude Code auto memory and claude-mem SQLite; (5) a minimal read-only viewer.

### Gaps
- Whether any competitor has file locking was not confirmable by absence; none of the READMEs or docs read mention it.
