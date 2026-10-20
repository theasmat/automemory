# Continuity: carry a long chat from one agent to another

Status: C1–C4 built (Oct 2026); C5–C8 are plan. Summary and phases are in [docs/PLAN.md §16](../docs/PLAN.md); tasks C1–C8 are in
[docs/TASKS.md](../docs/TASKS.md).

## 1. The problem and the target

You chat with Antigravity for hours: goal, options weighed, things rejected, files touched, half-made decisions. You switch to Claude
Code (or Command Code, Z code, Codex, a new session of the same agent after compaction). Today the new agent gets a mechanical
hand-off ("12 tool calls, last: cmd:success", files held, decisions in force, git state). It does **not** get the understanding:
what we are building, why we chose this, what we ruled out, what is still open, what comes next.

Target: a new agent starts with the *same understanding*, in a size that grows with need, and can dig into the original discussion
on demand. Works for as many agents as possible; one codebase that makes adding an agent a small, isolated change.

Decisions from the maintainer (Oct 2026):
| Question | Answer |
|---|---|
| What carries | Brief **plus rejected ideas and reasons plus searchable chat history** |
| Capture | **Both**: the agent writes the brief, and AutoMemory reads local transcripts to draft/verify it |
| Trigger | **Both**: automatic (at Stop, before compaction, at session end) and manual (button / command) |
| Which switches | As many agents as possible, both directions, and same-agent new sessions |
| Privacy | **Opt-in per agent, local only**; secrets removed before storing |
| Sync | **Brief syncs automatically; chat history syncs only when asked** |
| Agents we cannot read yet | Agent-written brief for now; research each agent later; **code must be modular** so a reader is easy to add |
| Size | "As much as needed": no fixed small cap; layered so the start of a session stays light and details are fetched on demand |

## 2. What exists today (achieved) and what is missing

| Piece | State |
|---|---|
| Resume pack at session start / first prompt: last hand-off, decisions in force, git state, digest of last session | Built (`resume.rs`, hooks) |
| Hand-off written at Stop / session end, `baton.pass` / `baton.pickup` | Built, mechanical (counts, files, last commands) |
| Decisions, notes, instruction files for every agent, rooms and tasks | Built |
| Hooks tell us tool calls and, for Claude, prompts; never the assistant's reasoning or the full chat | Limit |
| Where chats live locally (read-only research, this machine) | Claude Code `~/.claude/projects/<project>/<session>.jsonl` (user, assistant, `ai-title`, compaction summaries); Antigravity `~/.gemini/antigravity/conversations/<id>.db`, `conversation_summaries.db` (with full-text index) and `brain/<id>/*.md` plans and walkthroughs (agent-written summaries already); Codex keeps rollout files under `~/.codex/sessions` (**verify**) |
| A brief of goal / rejected / open questions / next steps, chat search, a reader per agent, the opt-in and redaction path | **Missing**: this plan |

## 3. Design

### 3.1 Two layers, three sources of truth
- **Brief** (small, structured, always injected in part): goal, status, constraints, decisions (by reference to the decisions table),
  **rejected options with reason**, open questions, next steps, key files, glossary, user preferences seen. Every item carries
  **provenance**: who wrote it (agent name + session, or "you"), when, and how (`agent`, `transcript-draft`, `human`), and a state:
  `draft` (from a transcript, unconfirmed), `confirmed` (an agent or you accepted it), `edited` (you changed it).
- **Chat archive** (large, searchable on demand, never injected whole): redacted messages of imported sessions with FTS5.
- Trust rules stay: decisions remain in the trusted decisions table (briefs only point to them); everything in a brief is shown to
  the next agent **with its provenance**, drafts marked as drafts. A brief never creates trusted knowledge by itself.

### 3.2 Capture
1. **Agent-written (works for every agent, day one).** MCP tool `continuity.brief` (`get`, `update` with a list of typed items). The
   rules text tells agents: before you stop, before a compaction, and when the human says "hand off", update the brief. Claude Code
   gets a PreCompact/Stop nudge through hooks; others get the rules text and the dashboard button.
2. **Transcript readers (opt-in per agent).** Each agent has a reader that finds that project's sessions, reads them incrementally
   (offset or timestamp since last import), redacts, and stores messages in the archive. Readers never write to the agent's files.
3. **Drafter (deterministic, no LLM).** From the archive and tool events: first user messages and the latest user restatements →
   goal; explicit choices ("let's use", "decided", "go with") → decision candidates; "no", "don't", "instead of", reverted edits →
   rejected options; unanswered questions → open; the agent's own summaries when the app keeps them (Antigravity
   `conversation_summaries` and `brain/*.md`, Claude compaction summaries) are used first because they are already the agent's
   words. An optional `[llm]` profile can draft better, but only as proposals (existing rule).
