# Consolidation audit: what overlaps, what is stale

Oct 2026. Read-only audit of the code, the 68 MCP tools, ~25 CLI commands, 25 dashboard pages and the tables, with row counts from the maintainer's real database (one user, so
"empty here" means "unproven", not "useless"). Nothing was changed. Effort: S under a day, M a few days, L a week or more. Risk is what could break for existing users/agents.

## A. Duplicated features (same job, built twice or more)

| # | Overlap | Evidence | Merge into | Effort / risk |
|---|---|---|---|---|
| A1 | **Two document systems**: tracked documents (scanner, `tracked_documents` 88 rows, `document_revisions`, `DocCompiler` L0/L1, `doc.list/get`, `automemory doc`, Documents page) and the Project library (tiers, revisions, FTS, `library.*`, `automemory library`, Library page, linked files with patterns) | Library "linked files" already do what the scanner does: patterns, in-place files, revisions, L0/L1 | One store: the scanner's default patterns (`PRD.md`, `docs/**`, `adr/**`...) become a built-in set of linked patterns; `document_revisions` into `library_revisions`; one tier function; one page; `doc.*` tools kept one release as aliases | L / medium (88 rows to migrate, docs page, doc sync) |
| A2 | **Four hand-off surfaces**: baton (`batons` 12 rows, `baton.pass/pickup`, CLI `baton/pass/pickup`), `memory.resume`, CLI `handoff` and `resume`, session-checkpoint memories, and the continuity brief | `build_handoff` writes a baton; `build_resume` already assembles baton + checkpoint + decisions + brief + library index in one place: the duplication is the surface (tools and commands), not the engine | `continuity.brief` (write, with a `handoff` flag for the ledger packet) and `memory.resume` (read). Drop `baton.pass/pickup` after one release; CLI `pass/pickup/baton` become hidden aliases | M / low |
| A3 | **Three sync systems**: (1) git-vault deltas, `sync auto` (the real one, now carrying memories, traces, decisions, brief, library); (2) cloud `sync init/status/push/pull` with blind wire formats and `/api/cloud/*` "test endpoint" routes (the unbuilt SaaS); (3) legacy text vault `sync export/import/restore` | `sync.rs` is 1,414 lines; `export_to_vault` is still called by `ports.rs`; there is also a separate `backup` module and command | Keep (1). Move (2) behind a feature flag or into `workers/` until the SaaS exists. Fold (3) into `backup`/`restore` (same job: disaster recovery) and delete `sync export/import` | M / medium (restore must keep working for old vaults) |
| A4 | **Seven read tools for "what do we know"**: `memory.recall`, `ask`, `about`, `context`, `search`, `list`, `topics`, `digest`, `constitution`, `skills`, plus `chat.search` and `library.search` | `recall` and `ask` are literally one match arm already; `chat.search`, `library.search`, `memory.search` each search one store | One `memory.search {query, scope: notes|chat|library|all}`; `memory.list {view: all|topics|skills|constitution|digest}`; keep `recall` (ranked context for a task) and `about` (one target). -6 tools | M / low (aliases for a release; update rules text) |
| A5 | **Proposal handling written twice**: `decide_proposal` has an if-chain per type (`llm_candidate`, `team_pin`, `library_edit`, `duplicate_merge`), `decide_all_proposals` has its own SQL that only knows `duplicate_merge` | Found while building L5: approve-all marks `llm_candidate` and `team_pin` approved without applying them (real bug); I had to special-case `library_edit` | A `ProposalKind` trait (`on_approve`, `on_reject`) used by both paths; approve-all calls it per row | S-M / low, **fixes a bug** |
| A6 | **Two profile systems with copied code**: embeddings profiles (`/api/recall/*`, `recall.vue` 361 lines) and LLM profiles (`/api/llm/*`, `tools.vue`): key storage, test, profile CRUD, "cloud sends text" consent | Same fields (`model`, `endpoint`, `api_key_env`, `allow_network`), same routes twice (`/profile`, `/profile/:name`, `/key`, `/test`) | One "Providers" component and route set with `kind: embeddings\|llm` | M / low |
| A7 | **Four places for the same instructions to agents**: `rules text` in `connect.rs` (static, ~100 lines), tool descriptions, `export-rules` files, `.automemory/library/INDEX.md`, plus the resume pack | The rules text repeats what tool descriptions say and has to be edited by hand each time a tool is added (I did this for continuity, library, put/propose) | Generate the rules section from the tool registry (name + one-line purpose + "when to call"), so a new tool cannot be forgotten or drift | M / low |
| A8 | **Five pages for one question ("what did agents do")**: `activity` (48 lines), `traces` (70), `replay` (427), `skills` (89), `failures` (204), plus Coordination | Skills table holds 8,702 rows with ~11 distinct names; traces 216 rows with ~37 titles (the X1 defect, still growing) | One Activity page with tabs (Timeline, Replay, Problems, Skills) after fixing the write path | M / low |
| A9 | **Reviews vs rooms**: `review.submit/next/post` (`reviews` table, 0 rows) is cross-review; planning rooms do propose / critique / revise with a human verdict | A review is a one-round room | Express reviews as a room type, drop the three review tools and the table (after confirming nobody uses them) | M / medium |

