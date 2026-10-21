# AutoMemory — Features

What the product does, for whom, and how far along each piece is. How it is built: [ARCHITECTURE](ARCHITECTURE.md). Order of work: [PLAN](PLAN.md). Open work: [TASKS](TASKS.md).

## Shared instruction files and human input

| Feature | Status | Notes |
|---|---|---|
| `automemory export-rules` writes a marked block (decisions in force + user-written knowledge, ≤3 KB, redacted) into AGENTS.md, CLAUDE.md, GEMINI.md, `.cursor/rules/automemory.mdc`, `.github/copilot-instructions.md`, `.windsurf/`, `.clinerules/`, `.roo/rules/`, `.kiro/steering/` | ✅ | Only files of agents the repo already uses (AGENTS.md always); `--agent`, `--all`, `--check`. Session start refreshes files that already carry the block; `disconnect` removes only the block |
| Only verified knowledge travels | ✅ | Active decisions and memories a person wrote; agent-saved memories reach other agents through MCP recall, not these files |
| `automemory note "…"`, dashboard **+ Add memory** | ✅ | Trusted at once; dashboard add needs the human password when one is set |
| Hook adapters for Cursor, Gemini CLI, Copilot, Cline, Kiro, OpenCode, Windsurf | ⬜ | Their instruction file + MCP already work; hooks need each vendor's documented payload |

## 1. Product

AutoMemory is a local-first daemon that gives every coding agent one shared, verified project memory plus file locks and handoffs. Agents reach it through MCP tools and optional hooks, so nobody re-explains the project each session.

| It is | It is not |
|---|---|
| Local Rust daemon, SQLite WAL, MCP server, CLI, dashboard | A hosted service or LLM |
| Memory of *why*: decisions, constraints, gotchas | A code indexer (CodeIntel bridge supplies the graph) |
| A referee: locks, trust tiers, verification, audit ledger | An LLM that writes answers or judges code |
| Pull-based: agents query on demand | A default boot-time prompt injector |
| Modular: every part behind a port | A monolith with fixed hooks |

**Problems it targets:** agent collisions (cross-agent merge conflicts reported at 41.7% vs 19.8% same-agent), lost context between agents, stale and contradicted memory, memory poisoning, retrieval noise, destructive commands, and no proof that memory helps. Deeper gap analysis: [research/multi_agent_ecosystem_needs.md](research/multi_agent_ecosystem_needs.md).

**Primary user:** one developer running 2–3 agents on one project across terminal and desktop apps. Later: small teams.

**Use cases:** switch agent mid-feature (`memory.about` + `baton.pickup`); parallel agents without overwrites (lease conflict shows holder/goal/age); "what did we decide?" (`memory.ask` with verified/changed/stale status); refactor or revert flips linked memories; plan a risky change in a Planning Room; catastrophic command blocked; swap a hook or adapter without restart.

## 2. v1 scope

| In v1 | Later (only after v1 works) |
|---|---|
| Core: ledger, store, plugin host, hook pipeline | More agents beyond the first three (17 connectors already exist, unsupported) |
| Adapters: **Claude Code, Codex, Antigravity** | Cloudflare managed SaaS / team sharing |
| Memory, decisions, trust tiers, verification | More agents beyond the first three |
| Locks, baton handoff, cross-review | Headless Runner, Autopilot as default |
| **Planning Room orchestration** | WASM plugins, neural embeddings |
| **Nuxt dashboard** (local `serve`) | Automated PRD/ADR generation, policy engine |
| Guardrails + human password gate | |
| Git-vault sync with encrypted keys | |
| Eval harness (memory on vs off) | |

Cut list: symbol-level leases, own AST/code graph (codebase-memory-mcp supplies it), auto-distill of every session, a full auto-generated AGENTS.md (only a small managed block is written, see Shared instruction files), shell-regex arms race, storing raw agent reasoning, agent-edited memory, debate as default, LLM calls in the daemon by default (the optional `[llm]` helper only suggests).

## 3. Feature catalog

Status: ✅ built · 🔧 in progress · ⬜ planned. "Verified" = measured or tested in this repo.