4. **Confirm.** The next agent's first prompt includes the draft items it should confirm or correct with `continuity.brief`
   (`confirm`, `correct`, `reject`). You can do the same in the dashboard.

### 3.3 Triggers
- **Automatic**: incremental import and brief refresh at Stop (cheap: only new lines), at SessionEnd, and before compaction where
  the agent has such a hook (Claude PreCompact). Always off for an agent until it is opted in; agent-written updates need no opt-in.
- **Manual**: dashboard **Hand off now** (imports, drafts, shows the brief for review, writes it), `automemory continuity
  handoff`, and the plain phrase "hand off" for agents that follow the rules text.

### 3.4 Delivery to the new agent
- Session start / first prompt hook injects the **core brief**: goal, status, constraints, next steps, open questions, rejected
  options (one line each), decisions in force, key files. No hard cap on what is stored; the injected part is layered and
  prioritised (confirmed over draft, recent over old) up to `[continuity] inject_tokens` (default generous, configurable per agent),
  and ends with "more: `continuity.brief` (all items), `chat.search`, `chat.get`".
- MCP tools: `continuity.brief`, `chat.search {query, session?, agent?}`, `chat.get {session, from, to}`. Search results carry
  session, agent, time and a snippet, never whole transcripts.
- Instruction files (`export-rules`) get one short paragraph pointing at these tools, so agents without hooks still know.

### 3.5 Privacy and control
- Reading an agent's transcripts is **off** until you enable that agent (Protected, password). The page shows exactly which folders
  will be read.
- All text goes through `redact_text` before it is stored; raw transcripts are never copied. Paths outside the project are dropped.
- `automemory continuity forget <session|agent|all>` and a dashboard button delete archived chat; retention default 90 days
  (configurable); nothing is ever sent to a model unless the optional `[llm]` profile is on and you ran a draft with it.
- Local only by default.

### 3.6 Sync
- **Brief**: a new synced record kind `brief` (per project key, like memories); it travels automatically with the same encrypted
  vault and keeps author and provenance. A teammate's or another device's brief arrives as `draft` until confirmed here.
- **Chat history**: never automatic. `automemory continuity sync-history --push|--pull` and a dashboard button, one encrypted file
  per session (`chat/<session>.enc`), same key as the vault, shown with size before sending.

## 4. Modular code (requirement)

New area `crates/automemory-core/src/continuity/`:
```
continuity/
  mod.rs              public API: brief store, import, draft, deliver
  model.rs            BriefItem, ItemKind, Provenance, ChatSession, ChatMessage
  store.rs            tables and queries (brief_items, chat_sessions, chat_messages + FTS5)
  redact.rs           uses redaction::redact_text, path scrubbing
  drafter/            mod.rs (trait Extractor + runner) and one file per heuristic: goal.rs, decisions.rs, rejections.rs, questions.rs
  deliver.rs          layered rendering for hooks and MCP (priority, token estimate)
  readers/
    mod.rs            trait TranscriptReader + registry (a single list)
    claude_code.rs    ~/.claude/projects jsonl
    antigravity.rs    conversations/*.db, conversation_summaries.db, brain/*.md
    (codex.rs, gemini_cli.rs, command_code.rs, zcode.rs added later: one file each)
    testkit.rs        contract test every reader must pass
```
- `trait TranscriptReader { fn id(&self) -> &'static str; fn locations(&self, home: &Path) -> Vec<PathBuf>; fn sessions(&self, project: &ProjectRef) -> Vec<SessionRef>; fn read(&self, s: &SessionRef, since: Option<Cursor>) -> Result<(Vec<RawMessage>, Cursor)>; }`
- `trait Extractor { fn id(&self) -> &'static str; fn extract(&self, ctx: &DraftContext) -> Vec<BriefItem>; }`
- **Adding an agent = one reader file + one registry line + a fixtures folder** (`tests/continuity/fixtures/<agent>/`, a small
  anonymised sample) + passing `testkit` (finds sessions of a project, reads incrementally without duplicates, survives a corrupt
  line, redacts a planted secret, never writes to the source). A checklist in this file's §8 is the contract.
- Agent-specific *delivery* stays in the existing adapter table; the brief itself is agent-neutral.
- CLI, dashboard routes and MCP tools call only `continuity::` functions (no logic in routes), like the rest of the project.

## 5. Dashboard
New page **Continuity** (under Agents in the menu):
- **Sources**: one row per agent: name, "Read its chats" toggle (Protected), folders read, sessions found, last import, size,
  Import now (Jobs tray), Forget. Agents with no reader show "Agent-written brief only" and the rules-text status.
