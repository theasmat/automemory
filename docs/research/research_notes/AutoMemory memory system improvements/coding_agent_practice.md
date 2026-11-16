# Memory and context for AI coding agents: practice, vendor features, failure modes (2026)

Research date: 2026-11-12. "Measured" = numbers from a study or vendor telemetry; "opinion" = practitioner or blog claim. codex.danielvaughan.com is a prolific single-author secondary blog; it is cited only where it summarises primary work or gives concrete config, and flagged.

## Q1. What the vendors ship for memory and project context, and how it is injected

### Takeaway
Every major agent now has two layers: a human-written instruction file loaded at session start (CLAUDE.md / AGENTS.md / .cursor/rules / .github/copilot-instructions.md / GEMINI.md) and an agent-written memory layer (Claude auto memory, Codex memories, Copilot Memory, Antigravity Knowledge Items, Windsurf Memories). Vendors converge on: instruction files = always-on and authoritative; agent-written memory = injected at start but subordinate to the instruction file; skills = metadata always on, body on demand. Cursor is the outlier: it removed Memories (Nov 2025) and kept only Rules.

### Cited Findings

**Claude Code (official docs, read 2026-11-12)**
- Two systems, "Both are loaded at the start of every conversation. Claude treats them as context, not enforced configuration. To block an action regardless of what Claude decides, use a PreToolUse hook instead." — [Claude Code memory docs](https://code.claude.com/docs/en/memory)
- CLAUDE.md loaded every session; auto memory loaded every session but only the "first 200 lines or 25KB". Scope of auto memory: "Per repository, shared across worktrees". — [Claude Code memory docs](https://code.claude.com/docs/en/memory)
- Size guidance: "target under 200 lines per CLAUDE.md file. Longer files consume more context and reduce adherence." Imports (`@path/to/import`) "don't reduce its context cost". Contradictions: "Claude may pick one arbitrarily." `/doctor prompt-audit` audits CLAUDE.md, CLAUDE.local.md, AGENTS.md, rules, skills, commands, subagents. — [Claude Code memory docs](https://code.claude.com/docs/en/memory)
- `.claude/rules/*.md`, discovered recursively; rules with a `paths:` frontmatter field load only when Claude touches matching files; `~/.claude/rules/` applies to every project; symlinked rule dirs supported (approval dialog once per project). — [Claude Code memory docs](https://code.claude.com/docs/en/memory)
- Subdirectory CLAUDE.md files load lazily, "once Claude reads, writes, or edits another file in that subdirectory". — [Claude Code memory docs](https://code.claude.com/docs/en/memory)
- AGENTS.md read natively from v2.1.277 when no CLAUDE.md/CLAUDE.local.md exists in cwd or above; setting `claude-md-and-agents-md` loads both; `claude-md-or-agents-md` is default; implemented as built-in plugin `cc-plugin-agents-md@builtin`. Docs explicitly say to remove "A SessionStart hook that prints AGENTS.md" because it "adds a second copy to the context". — [Claude Code memory docs](https://code.claude.com/docs/en/memory)
- Auto memory saves four typed notes (frontmatter `type`): `user` (role, preferences), `feedback` (corrections and confirmed approaches), `project` (ongoing work, deadlines, decisions "that Claude can't derive from the code or git history"), `reference` (where to find external info). "Claude skips anything it can derive from the codebase, such as architecture, file paths, or debugging fixes. It also skips anything your CLAUDE.md files already say." — [Claude Code memory docs](https://code.claude.com/docs/en/memory)
- Storage: `~/.claude/projects/<project>/memory/` (project derived from git repo, so worktrees share it); `autoMemoryDirectory` setting relocates it; disable with `autoMemoryEnabled: false` or `CLAUDE_CODE_DISABLE_AUTO_MEMORY=1`; off by default in self-hosted environments; `/memory` toggles. — [Claude Code memory docs](https://code.claude.com/docs/en/memory)
- `/init` with `CLAUDE_CODE_NEW_INIT=1` proposes CLAUDE.md, skills and hooks and imports `AGENTS.md`, `.devin/rules/`, `.windsurf/rules/`, `.clinerules`; `/import` copies another agent's config (instruction files, MCP servers, commands, subagents, skills). — [Claude Code memory docs](https://code.claude.com/docs/en/memory)
- Hook-based injection: SessionStart hook JSON `hookSpecificOutput.additionalContext` is injected as hidden context "Claude sees it, user does not" (third-party docs; claude-mem notes silent injection since Claude Code 2.1.0). — [claude-mem hooks architecture](https://docs.claude-mem.ai/architecture/hooks); [mememory bootstrap guide](https://scott-walker.github.io/mememory/guide/bootstrap.html)
- Security change: Claude Code v2.1.50 "removes user memories from the system prompt" after a memory-poisoning PoC (see Q7). — [Cisco blog, 2026-04-01](https://blogs.cisco.com/ai/identifying-and-remediating-a-persistent-memory-compromise-in-claude-code)

**OpenAI Codex (official docs, read 2026-11-12; page redirected from developers.openai.com/codex/memories to learn.chatgpt.com)**
- Memories off by default; enabled with `[features] memories = true`; keys under `[memories]`: `generate_memories`, `use_memories`, `disable_on_external_context` (excludes chats that used MCP, web search or tool search; old name `no_memories_if_mcp_or_web_search`), `min_rate_limit_remaining_percent`, `extract_model`, `consolidation_model`, `memories.version`, `memories.dual_write`. Stored in `$CODEX_HOME/memories_v2/` (or legacy `memories/`). Generated in the background from chats idle long enough; "redacts secrets from generated memory fields" but still advises not storing secrets. `/memories` controls the current chat; desktop Settings > Personalization > "Delete Codex memories". "Required team guidance should go in AGENTS.md or checked-in documentation"; memories are "a helpful recall layer". — [Codex memories docs](https://learn.chatgpt.com/docs/customization/memories?surface=app)
- Injection order (secondary): global `~/.codex/AGENTS.md`, then directory-chain AGENTS.md, then memories, then the prompt; memories injected into the system prompt before the first user message; extra keys reported `min_rollout_idle_hours`, `max_rollout_age_days`, `max_rollouts_per_startup`; `codex debug clear-memories`. — [Vaughan 2026-06-12 (secondary)](https://codex.danielvaughan.com/2026/06/12/codex-cli-memory-over-personalisation-sycophancy-risk-configuration-defence/)
- AGENTS.md combined budget 32 KiB loaded every turn (secondary, citing a blog). — [Vaughan 2026-07-27](https://codex.danielvaughan.com/2026/07/27/do-auto-generated-agents-md-files-actually-help-codex-cli/)
- AGENTS.md reported as an open standard under the Linux Foundation's Agentic AI Foundation since Feb 2026 (secondary, unverified). — [mem0 blog](https://mem0.ai/blog/how-memory-works-in-codex-cli)

**Cursor (official docs)**
- Rules: `.cursor/rules/*.mdc` (plain `.md` ignored, needs frontmatter); modes "Always Apply", "Apply to Specific Files" (`globs`), "Apply Intelligently" (`description`), "Apply Manually" (@-mention); "Keep rules under 500 lines"; AGENTS.md in root or subdirectories, nested files combine with more specific winning; Team rules from dashboard, precedence Team → Project → User, earlier wins on conflict; rule content "included at the start of the model context"; `@file` references are not inlined, the agent reads them with tools. — [Cursor rules docs](https://cursor.com/docs/context/rules)
- Memories feature removed from 2.1.17 (Nov 2025); staff advised exporting memories into Rules; some users still saw "ghost" memories injected afterwards. — [Cursor forum, staff reply](https://forum.cursor.com/t/memories-not-showing/143820); [Cursor forum, ghost rules](https://forum.cursor.com/t/ghost-user-rules-still-injected-in-agent-context-empty-in-settings-user-tab-not-in-aicontext-personalcontext-logout-login-did-not-fix/164709)
- Cursor 2.4 (2026-01-22) added Skills support; directory reported as `.cursor/skills/` or `.agents/skills/` (sources disagree). — [mdskills.ai list](https://www.mdskills.ai/learn/ai-agents-that-support-skill-md-2026)

**GitHub Copilot**
- Copilot Memory public preview 2026-03-04 (on by default for Pro/Pro+), user preferences added 2026-05-15; stored on GitHub's servers; repo-level facts carry citations to code and are re-checked against the current branch before use; unused entries expire after 28 days; owners review/delete under Settings > Copilot > Memory; code review uses repo facts only. GitHub documents no precedence between memories and instruction files; instruction precedence is personal → path-specific → `.github/copilot-instructions.md` → `AGENTS.md` → organization. — [startdebugging.net, Sept 2026](https://startdebugging.net/2026/09/copilot-memory-vs-repository-custom-instructions-vs-agents-md/)
- Visual Studio 2026 can save memories to `%USERPROFILE%/copilot-instructions.md` or `/.github/copilot-instructions.md`. — [Microsoft DevBlogs, 2026-01-15](https://devblogs.microsoft.com/visualstudio/copilot-memories)
- Copilot reads skills from `.github/skills/`. — [mdskills.ai list](https://www.mdskills.ai/learn/ai-agents-that-support-skill-md-2026)

**Gemini CLI / Antigravity**
- `save_memory` tool appends a `fact` to `~/.gemini/GEMINI.md` under `## Gemini Added Memories`; docs advise against saving transient material. — [Gemini CLI memory tool docs](https://geminicli.com/docs/tools/memory); [Gemini CLI auto memory](https://geminicli.com/docs/cli/auto-memory/)
- Antigravity Knowledge Items: "persistent memory system"; each KI has title, summary, artifacts (auto-generated docs, code examples, "persistent memories of user instructions"); created automatically from conversations; retrieval is relevance-based: "summaries of all KIs are available to the agent", it reads a KI's artifacts when relevant; viewable in Agent Manager. Storage path and workspace scope are not documented. — [Antigravity Knowledge docs (mirror)](https://antigravity-doc.pages.dev/en/artifacts/knowledge)
- Community reports of the `knowledge/` folder ending up empty and KIs leaking into a new workspace. — [Google AI forum: Knowledge feature failure](https://discuss.ai.google.dev/t/knowledge-feature-failure/129021); [Google AI forum: workspace knowledge exposed in new workspace](https://discuss.ai.google.dev/t/workspace-knowledge-items-exposed-in-a-new-workspace/122315.md)
- Third-party reports say Gemini CLI was discontinued (date given as 2026-06-18) in favour of Antigravity CLI (`agy`) which reuses `~/.gemini/` and honours GEMINI.md and AGENTS.md; no official Google source found. — [toolsbase.dev migration guide](https://toolsbase.dev/en/blog/gemini-cli-to-antigravity-migration)

**Windsurf**
- Official docs describe auto-generated Memories (by Cascade) plus user Rules; memories are tied to the workspace that created them and retrieved when Cascade judges them relevant; no credit cost. docs.windsurf.com/windsurf/memories now redirects to docs.devin.ai (404 at fetch time), so details are from the search snapshot only. — [Windsurf memories docs (search snapshot)](https://docs.windsurf.com/windsurf/memories)

### Inferences
- The common denominator is three injection surfaces: (1) always-on instruction file, (2) small agent-written memory block injected at start but ranked below the instruction file, (3) on-demand skills/rules keyed by description or path glob. A shared-memory product fits best by writing a short block to (1), exposing recall through (3)/MCP, and never trying to be (2) for all tools at once.
- Claude Code's own auto memory explicitly excludes "architecture, file paths, or debugging fixes" and anything already in CLAUDE.md; a product that stores decisions, constraints and failures-with-fixes is complementary, not duplicative, as long as it checks for overlap with the instruction file.
- Copilot's citation re-check and 28-day expiry is the closest vendor analogue to "verify notes against code hashes".

### Gaps
- Official Windsurf memory docs unreachable; Antigravity storage path/scope undocumented; Gemini CLI retirement unconfirmed by Google.
- No vendor publishes measured effectiveness of its own auto memory.

## Q2. The ICLR 2026 AGENTS.md study and follow-ups; when context files help

### Takeaway
Measured: LLM-generated context files give no success gain and cost ~20% more; developer-written files give a small, non-significant gain (+2.4%, p=0.21) with similar cost. The cost comes from agents obeying the file (more tests, more tool runs, more reads). Context files help most when the repo has no other docs, and when they contain only what the agent cannot infer.

### Cited Findings
- Gloaguen, Mündler, Müller, Raychev, Vechev (ETH Zurich / LogicStar.ai), arXiv 2602.11988, listed at ICLR 2026 virtual site. Agents: Claude Code (Sonnet 4.5), Codex (GPT-5.2, GPT-5.1 mini), Qwen Code (Qwen3-30b-coder). Benchmarks: SWE-bench Lite (300 tasks) and CTXbench (138 issues, 12 repos with developer-committed context files). — [arXiv 2602.11988v3](https://arxiv.org/html/2602.11988v3); [ICLR listing](https://iclr.cc/virtual/2026/10021235)
- LLM-generated files: resolution −0.5% (SWE-bench, p=0.87) and −2% (CTXbench, p=0.37); cost +20% / +23%; "increase the # steps in every setting". Developer-committed files: +2.4% average (p=0.21), ~+3.34 steps, cost up to +19%. — [arXiv 2602.11988v3](https://arxiv.org/html/2602.11988v3)
- Mechanism: agents follow the file. `uv` used ~1.6x per instance when mentioned vs <0.01 otherwise; repo-specific tools 2.5 vs <0.05; reasoning tokens +22% (GPT-5.2). Overviews "did not reduce the steps needed to reach the relevant files". — [arXiv 2602.11988v3](https://arxiv.org/html/2602.11988v3)
- Developer files helped every agent except Claude Code. With all repo documentation removed, LLM-generated files improved performance by 2.7% and beat developer-written ones; the authors suggest this explains anecdotal gains on poorly documented repos. — [arXiv 2602.11988v3](https://arxiv.org/html/2602.11988v3)
- Recommendation: omit LLM-generated files; human files "should only include instructions required for coding agents that are not already present in the README"; "Any attempts to improve performance should be rigorously evaluated before deployment." — [arXiv 2602.11988v3](https://arxiv.org/html/2602.11988v3)
- Content of developer files: 8/12 had a codebase overview, avg 641 words, 9.7 sections. — [arXiv 2602.11988v3](https://arxiv.org/html/2602.11988v3)
- Follow-up: Khatri, arXiv 2607.27250 (2026-07-28), Claude Code and Codex, 17 tasks from 3 repos, 288 runs. Context strategy did not change correctness; equivalence bound ~10–15 pp; failures were "implementation skill", not missing repo knowledge; the real AGENTS.md "never turned a near-miss into a pass". Borderline difficulty is agent-specific (Spearman ρ=0.75). — [arXiv 2607.27250](https://arxiv.org/abs/2607.27250)
- Earlier efficiency result: Lulla et al., arXiv 2601.20404 (Jan 2026), 10 repos, 124 PRs, Codex, developer-written AGENTS.md: median wall-clock 98.57s → 70.34s (−28.6%), median output tokens 2,925 → 2,440 (−16.6%); success not measured. — [Vaughan 2026-07-27 summarising](https://codex.danielvaughan.com/2026/07/27/do-auto-generated-agents-md-files-actually-help-codex-cli/)
- Harness-engineering survey of 2,853 GitHub repos: "Context Files dominate the configuration landscape", often the only mechanism; few repos adopt skills or subagents; skills mostly static text. — [arXiv 2602.14690](https://arxiv.org/abs/2602.14690)
- Configuration smells (dos Santos et al., arXiv 2606.15828, 100 repos, 61 CLAUDE.md + 39 AGENTS.md): 91 repos had ≥1 smell, 207 instances. Lint Leakage 62%, Context Bloat 42% (≥200 lines; worst 1,477 lines/27 sections), Skill Leakage 35%, Init Fossilization 24% (file never edited after /init), Conflicting Instructions 16% confirmed, Blind References 14% ("If you just mention the path... Claude will often ignore it"). — [arXiv 2606.15828](https://arxiv.org/html/2606.15828)
- Compliance decay (opinion, cited data): 95%+ compliance at messages 1–2, 60–80% at 3–5, 20–60% at 6–10 (Siddhant Khare); HumanLayer: models follow ~150–200 instructions reliably and Claude Code's system prompt already uses ~50. — [Wiegold 2026-03-09](https://thomas-wiegold.com/blog/claude-md-helpful-or-expensive-noise/)

### Inferences
- "Context file" in these studies means overview-heavy files. None tested a short file of only non-inferable facts (gotchas, decisions, failing-test fixes), so the measured null is about the typical file, not about that content class.
- The Lulla efficiency gain and the Gloaguen cost increase are not contradictory: a file that names the right test command can save steps, while a file that asks for broader checks adds steps. The content decides the sign.

### Gaps
- No study isolates gotcha/decision-style content; no measured results for agent-written memory layers' effect on task success.
- Venue of 2602.11988 (workshop vs main track) not confirmed from the arXiv page.

## Q3. What practitioners store that helps, and what creates noise

### Takeaway
Consensus across studies and practitioners: store what the agent cannot infer (non-obvious build/test commands with flags, architecture decisions that contradict the layout, conventions against common patterns, known traps, pointers with a "read when" sentence). Noise: directory trees, overviews, style rules a linter enforces, personality prompts, unreviewed `/init` output, verbose observation logs.

### Cited Findings
- Helped (opinion, with the ETH data behind it): "Non-obvious test or build commands with exact flags", "Architectural decisions that differ from what the code layout suggests", "Team conventions that go against common patterns", "Pointers to deeper docs", tool choices like `bun` not `npm`. Hurt: personality prompts, "write clean code", full style guides, directory trees and overviews, unreviewed `/init`, ~300-line files. Recommendations: under ~80 lines, "Grow it through failures", move non-negotiables into hooks. — [Wiegold 2026-03-09](https://thomas-wiegold.com/blog/claude-md-helpful-or-expensive-noise/)
- Include: conventions that contradict what the agent would infer (mixed naming across layers), deployment constraints and approval workflows, architecture decisions overriding defaults ("avoiding ORMs"), known traps ("directories with incompatible build systems"). Exclude: repo structure summaries, build/test commands already in config files, linter rules, generic advice. Target under 100 lines; "Never ship raw /init output". — [Vaughan 2026-07-27](https://codex.danielvaughan.com/2026/07/27/do-auto-generated-agents-md-files-actually-help-codex-cli/)
- Smells paper remediation: move product docs to README, move task-specific steps into skill files, rely on linters/hooks for style ("Spending instruction budget on style rules is dead weight"), explain what each referenced file contains and when to read it, review on repeated mistakes. Example fix: a file cut from 598 to 149 lines. — [arXiv 2606.15828](https://arxiv.org/html/2606.15828)
- Practitioner wiki case (2026-04-15): two or three short operating rules ("Research the codebase before editing. Never change code you haven't read.", "Verify work actually works before claiming done") and failure-specific notes ("don't flip-flop on workflow advice inside one thread") worked; "Specific reproductions of failures ... were more useful than general complaints"; risk that the wiki becomes a "swamp" without freshness tracking; "Any text re-injected into context at session start works as a control lever, whatever the filename." — [dev.to UB3DQY](https://dev.to/ub3dqy/my-wiki-stopped-being-memory-and-quietly-became-a-behavior-patch-for-ai-agents-178h)
- Claude Code docs: "If the instruction is something that must run at a specific point ... write it as a hook instead." — [Claude Code memory docs](https://code.claude.com/docs/en/memory)
- Noise from observation logs (vendor's own docs): claude-mem originally loaded 50 observations up front (~8,500 tokens); moved to an index (~800 tokens) plus 2–3 fetched on demand, ~87% context reduction; progressive disclosure layers of 50–100 tokens per index row vs 500–1,000 per full observation; `CLAUDE_MEM_CONTEXT_OBSERVATIONS` tunes count. — [claude-mem architecture evolution](https://docs.claude-mem.ai/architecture-evolution)
- claude-mem-lite fork claims the original's LLM "WHEN TO SKIP" prompt filter is unreliable and replaces it with deterministic code-level filtering plus compression of old low-value observations (third-party claim). — [claude-mem-lite](https://awesome.ecosyste.ms/projects/github.com%2Fsdsrss%2Fclaude-mem-lite)
- Memory of wrong fixes / over-personalisation examples (secondary, citing OP-Bench Jan 2026: 36 configs, 6 LLMs, 6 memory methods, relative drops 26.2–61.1% vs memory-free; "More sophisticated memory systems showed stronger effects"): stored "prefer microservices" applied to a small monolith team; Jest preference from a React project carried into a Go repo; stored "tabs" preference overriding `.editorconfig`. — [Vaughan 2026-06-12](https://codex.danielvaughan.com/2026/06/12/codex-cli-memory-over-personalisation-sycophancy-risk-configuration-defence/)

### Inferences
- The categories AutoMemory stores (decisions, constraints, gotchas, failures-with-fixes) match the "include" lists almost exactly; the risk is volume and staleness, not category.
- Store the *why* and the *when* with each preference; the over-personalisation examples all lack scope (project, date, reason).

### Gaps
- No controlled measurement of "failures-with-fixes" notes specifically; evidence is practitioner and vendor-doc level.

## Q4. Keeping agent docs (plans, PRDs, prompts, skills) out of git but versioned

### Takeaway
No settled standard. Patterns seen: a gitignored agent folder (`docs/.claude/`, `.agentic/plans/`, `.ai/plans/`) sometimes via `.git/info/exclude` rather than the tracked `.gitignore`; status-based promotion (live PRDs in `docs/prd/`, closed ones moved to a private vault); OpenViking as a separate "context database" with a `viking://` filesystem and L0/L1/L2 tiers.

### Cited Findings
- `agent-artifacts` skill routes plans, tasks and research to `docs/.claude/`, gitignored. — [agent-artifacts SKILL.md](https://claudskills.com/skills/agent-artifacts/SKILL.md)
- A dotfiles plan skill writes to `.ai/plans/` and adds the path to `.git/info/exclude`, not `.gitignore`, because "local plans are agent working state, not repository policy". — [skillsmp dotfiles plan skill](https://skillsmp.com/creators/justestif/dotfiles/home-agents-skills-plan)
- CashPilot PR 241: removed agent working docs from `docs/`, gitignored them and excluded them from MkDocs because MkDocs builds every file in `docs_dir`. — [GitHub PR](https://github.com/GeiserX/CashPilot/pull/241)
- Indexed PRD strategy: open-epic PRDs in `docs/prd/`, moved to a private vault when the epic closes. — [understandingdata.com](https://understandingdata.com/posts/indexed-prd-design-docs/)
- "Plan hygiene — extract, then remove": plans are extracted into durable docs, then deleted. — [skill_steward docs](https://docs.page/arenukvern/skill_steward/start_here/executable-plans)
- Claude Code issue: `.gitignore` should not double as AI access control; proposes `.claudeignore`. — [claudeissues 65812](https://claudeissues.com/issue/65812-design-gitignore-should-not-double-as-ai-access-control-introduce-clear-claudeig)
- OpenViking (Volcengine/ByteDance): "one filesystem for everything an agent knows: knowledge, memory, and skills"; `viking://resources/...` and `viking://user/{id}/memories/...`; L0 `.abstract.md` (one sentence), L1 `.overview.md`, L2 full data; session commit extracts memories as editable Markdown; integrations: Claude Code (hooks + MCP), Codex, Cursor, OpenCode, Hermes, LangChain; AGPLv3 (CLI Apache-2.0). LoCoMo user-memory benchmark (v0.3.22, Doubao models): Claude Code native 57.21% → 80.32% with OpenViking; input tokens −34.3% to −91.0%; latency −58% to −66%. — [OpenViking README](https://cdn.jsdelivr.net/gh/volcengine/OpenViking@main/README.md)
- Claude Code: `CLAUDE.local.md` is gitignored personal instructions; to share across worktrees import a home-directory file; `.claude/rules/` supports symlinks to a shared rules repo. — [Claude Code memory docs](https://code.claude.com/docs/en/memory)

### Inferences
- The "library outside git with tiered loading" design matches OpenViking's L0/L1/L2 and claude-mem's index-then-fetch independently arrived at by two vendors; the pattern is validated by vendor telemetry only (self-reported).
- OpenViking's AGPL licence makes it unusable as a dependency for a permissive-licensed product; it is a design reference only.

### Gaps
- No evidence on Obsidian/Notion as agent doc stores beyond forum posts; no measured comparison of in-git vs out-of-git agent docs.

## Q5. Pull (agent asks) vs push (inject at session start)

### Takeaway
Measured-ish (vendor/project telemetry): a small pushed index (~800–1,500 tokens) plus on-demand fetch beats both a large push (5k–20k tokens) and pure pull. Vendors push a bounded block (Claude: 200 lines/25KB; Codex: memories in system prompt; Copilot: cited facts) and pull the rest. Writes are better done by hooks than by asking the agent to call a store tool.

### Cited Findings
- Claude Code loads auto memory "first 200 lines or 25KB" every session; `MEMORY.md` is an index with topic files. — [Claude Code memory docs](https://code.claude.com/docs/en/memory)
- claude-mem: 50 observations pushed = ~8,500 tokens; index push 800 tokens + 2–3 fetched = ~87% less. — [claude-mem architecture evolution](https://docs.claude-mem.ai/architecture-evolution)
- mcp-memory-service moved session-start from MCP tool calls to direct code execution: 3,600 → 900 tokens for an 8-memory session start (−75%). — [mcp-memory-service Phase 2 docs](https://glama.ai/mcp/servers/@doobidoo/mcp-memory-service/blob/0a9494ce57e3cd46d70176bc5527b2f566ea64b4/docs/hooks/phase2-code-execution-migration.md)
- Event-sourced memory blog: session-start context 800–1,500 tokens in MCP mode vs 5,000–20,000 for stateless reconstruction; hook-driven automatic writes at "zero token cost" outperform 20–50 tokens per voluntary MCP store call. — [Vaughan 2026-07-03](https://codex.danielvaughan.com/2026/07/03/event-sourced-memory-layers-coding-agents-projectmem-esaa-codex-cli-pretooluse-governance/)
- Agent Skills spec: discovery loads only name+description at startup; full SKILL.md on activation; bundled files on execution. — [agentskills.io](https://agentskills.io/)
- Cursor: `@file` references in rules are not inlined; "anything that must always be in context belongs in the rule body". — [Cursor rules docs](https://cursor.com/docs/context/rules)
- Smells paper: a bare path reference is a "Blind Reference"; agents often ignore it unless told what it contains and when to read it. — [arXiv 2606.15828](https://arxiv.org/html/2606.15828)
- Over-personalisation blog: system-prompt placement of memories "gives memories high apparent authority" (a push downside). — [Vaughan 2026-06-12](https://codex.danielvaughan.com/2026/06/12/codex-cli-memory-over-personalisation-sycophancy-risk-configuration-defence/)
- Codex memories docs: `disable_on_external_context` keeps MCP/web-search sessions out of memory generation (pull tools can poison writes). — [Codex memories docs](https://learn.chatgpt.com/docs/customization/memories?surface=app)

### Inferences
- The effective pattern is "push the index, pull the body": a short managed block naming what exists and when to read it (not bare paths), with retrieval by MCP/skill. Pure pull fails because agents don't know to ask; pure push fails on cost and authority.
- Writes should be hook-triggered or reviewed; agent-initiated store calls are both token-costly and the main poisoning vector.

### Gaps
- No independent benchmark comparing push vs pull on coding task success; all numbers are token counts from vendors/projects.

## Q6. Agent Skills (SKILL.md) and cross-tool formats

### Takeaway
SKILL.md (Anthropic, released as open standard Dec 2025) is read by Claude Code, Codex (`.agents/skills/`), Copilot (`.github/skills/`), Gemini CLI (`.gemini/skills/`), Cursor 2.4+ and others; counts cited range 26–40+ tools. A shared skill tree is possible but directory names differ per tool and portability is imperfect; in the wild, few repos use skills at all.

### Cited Findings
- Format: folder with `SKILL.md` (frontmatter `name`, `description` minimum) plus optional `scripts/`, `references/`, `assets/`; three-stage progressive disclosure; "originally developed by Anthropic, released as an open standard"; GitHub org agentskills/agentskills. — [agentskills.io](https://agentskills.io/)
- Adoption counts: 26+, 30+ (Apr 2026), 32+ (Jun 2026), 40+ on the agentskills.io showcase (mid-2026); the clients page itself rendered empty on fetch. — [noqta.tn](https://www.noqta.tn/en/news/skill-md-open-standard-30-ai-coding-tools-adoption-2026); [mdskills.ai](https://www.mdskills.ai/learn/ai-agents-that-support-skill-md-2026); [agentskills.io clients](https://agentskills.io/clients.md)
- Paths: Codex `.agents/skills/`; Copilot `.github/skills/`; Gemini CLI `.gemini/skills/`; Cursor `.cursor/skills/` or `.agents/skills/` (sources disagree); Cursor Skills since 2.4 (2026-01-22). — [mdskills.ai](https://www.mdskills.ai/learn/ai-agents-that-support-skill-md-2026); [mcp.directory](https://mcp.directory/blog/cross-agent-skills-cursor-codex-cline-antigravity-gemini-mastra-portability)
- A "compatibility stress-test" reportedly showed Cursor failing to fully load skill assets that ran in Claude Code, Codex and Gemini CLI (secondary). — [mcp.directory](https://mcp.directory/blog/cross-agent-skills-cursor-codex-cline-antigravity-gemini-mastra-portability)
- Of 2,853 repos, few adopt skills or subagents; skills "mostly rely on static instructions rather than executable scripts". — [arXiv 2602.14690](https://arxiv.org/abs/2602.14690)
- Claude Code `/import` carries another agent's skills over; `.claude/rules/` symlinks allow one shared rules tree across projects. — [Claude Code memory docs](https://code.claude.com/docs/en/memory)
- Smells paper: "Skill Leakage" (35%) is task-specific content left in the main file instead of skill files; Google's adk-python moved its style guide to a skill file. — [arXiv 2606.15828](https://arxiv.org/html/2606.15828)

### Inferences
- A shared skill tree works today by keeping one canonical folder and symlinking/copying it to each tool's path; adoption data says most teams have not done this yet.

### Gaps
- agentskills.io client list did not render; exact per-tool list and Cursor's canonical path unverified from primary sources.

## Q7. Failure modes

### Takeaway
Documented: stale instruction files beat the code 90% of the time; stale MEMORY.md served mid-task; memory poisoning via hooks (Cisco PoC, fixed in Claude Code 2.1.50; OWASP ASI06); cross-project leakage from global consolidation; over-personalisation measured at 26–61% relative drops; secrets passing through agent runtime; ghost memories after feature removal.

### Cited Findings
- Stale instruction files: Xu & Wu, arXiv 2610.30571 (2026-10-22), 40 Python projects, Claude Code + Codex, 320 runs. Without mitigation 90.1% of convention choices followed the stale file and 4.2% the code; Claude Code noticed the conflict in 79/80 runs and still followed the file; Codex rewrote existing code in 20/80 runs, breaking tests in 17. One sentence in the user-level instruction file giving existing code precedence raised code-following to 91.4%, stopped rewrites, test pass rate 89.4% → 100%. Explanation: the file arrives "as an instruction", code arrives as tool output. — [arXiv 2610.30571](https://arxiv.org/abs/2610.30571)
- Stale memory: Claude Code 2.1.139–2.1.141 reported surfacing MEMORY.md content mid-task without verifying against current code. — [aiweekly alert](https://aiweekly.co/alerts/claude-code-serves-stale-memorymd-data-mid-task)
- Memory poisoning (Cisco, 2026-04-01): npm `postinstall` → `UserPromptSubmit` hook in `~/.claude/settings.json` → overwrote `MEMORY.md` across projects, appended an rc alias re-enabling auto memory; poisoned agent recommended hardcoding an API key in committed source and discouraged `.env`, "framed ... as mandatory project constraints". Anthropic mitigation in v2.1.50 removes user memories from the system prompt. — [Cisco blog](https://blogs.cisco.com/ai/identifying-and-remediating-a-persistent-memory-compromise-in-claude-code)
- OWASP added memory and context poisoning to its 2026 Agentic Top 10 (ASI06). — [pulse.bot summary](https://www.pulse.bot/cybersecurity/news/bad-memories-still-haunt-ai-agents-e1ea06e5-7397-4880-98e9-960e28e4d92c/)
- Cross-project leakage and conflicting-with-config memories (Jest→Go, tabs vs `.editorconfig`); "Consolidation merges memories globally, with no per-project scoping by default" (Codex). Defences: `max_rollout_age_days`, `disable_on_external_context`, explicit AGENTS.md, don't stack a third-party memory MCP on native memories. — [Vaughan 2026-06-12](https://codex.danielvaughan.com/2026/06/12/codex-cli-memory-over-personalisation-sycophancy-risk-configuration-defence/)
- Antigravity: knowledge items from one workspace exposed in a new workspace (user report). — [Google AI forum](https://discuss.ai.google.dev/t/workspace-knowledge-items-exposed-in-a-new-workspace/122315.md)
- Cursor ghost memories still injected after the feature's removal. — [Cursor forum](https://forum.cursor.com/t/ghost-user-rules-still-injected-in-agent-context-empty-in-settings-user-tab-not-in-aicontext-personalcontext-logout-login-did-not-fix/164709)
- Secrets: Claude Code, Cursor and Codex "move secrets through runtime memory, bypassing the artifact layer"; a user with a memory rule against printing secrets still saw them exposed and had to gitignore the file and persist an explicit rule. Codex redacts secrets from generated memories but tells users to review before sharing `~/.codex`. — [knostic.ai](https://www.knostic.ai/blog/ai-coding-assistants-leaking-secrets); [claudeissues 59094](https://claudeissues.com/issue/59094-secret-leak-agents-read-env-credential-files-without-redaction-remediation-pushe); [Codex memories docs](https://learn.chatgpt.com/docs/customization/memories?surface=app)
- Init fossilization: 24% of 100 repos never edited the generated file; conflicting instructions in 16%. — [arXiv 2606.15828](https://arxiv.org/html/2606.15828)
- Copilot's defence against stale facts: citations re-checked against the current branch, 28-day expiry for unused facts. — [startdebugging.net](https://startdebugging.net/2026/09/copilot-memory-vs-repository-custom-instructions-vs-agents-md/)
- Context duplication: SessionStart hook printing AGENTS.md adds "a second copy". — [Claude Code memory docs](https://code.claude.com/docs/en/memory)

### Inferences
- Verifying notes against code hashes directly targets the two best-measured failures (stale file 90.1%, stale MEMORY.md) and mirrors Copilot's citation re-check; a note whose anchor hash no longer matches should be demoted, not injected.
- The managed block in AGENTS.md/CLAUDE.md should carry the one-sentence precedence rule from Xu & Wu ("existing code takes precedence over these notes when they conflict") given its measured 4.2% → 91.4% effect.
- Agent-written memory needs: per-project scoping by default, a reason/date on every preference, a size cap, and write paths that do not run from hooks an installer can modify.

### Gaps
- No measured incidence of "memory of a wrong fix" beyond OP-Bench's general over-personalisation numbers; no public incident of cross-project leakage with a quantified cost.
- Claude Code's post-2.1.50 injection location for auto memory (if not the system prompt) is not described in the docs fetched.