## B. Code-level duplication (same logic copied)
| # | What | Where | Fix |
|---|---|---|---|
| B1 | Five transcript readers each re-implement the byte-offset tail (`rposition(b'\n')`), `same_project`, a `tool_line` summariser and noise stripping | `readers/{claude_code,codex,command_code,cursor,antigravity}.rs` | `readers/util.rs`: `JsonlTail`, `same_project`, `short_tool_line`. About 150 lines fewer, and every new reader starts smaller. S |
| B2 | Three "spawn myself in the background, throttled" helpers | `hook.rs`: `spawn_background_sync`, `spawn_chat_refresh`, `spawn_library_sync` | One `spawn_self(args, cwd, key, min_seconds)`. S |
| B3 | Five `clip()` functions and several `normal()`/hash helpers | `resume.rs`, `llm.rs`, `library/tiers.rs`, `rooms.rs`, `continuity/store.rs` | One `text::clip` in core. S |
| B4 | Five 7-line redirect pages | `coordination`, `guardrails`, `memories`, `overview`, `proposals` | Delete after one release (the old links are only in old bookmarks). S |

## C. Stale or unproven (real database)
| Item | Evidence | Suggestion |
|---|---|---|
| `sessions` and `session_notes` tables | 0 rows, 0 in use | Remove, or find what was meant to write them |
| Own graph (`graph.index/query/neighbors/impact`, `graph_entities`/`graph_edges`, Graph page 294 lines) | 0 rows; `codeintel.rs` says AutoMemory builds no AST graph and delegates to `codebase-memory-mcp`, which `connect` already registers | Decide: drop our four graph tools and the page in favour of the external one (-4 tools) |
| `memory_links`, `memory_embeddings`, `memory_vectors`, `custom_guardrail_rules`, `preferences`, `leases`, `lock_alerts` | 0 rows here (features default off or not yet used by this user) | Keep, but they are the first things to test in the live trials; no deletion on one user's data |
| `memory_versions` | 245 rows for 21 memories | Normal (history), but cap per memory |
| `procedural_skills`, `success_traces`, `failure_records` | 8,702 / 216 / 2 | Fix the write path (dedupe skills by name + step hash; error signature on failures) and add `memory prune`: the largest defect, tracked as X1 |
| Six tables for locking (`leases`, `lock_coverage`, `lock_alerts`, `hook_claims`, `workspace_snapshots`, `locks` naming) | one concept, two names ("lease" in code and tables, "lock" in tools and UI) | Rename in the UI/docs only; do not migrate tables |

## D. Things that look similar but should stay separate
- **Memory vs brief vs library**: verified long-lived notes, per-handoff session state, and documents. Different lifetimes and trust. Share search (A4), not storage.
- **Chat archive vs tool events**: what was said vs what was done. Both feed the drafter; do not merge.
- **Rooms vs tasks**: rooms decide, tasks carry out; the link already exists.

## E. Order of work (cheapest and safest first)
1. **A5** (bug) + **B1-B4** (small, mechanical, covered by existing tests). About 3 days.
2. **A4** unified search and read-tool merge, **A2** baton into the brief, **A7** generated rules text. The tool count goes from 68 to about 55; every removed tool stays as a deprecated alias for one release. About a week.
3. **A6** providers, **A8** activity page after X1. About a week.
4. **A1** documents into the library, **A3** sync cleanup. The biggest and riskiest; do them last and each behind its own scenario test.
5. Decided (Oct 2026, maintainer asked for one clear plan): drop our graph tools (C), reviews become rooms (A9), remove the two empty tables (C), cloud stub behind a flag (A3). The plan is PLAN §18.

Every step keeps `tests/run_all.sh` green and updates the counts (tools, tests) in the docs, per the project rules.
