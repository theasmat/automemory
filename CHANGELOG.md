# Changelog

What has shipped, by area. Dates are in `git log`. Open work: [docs/roadmap.md](docs/roadmap.md).

## Foundations

- Worktree-aware project resolution, shared leases; committed
- `connect`/`disconnect` for Claude Code and Codex (hooks + MCP), Codex via `config.toml`; refuse unparseable configs, atomic writes
- Dashboard origin/host guard; human password on proposal decisions (dashboard, CLI, and MCP refuses agents); UI prompts once per tab
- Lease acquisition atomic across processes; migrations atomic and loud; poisoned mutex recovers
- Guardrail precision (printed text, here-documents and file content are not commands) and bypass coverage; warnings now reach the agent as hook context
- CodeIntel via codebase-memory-mcp (answers "unknown" when the engine is absent); `doctor` checks engine version and installed hooks
- Automatic `Session checkpoint` memory on Stop; `automemory overlap` for pre-merge file overlap; revert/contradiction staleness covered by PostCommit and the consolidation worker
- Dashboard Agents & Locks page (`/api/coordination`)
- Delta sync wired into the daemon pass; fixed two bugs found by wiring it (edits were never pushed after a pull; old delta files looked like conflicts)
- Hook opens the database without re-running the schema: end-to-end p95 21 ms → 9 ms
- Sync gateway: webhook signatures verified, unsigned test payload only with the dev flag, constant-time admin secret
- `tests/` directory: contract, guardrail, collision, lifecycle, password gate, dashboard, CodeIntel, latency, eval harness, live checklist

## Automatic locking

- Session-bound leases (heartbeat on any activity, SessionEnd releases, idle TTL 600 s); per-path ignore policy (`[locks] ignore`)
- Shell write prediction; `git status` reconciliation with retroactive leases and PostToolUse warnings
- Prompt hook lists files other agents hold; `locks --coverage` and `doctor` coverage line
- Watcher alerts for changes under an idle holder's lease (dashboard)
- Antigravity hook output follows its documented schema (found by a live run: PostToolUse `note` was rejected); notes delivered on PreInvocation
- `mode set` / `skill` used the folder name as the project while hooks used the registered project (live run: Strict was set on `automemory`, hooks ran under `auto`, so nothing was denied); both now use `project_slug_for_dir`
- `connect agy` registered hooks twice (workspace + plugin copy), so every agy event ran and was recorded twice; plugin copy removed
- Antigravity: single hook registration (global or workspace), safe merge into an existing hooks.json, CLI + App + IDE skill paths, no stale plugin copies; hooks always answer in schema and fail open; duplicate registrations replay one reply; `disconnect all [--everywhere] [--dry-run]` removes every AutoMemory registration; `connect` no longer overwrites a user's git post-commit hook

## Sync, resume and retrieval modes

- Stable `project_key` (git remote / root commit) on projects; deltas carry it and map to the local project on apply
- Op-log as the only sync path; pull on SessionStart (2 s budget), push on SessionEnd; retire last-writer-wins import
- `tests/scenarios/two_machines.py`: two HOMEs, different checkout paths, one shared vault
- Resume pack (`automemory resume`, `memory.resume`, shown once at session start) and hand-off bundle (branch, sha, dirty files, next step)
- Sync baton, checkpoint, review-proposal and doc kinds; `automemory sync bootstrap` for a new machine
- `[retrieval] mode = keyword|hybrid`; `EmbeddingProvider` v2 (model id, batch, local); vectors per (memory, model); lazy indexing during hybrid search (a background reindex in `serve` is not built; use `embeddings reindex`)
- Providers: `ngram`, `ollama`, `openai`-compatible, `exec` (any command); `automemory embeddings status|test|reindex|use|off|eval`; `tests/scenarios/embeddings_modes.py`

## Shared knowledge across agents and IDEs

- `build_rules_view`, `export-rules`, session-start refresh, `disconnect` removes the block; `tests/scenarios/rules_export.py`
- `automemory note`, dashboard **+ Add memory** (`POST /api/memories`)
- Dashboard "Project brain" page (plain-language Decisions / Conventions / In flight; which agent saw what)

## Dashboard UI/UX redesign

- First-run tour (CLI + dashboard): brief for an implementing agent in [onboarding tour spec](docs/specs/onboarding-tour.md) (L1 shared core, L2 CLI tour, L3 dashboard, L4 agent notice)
- P1 foundation + Home, P2 Project brain, P3 Agents & locks + Inbox, P4 Sync wizard + Safety, P5 polish, §12 Light theme (WCAG 2.2 AA), §14 Cleaner/calmer `ui-ux-pro-max` pass (progressive-disclosure step editor, global form boundaries, mobile TopBar, vector icons)

## Configurable embeddings and secure keys