### Memory and recall
| Feature | Status | Notes |
|---|---|---|
| `ask`, `about`, `get`, `topics`, `search`, `recall`, `propose` | ✅ | FTS5 BM25 × time decay × confidence; ~0.56 ms over 500 memories (bench) |
| Candidate tier, promote on verify/human confirm | ✅ | `propose` rejected without provenance |
| Scope-aware dedup + version history | ✅ | In-place upsert |
| Decay, supersede chains, contradiction flag | ✅ | Contradiction = same tool, different version |
| Vector search | ✅ keyword (default) or hybrid; saved model profiles (ngram, Ollama, any OpenAI-compatible endpoint, a command), preset catalog, cloud API keys in the OS keychain (never in a file), real Recall settings page with test, background indexing with progress, cost estimate and daily cap | Cohere / Voyage / native Gemini adapters, `onnx` build feature |
| Procedural skills from trajectories | ✅ | `memory.skills` |
| Failure catalog (`failure.save/resolve`, proactive warning) | ✅ | |
| Constitution + token-capped context | ✅ | `boot_injection` off by default |
| Time-span digest (`15m`…`session`) | ✅ | Edits, files, red→green, failures paired with fixes |

### Verification and trust
| Feature | Status | Notes |
|---|---|---|
| Trust tiers 1–5, provenance, revoke | ✅ | Tier 4–5 withheld from default recall |
| Symbol/file-hash verify on recall | ✅ | File hashes built in; symbols from codebase-memory-mcp |
| PostCommit flips linked memories to `changed` | ✅ | |
| Revert/rewrite staleness | ✅ | A revert is a commit: PostCommit flips memories linked to its files to `changed` |
| Contradiction across memories | ✅ | Consolidation worker: replacement cues, mutually exclusive tool groups, version mismatch |

### Coordination
| Feature | Status | Notes |
|---|---|---|
| File/glob leases, TTL, heartbeat, override | ✅ | Advisory in Vibe, hard in Strict |
| Automatic locking (hook leases, shell prediction, `git status` reconciliation, prompt visibility, idle-holder watcher, coverage report) | ✅ | Agents never call `lock.acquire`; see ARCHITECTURE §7. Hook-less agents get alerts only |
| Worktree-shared memory/decisions/leases | 🔧 | Uncommitted work in `sync/` |
| Baton handoff | ✅ | |
| Task queue + cross-review (tests as judge) | ✅ | |
| Planning Room: state machine + MCP tools, human verdict (`automemory room`, dashboard Planning rooms): approve records the decision (critique as dissent) and one task per step; reject; another round | ✅ | Live trial T11 passed with Claude Code and Antigravity (3 fixes); the A/B eval is still open |
| Blackboard / checkpoint | ✅ | `lock.status` is the blackboard; `memory.digest auto_save` and the automatic `Session checkpoint` on Stop cover checkpoints |
| Pre-merge file-overlap check | ✅ | `automemory overlap`; file-level, git only |

### Safety
| Feature | Status | Notes |
|---|---|---|
| Shell tokenizer guardrail, Vibe/Strict/Review | ✅ | ~1.26 ms hook pipeline mean |
| Custom rules, Zero-SQL rule, review proposals | ✅ | |
| Human password gate for policy writes | ✅ | |
| Shadow mode for new rules | ✅ | Measure false-block rate |

### Interfaces
| Feature | Status | Notes |
|---|---|---|
| MCP server (58 tools), CLI | ✅ | |
| Nuxt dashboard + REST + WebSocket | ✅ | v1 scope; includes Agents & Locks (leases, rooms, batons) |
| Guided onboarding (CLI tour + dashboard coachmarks) | ✅ | `automemory tour`, `next`, `learn`, welcome card, 6-stop tour, sample data mode |
| Hook pipeline, hot reload, custom exec handlers | ✅ | |
| `next_action` on every tool reply | ✅ | |

### Sync
| Feature | Status | Notes |
|---|---|---|
| Git-ignored plan vault, AES-256-GCM, BIP-39 pairing | ✅ | |
| Cloudflare managed mode | ⬜ later | `workers/` |

