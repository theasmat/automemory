# Agent brief: first-run onboarding (CLI tour + dashboard tour)

You are implementing a planned feature in AutoMemory (local-first Rust daemon + CLI + Nuxt dashboard). This file is
self-contained: read it, then work through the phases in order. The design and rationale live in
[ui-ux.md](ui-ux.md) section 13 (read it first), the theme and design system in sections 5 and 12.

## Ground rules
- Read `CLAUDE.md` first. The daemon makes no LLM calls; agents never use raw SQL on the DB (Zero-SQL rule). Keep doc counts
  (tools, tests) in sync with code. Do not add new top-level docs; update `docs/TASKS.md`, `docs/FEATURES.md`, `docs/PLAN.md` in place.
- Build/test: `tests/run_all.sh` (or `cargo test --workspace`). Use `AUTOMEMORY_BIN=$PWD/target/debug/automemory` for single scenarios.
- Commit per phase with a clear message ending with the attribution line your environment requires. Never commit `.mcp.json`.
- **Another agent may be editing `clients/web` (dashboard redesign, theme).** Before touching any file under `clients/web`,
  run `git status` and read the file's current content; do not revert changes you did not make; keep
  `tailwindcss: { cssPath: '~/assets/css/main.css' }` in `clients/web/nuxt.config.ts` (without it no theme colours load).
  Do Phases L1 and L2 (Rust only) first; they never touch `clients/web`.
- Rebuild the embedded dashboard only in L3/L4: `cd clients/web && pnpm run generate`, then
  `rsync -a --delete dist/ ../../crates/automemory-cli/src/dashboard/assets/` (no `rm -rf` with variables).
- When installing the binary: `rm ~/.cargo/bin/automemory; cp target/release/automemory ~/.cargo/bin/automemory; codesign --force -s - ~/.cargo/bin/automemory`.
- Tests and the tour must never touch the user's real `~/.automemory`: use a temp `HOME` (see `tests/lib/harness.py`, class `Sandbox`).

## What exists that you should reuse
| Need | Use |
|---|---|
| Key/value state shared by CLI and daemon | `Database::sync_kv_get/sync_kv_set` (`crates/automemory-core/src/sync/deltas.rs`) |
| Is a human password set | `automemory_core::auth::stored_password_hash()` |
| Is a sync remote set | `automemory_core::sync::auto::vault_settings()` (returns `(vault_root, Option<remote_url>)`) |
| Which agents are registered | `commands::disconnect::inventory(home, cwd)` (`crates/automemory-cli/src/commands/disconnect.rs`) |
| Instruction-file block present | marker `<!-- automemory:begin` in the files listed in `commands::rules_export::TARGETS` |
| Project for a folder | `commands::continuity::project_for` |
| Notes, decisions, hand-off, resume | `continuity::note/resume/handoff`, `decision::record_decision`, `resume::build_resume` |
| Hook calls with fake agents | `automemory hook --agent claude --event pre-tool/post-tool/prompt ...` payloads in `tests/contract/` and `tests/lib/harness.py` (`Sandbox.hook`) |
| First-prompt one-time message | `resume_once` in `crates/automemory-cli/src/commands/hook.rs` |
| REST routes and human gate | `crates/automemory-cli/src/commands/serve/routes/*.rs`, `require_human_header` in `routes/guardrails.rs` |
| Dashboard store/API/components | `clients/web/stores/automemory.ts`, `utils/api.ts` (`request()` returns `{ok,data,error}`; `humanFetch(endpoint, actionName, options)`), `components/*` |
| WebSocket live updates | `clients/web/composables/useWsPulse.ts`, server `serve/ws.rs` |

## Phase L1: shared core (Rust only)
Goal: one source of truth for "what is set up and what is next".
1. `crates/automemory-core/src/setup.rs` (register in `lib.rs`): `pub struct Step { id, title, done: bool, why, command, link }`;
   `pub fn checklist(db, home, cwd) -> Vec<Step>` for: agent connected, instruction files present, password set, sync
   remote set, plus the five learning moments (connected, remembered a note, decided, saw a lock block, reviewed a
   suggestion; derive from DB rows and `onboarding` state). `pub fn next_step(&[Step]) -> Option<&Step>`. Unit tests with an in-memory DB.
2. `crates/automemory-core/src/onboarding.rs`: progress in `sync_kv` key `onboarding` (JSON: completed step ids,
   dismissed flags, demo_loaded, tour_step). Functions `load/save/mark_done/dismiss/reset`. Unit tests.
3. CLI `automemory next` (new `commands/learn.rs`, enum entry + match arm in `main.rs`): prints the best next command with
   a one-line reason; exit 0. Extend `doctor` with a "Getting started" section using the same checklist.
4. REST `GET /api/setup` (checklist) and `GET/POST /api/onboarding` (progress; POST human-gated), registered in
   `serve/routes/mod.rs`; new file `serve/routes/setup.rs`.