- E1 `secrets.rs` on the `keyring` crate (encrypted-file fallback only without a keychain); `embeddings key set|status|remove`; master key written through keyring, not the command line, `embeddings key set|status|remove`, master key off the command line
- E2 profiles + preset catalog, `embeddings profile add|list|use|remove`, `embeddings models` (lists installed Ollama models); the flat fields stay as the `default` profile
- E3 `/api/recall*` (all writes behind the human password, keys write-only) and a real Recall settings page
- E4 indexing with progress, retries, daily token cap and cost estimate, cancel, unused-vector cleanup (`embeddings reindex|clean`, `/api/recall/index`)
- E5 native Cohere/Voyage/Gemini adapters (see I7). ONNX is not planned

## Control everything safe from the dashboard

- U1 `policy.rs` tiers + route audit test, job runner (`/api/jobs`, Jobs tray), indexing on it, local indexing without password
- U2 Settings > Tools / Health (doctor, export-rules, hand-off, sync pull, docs rescan, graph index, eval, backup); logic reused via subprocess (`run_self`) and library calls, no separate `ops/` module
- U3 Agents > Set up an agent (preview, connect, disconnect)
- U4 Decisions view; restore, pin, veto, revoke in Project brain
- U5 Settings > Locks and Data; activity log view

## Quality, proof and reach

- N1 Proof: eval harness with/without AutoMemory, published numbers, CI gate
- N2 Optional LLM helper: conflicts, duplicates, session distill (proposals only; one profile). Staleness without an LLM: Tidy up flags notes that mention files that no longer exist
- N3 Memory hygiene UI: confidence, last used, never recalled, bulk archive
- N4 Agent matrix test: 13 agents connect, get their instruction file and disconnect cleanly. Hooks/contract fixtures for Cursor, Windsurf, Gemini CLI, Copilot still need recorded real payloads (they stay experimental, MCP + rules only)
- N5 Team sync (first step): records carry their author, notes from a teammate show "from <name>", approved notes arrive active and unapproved ones stay candidates (tested with two people on a keyed vault). Not built: per-person trust tiers (a receiver-side overlay is needed; clamping the record itself makes fingerprints diverge and creates false conflicts), person setting UI
- N6 Performance gate in CI (hook latency, recall, binary size)
- N7 `automemory update` (asks first, verifies .sha256, refuses unchecked downloads; scenario with a mock release server), installers verify checksums, `scripts/make_formula.sh` prints a Homebrew formula. Not done: Apple signing/notarisation (needs a developer account), publishing the tap
- N8 Hardening audit of `serve/`, sync and export-rules. Fixed: plaintext sync deltas accepted by a keyed vault (forged trusted memories), `is_loopback` substring bypass (`localhost.evil.com` skipped consent and redaction), export-rules following symlinks out of the repo, unbounded backups, agent name as CLI flag. Not covered: fuzzing the hook JSON parsers

## Improvements

- I1 Value report: `automemory stats`, `/api/value`, Home "Last 7 days" card (figures come from existing rows; sample data excluded)
- I2 `automemory hooks record on|off|status` saves redacted payloads; `tests/contract/record_to_fixture.py <agent>` turns them into a golden contract fixture. Fixtures for Cursor/Windsurf/Gemini/Copilot still need recordings from real sessions
- I3 Trust overlay for teams: a teammate's pin arrives active with a `team_pin` request; approving restores their exact record, so no sync conflicts or bounced copies (tested incl. teammate edits). Person name is set by AUTOMEMORY_PERSON or git user.name; no settings UI
- I4 Browser smoke test (Playwright): 21 pages x light/dark, password prompt; skipped with a message when Playwright is missing
- I5 Lock hints from `/api/policy`: `<Button lock="/api/...">` shows a lock when the server says Protected; a path missing from the table logs an error that the browser smoke test fails on. Placed on the main Protected buttons (11), not every one
- I6 `automemory backup [create|list|restore]` (restore keeps the old DB as before-restore-*, refuses while in use or for damaged files); doctor shares the backup code (newest 10 kept) and warns about unreadable copies
- I7 `[llm.profiles.*]` (CLI `llm profile add|use|list|remove`, `key-set --profile`, `/api/llm` profiles, Tools card with a setup picker; keys under `llm.<name>`) and native Cohere/Voyage/Gemini embedding adapters (query vs document input types, presets). Request shapes follow each vendor's published API and are tested against mock servers only; not run against the live services
- I8 Seeded mutation fuzzing on stable (no nightly/cargo-fuzz here): `crates/automemory-core/tests/fuzz_like.rs` (shell parser, write-target prediction, guardrails, redaction, LLM JSON, delta files) and `tests/scenarios/hook_fuzz.py` (garbage into the real hook). Found and fixed: ~8000 nested `$((` overflowed the stack and aborted the hook (commands after it went uninspected), 60000 nested made the hook hang for 30 s+. Now cut off at depth 40 and blocked. Not done: coverage-guided cargo-fuzz in CI
- I9 (written, NOT run on Windows): `automemory update` supports Windows (zip, rename-aside swap), Unix-only tests are gated, git post-commit hook path uses forward slashes, test harness knows .exe/USERPROFILE, and CI has an advisory `windows` job (build, cargo test, update/registration/agents_matrix/hook_record/hook_fuzz scenarios). Make it required after its first green run; expect fixes. Known gaps: the `dirs` crate ignores HOME overrides on Windows, hook commands assume the agent runs them through a shell that accepts a quoted path, exec handlers need `sh` (Git for Windows)
- I10 Cosmetic backlog: decision cards show the title and "Why:" only, Home agents line no longer repeats the number, Activity is its own page (filters, linked from Settings and the Advanced menu); smoke test covers `/activity`