### Proof
| Feature | Status | Notes |
|---|---|---|
| Optional LLM helper (`automemory llm`, Tools page): conflicts, duplicates, candidate notes from sessions | ✅ | Off by default; local model needs no password, cloud needs consent + key in keychain + daily token cap; output is Inbox proposals only. Tidy up also flags notes naming deleted files |
| Value report (`automemory stats`, Home card): collisions prevented, risky commands stopped, notes recalled, hand-offs | ✅ | Built only from existing audit, lease, memory and baton rows |
| Backups (`automemory backup`, restore with a safety copy, doctor check) and hook payload recording (`hooks record`) | ✅ | Restore is terminal-only (replaces the database) |
| Team trust overlay: a teammate's pin is a request, not a pin | ✅ | Needs a shared vault key; see `sync/deltas.rs` |
| Native Cohere, Voyage and Gemini embeddings; several LLM profiles | ✅ | Mock-server tests only, not run against the live services |
| Continuity brief (C1): goal, ruled-out options with reasons, open questions, next steps left by one agent and handed to the next with provenance; `automemory continuity`; syncs between computers | ✅ | Claude Code and Antigravity chats can be imported (opt-in), and Codex rollout files, Command Code transcripts and Cursor chats too (experimental: built from public descriptions, not yet checked against real chats), searched and opened by agents; the dashboard Continuity page (opt-in toggles, import, brief editor, preview, Hand off now) is built; the deterministic drafter (`continuity draft`, runs after every import; goal, constraints, ruled-out options, decisions, next steps, open questions as unconfirmed drafts) is built, the dashboard also shows a stale-brief hint, history sync and an AI-helper button; optional `llm run brief` suggests drafts (never trusted); chat refresh runs by itself at Stop, SessionEnd and Claude PreCompact for agents you turned on; `continuity sync-history` moves chat between your computers only on request (encrypted, password to send) (plan/continuity-plan.md) |
| Project library: PRDs, plans and notes for agents kept outside git, synced through the vault, found by every agent (L1–L6) | 🟡 | L1 built: `automemory library add/new/ls/show/search/rm/history` (password to change), `library.list/get/search`, tiers (title, outline, whole file), secrets refused, session-start index. Also (L2): a read-only copy under `.automemory/library/` with INDEX.md for agents without MCP, kept out of `git status` (local exclude, `.gitignore` never touched), `library link <glob>` / `[library] files` keeps files in place whether git tracks, ignores or never saw them, edits in the folder come back as revisions, `doctor` checks it. Also (L3): files sync between your computers through the encrypted vault (automatic with `sync auto`; `--personal` files stay put; removals travel; a file changed on both computers keeps yours and parks theirs in `.automemory/library/_incoming/`; kept-in-place files arrive in place). Also (L4): the dashboard Library page (file tree, summary/outline/whole-file tiers, new/edit/remove with the password, history and restore, git-status banner, Sync folder, files kept in place) and library files on the Documents page. Also (L5): agents save their own notes with `library.put` (never over your files, no secrets) and suggest edits to yours with `library.propose` (Inbox item with a diff; only you apply it, one at a time); `library:<path>` can be cited in brief items and as room evidence; `library add --move` tells you whether git tracked the file. Not yet: attachments and model-written abstracts (L6). Plan: plan/library-plan.md |
| Dashboard control (Safe / Protected / Terminal-only tiers, jobs tray, Tools, agent set-up, decisions, lock settings, activity) | ✅ | Every write route has a tier (audit test); Protected asks the password every time |
| Benchmarks (`cargo bench`) | ✅ | Hook ~1.26 ms, recall ~0.56 ms, distill (40 events) ~0.11 ms, binary 10.3 MB, 303 Rust tests + 38 script suites ([tests/README.md](../tests/README.md)) |
| Eval harness | 🔧 | `tests/eval/run_eval.py` (prepare / score / report) is ready; needs ≥20 real tasks and the runs (Phase 2 gate). `automemory eval` is only a self-check suite |

## 4. Success metrics

| Metric | Target |
|---|---|
| Silent overwrites | 0 |
| Hook p95 | < 5 ms for the pipeline (≈1.3 ms mean). End to end, process start included: ≈9 ms p95, ≈5 ms of it binary start-up (`tests/perf/hook_p95.py`) |
| Task success, memory on vs off | Not worse overall; better on non-inferable tasks |
| Stale-flag precision | ≥ 90% |
| Install → first lock or recall | < 5 min |
| Tokens per session vs baseline | Lower |

Not measured and therefore not claimed: distillation compression ratio, dedup row counts, third-party latency comparisons.

## 5. Acceptance criteria (v1)

New chat continues from prior decisions; projects never leak into each other by default; global preferences apply unless overridden; capture works with no agent cooperation; a failure with a later fix becomes a linked record; works fully offline; synced data is encrypted before leaving the machine.