- **Brief**: the items grouped (Goal, Decisions, Rejected, Open, Next, Files), each with provenance chips and state; edit, confirm,
  delete (Protected). **Hand off now** button; preview of exactly what the next agent will be given and its size.
- **History**: search box over the archive; a result opens the exchange in context; "Sync history to my other computer" with size.
- Home gets a line when a brief is stale ("Antigravity worked 2 h since the last brief").

## 6. API and tiers
| Route | Tier |
|---|---|
| `GET /api/continuity` (sources, brief, status), `GET /api/continuity/search`, `GET /api/continuity/preview` | read |
| `POST /api/continuity/import` | Safe (only for opted-in agents; local read) |
| `PUT /api/continuity/source/:agent` (enable or disable reading) | Protected |
| `PUT/DELETE /api/continuity/items/:id`, `POST /api/continuity/handoff` | Protected |
| `DELETE /api/continuity/history` (forget) | Protected |
| `POST /api/continuity/sync-history` | Protected (sends data off the machine) |

## 7. Phases
| Phase | Scope | Done when |
|---|---|---|
| C1 | Model, store, redaction, `continuity.brief` MCP tool, brief injection in hooks, rules text, `automemory continuity` CLI (show, update, forget), brief sync as a record kind | Any agent updates the brief; the next agent (different adapter) receives it with provenance; brief appears on a second machine |
| C2 | Reader trait, registry, testkit, **Claude Code reader**, archive + FTS, `chat.search` / `chat.get`, redaction test with a planted secret | A long Claude session is importable incrementally and searchable; no raw copy exists |
| C3 | **Antigravity reader** (conversation db, summaries db, brain files) | Same as C2 for Antigravity; its own summaries seed the brief |
| C4 | Dashboard Continuity page, opt-in toggles, Jobs integration, preview, Hand off now | You can enable an agent, import, review, edit and hand off from the page |
| C5 | Deterministic drafter (goal, decisions, rejections, questions), draft/confirm flow in the first prompt | A fixture transcript yields the expected items; a new agent confirms or corrects them |
| C6 | Triggers: Stop, SessionEnd, PreCompact incremental refresh; `sync-history` manual push/pull | Switching mid-task after a Stop needs no manual step; history moves only on request |
| C7 | Research spikes and readers for Codex, Gemini CLI, Command Code, Z code, Cursor and others, one agent at a time (a sample of the agent's chat files first) | Each reader passes the testkit; the agent is marked "transcripts supported" |
| C8 | Continuity eval + live trial: golden transcripts → brief → questions a new agent must answer (decisions, rejections, next step); trial **T12** Antigravity long chat → Claude Code continues; optional `[llm]` drafting as proposals | Eval score recorded; T12 passes; LLM drafting beats the deterministic drafter or is dropped |

Order: C1 (value for every agent immediately) → C2 → C4 (so the human sees and controls it) → C3 → C5/C6 → C7 per agent → C8.

## 8. Checklist to add an agent reader (the modular contract)
1. Collect a small **anonymised sample** of that agent's chat files for one session (no secrets) into `tests/continuity/fixtures/<agent>/`.
2. Write `readers/<agent>.rs` implementing `TranscriptReader`; read-only; tolerate unknown fields and corrupt lines.
3. Add one line to the registry in `readers/mod.rs`.
4. Run `testkit` (sessions found for the project, incremental read yields each message once, corrupt line skipped, planted secret
   redacted, source files unchanged). Fix until green.
5. Add the agent to the dashboard Sources list automatically (the page renders the registry); add its row to FEATURES.md.
6. Add a research note (where the files live, format, version seen, what breaks it) to `docs/research/`.

## 9. Risks
- **Private formats change.** Mitigation: readers are isolated, tolerate unknowns, the testkit runs on fixtures, and a reader that
  fails marks the source "needs update" instead of breaking hand-off; the agent-written brief keeps working.
- **Secrets in chats.** Mitigation: opt-in, redaction before storage, no raw copies, forget command, local only, history sync manual.
- **A wrong brief misleads the next agent.** Mitigation: provenance and draft state on every item, confirm step, human edit, brief
  never creates trusted decisions.
- **Context cost.** Mitigation: layered delivery with a configurable ceiling; details by tool.
- **Duplicate or stale briefs across devices.** Mitigation: brief is a normal synced record with fingerprint and conflict proposal
  (existing sync rules).

## 10. Still to settle (ask when reached)
- Exact names and folders for **Command Code** and **Z code** (and any other agents you use): send a sample chat file or path for each.
- Default retention (90 days proposed) and whether the archive may include other projects' chats an agent opened in this folder.
- Whether drafting with an `[llm]` profile should be offered at C5 or only at C8.