## Planning rooms people use

- R1 Start a room from the dashboard (topic, question, invited agents, evidence picker, scope, constraints); the prompt hook tells an invited agent once per turn, with its own session id; `room.join`/`room.status` return the brief. Brief kept in `sync_kv` (rooms do not sync)
- R2 Human notes in a room; edit the plan (decision, steps with owner/files/check, persistent drafts, `edited_by_human` provenance) before approving
- R3 Tasks with order/owner/files/check and live state on the room page; claiming locks files and submitting releases them; Revise → follow-up room supersedes prior decision and cancels unclaimed tasks
- R4 1–3 rounds, multi-critic rotation, 30-min turn timeout with Skip turn / End room; live WebSocket `room_updated` updates

## Continuity

- C1 Brief store (schema v15) + `continuity.brief` tool + injection in the resume pack (token budget `[continuity] inject_tokens`) + rules text + `automemory continuity` + brief sync with "from <device>" labels. Also fixed: a config.toml that does not parse now locks the human gate (it used to switch the password off) and `doctor` reports it
- C2 Reader trait + registry + `testkit` contract, Claude Code reader (jsonl, incremental by byte offset, thinking/tool output/side chains never kept), archive with FTS5 (schema v16), importer (secrets and home folder removed, 4000-char clip), `chat.search`/`chat.get`, `automemory continuity sources|enable|disable|import|search|history-forget`; opt-in per agent in `[continuity] read_chats`. Checked on a copy of the real chats: 13 sessions, 234 messages, no secrets found
- C3 Antigravity reader: conversation databases (protobuf steps decoded by path), workspace match, title from annotations, the agent's own plan files as artifacts, tool-call notes searchable only through chat.get. Checked read-only on your real chats: 9 conversations found, 8 imported (2,082 messages). Format notes in docs/research/antigravity_chat_format.md
- C4 Dashboard Continuity page (opt-in, import, brief editor, preview, Hand off now)
- C5 Deterministic drafter + confirm flow
- C6 Automatic triggers + manual history sync
- C8 Continuity eval (golden chats, floor 0.8), optional LLM drafting (`llm run brief`, drafts only), dashboard history sync + stale hint + AI-helper button; live trial T12 prepared (`tests/manual/t12_check.py`) but NOT RUN: needs you

## Project library