5. Test `tests/scenarios/onboarding.py` (first part): `next` after each setup action; `/api/setup` reflects a removed remote.
Done when: `cargo test --workspace` and `tests/run_all.sh` are green, checklist output matches reality in the scenario. Commit.

## Phase L2: CLI tour, learn topics, demo
1. `automemory tour` in `commands/learn.rs`: 8 steps exactly as in ui-ux.md 13.3. Interactive (stdin) with `--auto`
   (no prompts, prints transcript), `--resume`, `--reset`, `--step N`, `--no-color`. Non-TTY stdin => behave as `--auto`.
   Runs in a temp HOME + temp git repo it creates and deletes; spawns `std::env::current_exe()` with `HOME` set for the
   sandbox. Each step: explain (<= 2 lines), run or ask the user to type the command (accept near matches, "Did you
   mean..." on typos), show real output, verify state changed, print `done` or a hint. Step 5 simulates two agents with the
   payloads from `tests/contract/` so the block message is the real one. Step 8 (connect real agent) is opt-in and
   prints exactly which files change and how to undo (`automemory disconnect`).
2. `automemory learn [topic]`: topics `memory decisions locks handoff sync modes export-rules embeddings`, 10 lines each,
   copy-paste commands; no topic lists topics. Text lives in one `const` table (also exported for the dashboard help drawer later).
3. `automemory demo [--remove]`: seeds a sample project (3 decisions, 5 notes, 1 pending suggestion) tagged
   `metadata.demo = true`; excluded from sync deltas (`sync/deltas.rs` collect), `rules_export` view, and recall;
   `--remove` leaves zero rows. Add tests.
4. Polish: every user-facing error in `learn/connect/project` ends with `Try: ...`; group `--help` into Daily/Setup/Advanced
   (clap `help_heading`); add `Examples:` to the top 8 commands; first-run one-line hint (once, via `sync_kv`
   `tour_hint_shown`); `automemory completions <shell>` using `clap_complete` (add dependency to the workspace).
5. Tests: `tests/scenarios/tour.py` (runs `tour --auto`, asserts each step's state change, real `HOME` untouched,
   `--reset/--resume`); register both new scenarios in `tests/run_all.sh` and the table in `tests/README.md`.
Done when: all tests green; `automemory tour --auto` transcript reads cleanly; commit.

## Phase L3: dashboard onboarding (touches `clients/web`; check `git status` first)
1. `utils/api.ts`/`stores/automemory.ts`: replace the guessed `setupChecklist` getter with data from `GET /api/setup`; progress via `/api/onboarding`.
   (Known bug: today sync/password/instruction-files are marked done from unrelated data.) Live tick: when a WebSocket
   `setup_changed` event arrives (emit it from the server when a step completes), refetch `/api/setup`.
2. Components: `WelcomeCard.vue` (3 paths: tour / sample data / skip), `CoachTour.vue` + `useCoachTour.ts` (6 stops from
   ui-ux.md 13.4, spotlight + popover, Enter/Esc/arrows, focus trap, `aria-live`, reduced motion), `HelpDrawer.vue`
   (glossary, shortcuts, replay tour, learn topics), `DemoBanner.vue`. Use existing `Modal`/`Drawer`/`Button`/`CopyCommand`; theme tokens only (no raw colours).
3. Demo mode in the UI: `POST/DELETE /api/demo` (human-gated), "Sample" badge on demo items, one-click removal.
4. `CopyCommand` gets an optional "Run it for me" for safe commands (`note`, `export-rules`, `sync now`) calling existing routes.
5. Verify in the in-app browser at 360/768/1280, light and dark; fill `tests/manual/UI_CHECKLIST.md` additions.
Done when: finishing a step in a terminal ticks the open dashboard within 2 s; demo removal leaves no trace; keyboard-only tour works; both themes readable. Rebuild assets, commit.

## Phase L4: agent notice and docs
1. In `hook.rs` `resume_once` path: a one-time 3-line notice per project for a newly connected agent
   ("AutoMemory is active... `automemory resume`... `automemory tour`"), keyed in `sync_kv`; must not break the Claude/Codex/agy output formats
   (see `native_context`, `agy_injection`) and must stay within the existing context budget; contract test added.
2. Rewrite `memory.about` text (`crates/automemory-mcp/src/tools.rs`) to use the glossary words from ui-ux.md section 6.
3. Update `docs/FEATURES.md` (onboarding row), `docs/TASKS.md` (tick items), `docs/PLAN.md` (status), tool and test counts.
4. Final: `tests/run_all.sh` fully green, reinstall the binary, commit.

## Definition of done (whole feature)
- A new user can reach all five learning moments from either surface without reading docs.
- Nothing in the tour or demo touches real data unless the user explicitly connects an agent (step 8) or keeps demo data off.
- No telemetry leaves the machine; progress is local (`sync_kv`).
- Every phase committed separately; docs and counts updated; `tests/run_all.sh` green.

## Report back
When finished (or blocked), summarize per phase: what was built, test output, anything skipped, and any decision that
needs the user (for example whether `connect` should offer to start the tour; proposed default: yes, once).
