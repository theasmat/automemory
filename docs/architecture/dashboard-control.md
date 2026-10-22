# Dashboard control and permission tiers

**Status: built (U1–U5).** Tiers live in `serve/policy.rs` (`GET /api/policy`, route audit test), jobs in `serve/jobs.rs` (`/api/jobs`, Jobs tray), actions in `routes/control.rs`, pages Tools / Agents / Brain / Settings. Command logic is reused via subprocess (`run_self`) instead of a separate `ops/` module. Not built: lock icons from `/api/policy`, native Cohere/Voyage/Gemini adapters (E5).

Goal: anything a person does in the terminal that is safe can be done (and watched) in the dashboard: start or stop indexing, sync, export instruction files,
run a health check, connect an agent, record a decision. The terminal stays the way to do the few things that must not be reachable from a web page.

### 1 Rules
1. **One implementation.** Command bodies move into `automemory-core/src/ops/` (functions returning data, no printing). The CLI prints them, REST returns them as JSON. No
   logic lives only in a command or only in a route.
2. **Three tiers, one table** (`automemory-core/src/policy.rs`: action name to tier, used by every route and shown in the UI as a lock icon):
   - **Safe**: read-only, or derived from verified data, local and undoable. No password.
   - **Protected**: creates trusted knowledge, changes what agents may do, or sends data off the machine. Human password every time, never stored (section 10 gate).
   - **Terminal only**: never reachable over HTTP.
3. **Decided by effect, not by name.** Indexing with a local model is Safe; the same button with a cloud profile is Protected. Sync pull is Safe; push to a remote is Protected.
4. **Why "safe" is narrow.** The dashboard API listens on localhost, so an agent with a shell can call any Safe endpoint. A Safe endpoint must therefore be harmless in an agent's hands:
   notes, decisions, pins, approvals and mode changes create trust or loosen safety, so they stay Protected.
5. **Long work is a job.** One job runner (`/api/jobs`, WebSocket `job_progress`, cancel) replaces the one-off indexing state; a Jobs tray in the top bar shows what is running.
6. **Preview before writing.** Anything that writes files shows the exact change first (export-rules diff, connect file list), then applies.
7. **Everything is traced.** Each UI action writes an `audit_log` row (action, tier, result); a "Recent activity" view shows it next to the blocks agents hit.

### 2 What goes where
| Operation | Tier | Dashboard home |
|---|---|---|
| Health check (`doctor`), hook dry run, guardrail evaluate, overlap check, resume preview, recently blocked, audit view | Safe | Settings > Health, Safety, Agents |
| Index memories: start, stop, re-index, clean (local model) | Safe | Recall settings (done, today Protected for all models) |
| Index with a cloud model, store or remove an API key, add or remove a model profile | Protected | Recall settings (done) |
| Export instruction files (preview, apply, drift check), write a hand-off now | Safe | Project brain > Preview export; Agents > Hand-off |
| Rescan and compile docs, index the code graph, run eval suites, backup the database now | Safe (jobs) | Settings > Tools |
| Sync: status, pull | Safe | Sync |
| Sync: push, set or change the remote, bootstrap another machine | Protected | Sync |
| Add note, record or supersede a decision, edit memory, pin, veto, revoke, restore | Protected | Project brain, new Decisions view |
| Approve or reject suggestions | Protected | Inbox (done) |
| Connect or disconnect an agent in this project (shows the files first) | Protected | Agents > Set up an agent |
| Release a lock, kill switch (release every lock) | Protected | Agents & locks |
| Safety mode, guardrail rules, lock settings (ignore patterns, idle time) | Protected | Safety (done), Settings > Locks |
| Set or change the human password, show the recovery phrase, `disconnect --everywhere`, `init`, `serve`, `hook`, `mcp`, export of keys | Terminal only | n/a (the page explains the command) |

### 3 Phases
| Phase | Scope | Done when |
|---|---|---|
| U1 | `policy.rs` tier table, route audit test (every POST/PUT/DELETE route must have a tier), job runner + `/api/jobs` + Jobs tray, move indexing onto it, effect-based gating (local index is Safe) | The audit test fails when a route is added without a tier; indexing progress appears in the tray from any page |
| U2 | `ops/` extraction for doctor, export-rules, handoff, sync, docs rescan, graph index, eval, backup; Settings > Tools and Health cards | Each card runs the same code as the CLI and shows its result; scenario compares CLI and REST output |
| U3 | Agents > Set up an agent (list adapters, status here, file preview, connect, disconnect) | Connect then disconnect leaves no registration (`disconnect --dry-run` empty); preview equals what is written |
| U4 | Decisions view (record, supersede, history), memory restore/pin/veto/revoke in Project brain | Every action asks for the password; each is undoable or logged |
| U5 | Settings > Locks and Data (lock settings, backup now, export, archive data), activity log view | Config changes persist and apply to the next hook run |

Tests: a policy-table unit test; the route audit; a scenario per tier (Safe works without a password, Protected is refused without one and throttled after five wrong tries,
Terminal-only paths return 404); a manual checklist item per new card.

Decisions to confirm: (a) notes and decisions stay Protected (the password is asked every time, as you chose); (b) the kill switch stays Protected so an agent cannot call it; (c) local
indexing needs no password.