- L1 Store (schema v17), secrets check, deterministic tiers, CLI (`automemory library add|new|ls|show|search|rm|history`), MCP `library.list/get/search`, session-start index
- L2 `.automemory/library/` copies with INDEX.md (kept out of git status through `.git/info/exclude`, `[library] exclude_from_git`), files kept in place from global config `[library] files` (any git state), edits taken back, doctor check; event-driven by hooks and `library sync` (no always-on file watcher)
- L3 Vault sync of library files as a `library` record kind (encrypted deltas, same key as the vault; removals travel as markers; personal files stay; both-changed files keep yours, theirs parked in `.automemory/library/_incoming/` with an Inbox item; kept-in-place files are written in place on the other computer and never overwrite a different local file)
- L4 Dashboard Library page (tree, summary/outline/whole-file tiers, new/edit/remove, history and restore, git-status banner, sync folder, files kept in place) and library files on the Documents page
- L5 `library.put` (agent's own files, no overwriting yours, no secrets, 100-file cap), `library.propose` (Inbox item with a diff, applied only when a person approves, never in bulk, 5 waiting at most), `library:<path>` citations in brief items and room evidence, `library add --move` says whether git tracked the file

## Consolidation

- K1 One approve/reject path for Inbox items (fixes approve-all), shared reader helper, one spawn helper, one `clip`, remove 5 redirect pages (the two empty tables stay: `sessions` is written by the demo data and `session_notes` is a short-lived lock-note buffer, empty between sessions)
- K2 `memory.search {scope}`, `memory.list {view}`, `continuity.brief {action: handoff}` (baton), 10 names retired but still answering, a test that fails when a tool is not taught in the rules text (instead of generating the text)
- K3 Skills/traces/failures written once per thing (stable ids; grep/diff exit 1 is not a failure), `memory prune` and a doctor line for existing duplicates (on the maintainer's database a dry run finds 8,880 skills and 168 traces; not run for him), one Activity page with tabs (old links forward), one Models page for the search model and the AI helper (the two screens are merged; their routes stay separate)
- K4 Graph tools and the Graph page dropped (the codebase-memory server covers symbols; backend routes and the "Index the code graph" job stay). Reviews were NOT turned into rooms: a review is a diff with a test verdict, a room is a plan with a human verdict, so the mapping would lose the test gate; the three review tools are only hidden from the tool list
- K5 One place for documents: the Library page and `library.list/get` also show the project's own documents found in the repository (source repo; the Documents page and `doc.list/get` names are folded); the Rules page became a card on Project brain; cloud-sync commands hidden and gated by AUTOMEMORY_EXPERIMENTAL_CLOUD; legacy `sync export/import/restore` hidden. NOT done: the scanner and watcher still index repository documents separately from the library files (a deeper merge would rework retrieval), and the legacy vault code is hidden, not deleted

## Identity and removals

- Y1 Agent name stamped by the server (`automemory mcp --agent <name>` written by `connect`, which also upgrades an older Codex entry); a different `agent` argument is ignored and written to the audit log as `mcp.agent_mismatch`. Entries made before this keep trusting the caller's name until `connect` is run again
- Y2 A room proposal cannot be critiqued by the server connection that made it, whatever session id the call uses (other "same author" rules are about person versus agent, which the name does not affect)
- Y3 The client name from `initialize` is recorded; `doctor` ("Checking agent identities") flags another program on an entry and entries with no agent name
- Y4 Records written through a configured server carry its name; an unconfigured server of a known agent (Claude, Antigravity, Codex, Cursor) fills in the name from its client when the call gives none
- Y5 MCP servers re-execute themselves after an update (Unix): before serving a request the server checks whether the installed file was replaced and, if so, becomes the new binary on the same pipes and answers that request itself; a new file that does not answer `--version` is ignored; at most 5 restarts per process. Servers started before this version still need one manual restart (`doctor` names them)
- Y8 Session checkpoints say what happened: files edited (relative), tests passed/failed, what is still failing, the brief's next step. Also fixed: the digest never saw Antigravity's `Path` or Claude Code's `file_path`, so it reported "edits across 0 files"
- Y9 Agent-written notes not recalled for 60 days rank at half (a quarter after 120) and are offered in the Inbox as "archive this?" (5 per session start, 15 waiting at most, each note once); a person's notes, pinned notes and decisions never fade; keeping a note counts as a use
- R1 Removed: the 19 retired MCP names (old names answer "was removed, use …"), cross-review (tools, queries, models and the Stop nudge; the unused `reviews` table stays in the schema)
- R2 Legacy vault and cloud client removed from the CLI and dashboard server (`sync export/import/restore/init/status/push/pull/watch`, `/api/cloud/*`, the edge branch of `memory search --semantic`); `sync pair` keeps the recovery phrase and no longer talks to a cloud. Kept on purpose: `[cloud] encryption_key` in config (it can still be the vault key), and the `link` command (hidden, superseded by the Library)
- R3 `connect` for kilo, aider, warp, devin, openclaw and hermes removed (`disconnect` still removes what an older version wrote); `connect other --mcp-file <path> --name <agent>` covers any tool that reads an MCP servers JSON file. The audit said 19 of 22 targets were untested: wrong, 13 of them have a connect/disconnect/instruction-file check in `tests/scenarios/agents_matrix.py` (still no live-agent trial), so those were kept
- R4 Own code graph code deleted (`graph/`, graph tables via schema v20, `/api/graph`, the "Index the code graph" job; `NoCodeIntel` replaces the file-hash fallback); skills are no longer written by the daemon unless `[skills] auto = true`; `plugin` and `adapter` hidden from `--help`

## Security hardening

- Agents cannot overwrite or unlock a locked preference (database rule and MCP refusal); changing the human password needs the current one
- Documents: symlinks, excluded paths and files with secrets are no longer tracked; same-named documents no longer overwrite each other; categories match whole path words
- Git sync refuses remote and branch names that start with `-`
- Rules export keeps the file's permissions (temp file, then rename)
- The extraction worker runs its SQLite batch off the async threads that serve the dashboard
- `lock list` no longer returns session ids
- Agent rules, skill and Cursor rules now say to do only what was asked: no commits, pushes, hand-offs or saved lessons unless the person asks (X8; the live trial is still open)
