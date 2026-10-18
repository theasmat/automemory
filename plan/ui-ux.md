# AutoMemory dashboard: UI/UX redesign spec

Status: plan, nothing built. Scope: `clients/web` (Nuxt 3, Tailwind, pinia) and the few REST routes it needs
(`crates/automemory-cli/src/commands/serve/routes/`). Written Oct 2026 from an audit of the current code.

## 1. Why, and for whom

Today the dashboard shows AutoMemory's internals. A person opening it wants four answers:

1. **Is it working?** Which agents are connected, is sync running, is anything broken.
2. **What do my agents know?** The decisions, conventions and gotchas the next session will start from.
3. **What needs me?** Proposals to approve, a collision to resolve, a password to set.
4. **Can I tell them something?** Add or correct knowledge in plain words.

Three users, in this order of importance:

| User | Arrives wanting | Must be able to |
|---|---|---|
| First-time | "Did setup work?" | See connected agents and a 4-step checklist; finish setup without reading docs |
| Daily | "What changed, what needs me?" | Approve/reject, read and edit the project brain, add a note in under 10 s |
| Operator | "Why did that get blocked? Is sync healthy?" | See locks and who holds them, switch mode, inspect sync and devices |

**Success measures** (check by hand at the end of each phase): a new user sees the state of their setup within
30 s of opening `/`; no primary screen uses a term from the jargon list in section 6; every empty state names the
next action; every write gives visible success or an explained failure; the whole app is usable at 360 px wide.

## 2. Audit of today's UI

What exists (13 pages, ~3200 lines): fixed 256 px sidebar with 12 equal links, a status bar (WebSocket light, project
selector, mode switch, Sync Vault, refresh), then one page per data type. Only two shared components exist
(`Sidebar.vue`, `StatusBar.vue`); every card, badge, modal and empty state is a copy-pasted class string.

| Area | Finding | Evidence |
|---|---|---|
| Mental model | Pages mirror database tables (memories, traces, failures, skills, replay), not tasks | 12-link sidebar |
| Jargon | "Cognitive Memories", "Hot Memory Constitution", "Crystallized", "100-to-1 trajectory distillation", "BM25 context assembly", "Sub-ms Rust" as status, "Fleet Quota", "Deltas", leases/batons/rooms | page headers and copy |
| Fake values | "~98.5% saved" literal, proposal confidence defaults to 95, "1-Click Connected" and "Live Subscriptions Active" always shown, WS URL hard-coded, `dev_local_machine` fallback | `overview.vue`, `proposals.vue`, `sync.vue`, store |
| Errors | `request()` swallows failures and returns null or `[]`, so an outage looks like "No memories yet"; Sync Vault says nothing on failure | `utils/api.ts` |
| Empty states | Plain text, no next step | most pages |
| First run | Marketing copy, four mostly-zero stat cards, one `connect agy` command; nothing says which agents are connected or what they know | `overview.vue` |
| Mobile | Fixed `w-64` sidebar, `h-screen overflow-hidden`, `p-8` everywhere, the mode switch is `hidden md:flex` (cannot change mode on a phone); unusable below ~900 px | `app.vue`, `StatusBar.vue` |
| Accessibility | ~14 aria/role/focus hits in total; icon buttons rely on `title`; toast has no `role="status"`; no skip link, no visible focus ring, hand-rolled modals without focus trap or Escape; 151 hard-coded 10-11 px sizes with gray-500/600 on black | grep |
| Typography | 190 `font-mono` uses: paragraphs and labels are monospace; uppercase tracking-wider everywhere | pages |
| Theme | Palette in `tailwind.config.js` is not enforced (pages use stock emerald/amber/red); square corners, pulse animations, emoji mixed with lucide icons | pages |
| Dialogs | Native `confirm()` for archive and guardrail delete; archive text says "injected into active LLM context"; no undo | `memories.vue`, `guardrails.vue` |
| Password | Unmasked `window.prompt()`, wrong password cleared silently, caller only sees `ok`; `sync/trigger` and `setMode` bypass it | `humanFetch` in `utils/api.ts` |
| Live updates | Toast overwrites the previous one, no history, not announced; coordination polls every 5 s while others refetch on WS messages | `useWsPulse`, `coordination.vue` |
| Data not surfaced | `/api/context` (what agents will be told), `/api/events` and `/api/stream` (activity), `/api/graph`, `/api/preferences` have no UI | routes list |

## 3. Information architecture

Six task areas replace twelve table pages. The advanced pages stay reachable but leave the main navigation.

```
Main nav (6)                      Advanced (collapsed "More")
  Home                              Session replay      Success traces
  Project brain                     Failures            Skills
  Agents & locks                    Documents           Constitution
  Inbox            (badge)          Knowledge graph     Recall settings
  Sync & devices
  Safety
```

| Old page | New home | Notes |
|---|---|---|
| Overview | **Home** | Rebuilt around health + "needs me" |
| Memories | **Project brain** | Plus decisions and preferences in one view |
| Review Proposals | **Inbox** | Same data, task wording |
| Agents & Locks (coordination) | **Agents & locks** | Humanised, agent names, relative times |
| Cloud Sync | **Sync & devices** | Real status only; the fake fleet/quota block is removed |
| Safety Guardrails + mode switch | **Safety** | Mode moves here from the status bar (still shown as a pill) |
| Product Specs, Constitution, Procedural Skills, Success Traces, Failures & Gotchas, Session Replay | **More** | Renamed: Documents, Rules of the project, Learned skills, What worked, Problems & fixes, Session history |

Global chrome: a slim top bar (project picker, connection pill, command palette button, theme toggle) and a left nav rail
(icons + labels on desktop, icon-only at 768-1279, bottom tab bar under 768).

## 4. Screens

Conventions in every wireframe: `[ ]` button, `( )` toggle, `…` truncated, `●` live. Every screen has the same
four states: **loading** (skeleton shaped like the content), **empty** (one sentence + the next action),
**error** (what failed + retry + the command to run if the daemon is down), **success** (inline, plus a toast with Undo
where reversible).

### 4.1 Home

Answers "is it working" and "what needs me" in one screen.

```
┌ shop ▾ ─────────────────────── ● Connected · Light/Dark · ⌘K ┐
│ Good afternoon. 3 agents are working on shop.               │
│                                                             │
│ ┌ Needs you ────────────────┐ ┌ Agents ─────────────────────┐│
│ │ 2 suggestions to review   │ │ ● Claude Code  editing      ││
│ │ [Review now]              │ │   src/checkout.rs · 3 min   ││
│ │ 1 file lock held 40 min   │ │ ● Cursor       idle         ││
│ │ [See who]                 │ │ ○ Codex        not connected││
│ └───────────────────────────┘ │ [Connect an agent]          ││
│ ┌ Setup (3 of 4) ───────────┐ └─────────────────────────────┘│
│ │ ✓ Agent connected         │ ┌ What agents know ───────────┐│
│ │ ✓ Instruction files       │ │ 6 decisions · 14 notes      ││
│ │ ✓ Password set            │ │ "Use Stripe webhooks…"      ││
│ │ ☐ Sync to another machine │ │ [Open Project brain]        ││
│ │   [Set up sync]           │ └─────────────────────────────┘│
│ └───────────────────────────┘ ┌ Recent activity ● ──────────┐│
│                               │ 14:02 Claude recorded …     ││
│                               └─────────────────────────────┘│
└─────────────────────────────────────────────────────────────┘
```

- **Needs you** card is the only place with attention colour. Hidden when empty ("All clear").
- **Agents** from `/api/coordination` (leases grouped by agent) and a new `/api/agents` (section 9). "Connected" means the
  agent's config is registered, "active" means an event in the last 10 min.
- **Setup checklist** disappears once all four items are done. Each item deep-links to the screen that completes it, and
  shows the CLI equivalent (copy button).
- **What agents know** uses `/api/context` as the preview of what a new session is told.
- Replaces the literal "~98.5% saved", the architecture banner and the always-on "Connected" claims. Token savings only
  appear if `/api/stats` returns a real number, labelled "estimated".

### 4.2 Project brain

The place a person reads and edits what every agent starts from.

```
Project brain · shop                          [+ Add]  Search…
Tabs: All · Decisions · Conventions · Gotchas · In flight
┌─────────────────────────────────────────────────────────────┐
│ DECISION  Payments                         you · 2 d ago    │
│ Use Stripe webhooks, not polling.                           │
│ Why: polling missed events during outages.                  │
│ Seen by Claude · Cursor            [Edit] [Pin] [Archive ⋯] │
└─────────────────────────────────────────────────────────────┘
        ▸ Preview: what the next agent is told  (opens drawer)
```

- Cards, not table rows. Source badge says who wrote it: **You**, **Claude Code**, **Cursor**… Agent-written items that
  are not yet approved sit in Inbox and show "Waiting for review" here (greyed).
- **+ Add** is a one-field modal ("Tell every agent…") with an optional "More detail" and a type chooser in plain words:
  Fact, Preference, Lesson, Warning, Pattern (labels: "Something true", "How we like it", "What we learned", "Watch out",
  "A way of doing it"). Saves trusted at once (already supported by `POST /api/memories`).
- **Edit** inline in the card (no separate modal); **Archive** shows an Undo toast for 8 s instead of `confirm()`.
- **Pin** keeps an item in the exported instruction-file block even when 12+ items compete (new `pinned` metadata flag).
- **Preview drawer** shows the exact block `export-rules` writes (from a new `/api/rules-preview`) and a button
  "Update AGENTS.md, CLAUDE.md…" that runs the export (new `POST /api/rules-export`), listing which files changed.
- Search is a single box (no "FTS5 BM25" wording); filter chips for source and age. Token cost is moved into the
  preview drawer ("This is about 640 tokens per session").
- Empty: "Nothing here yet. Add the first thing every agent should know, for example how to run the tests. [+ Add]".

### 4.3 Agents & locks

```
Agents & locks                                  ● live
┌ Who is working ─────────────────────────────────────────────┐
│ Claude Code   src/checkout.rs, src/webhook.rs   since 14:02 │
│ Cursor        waiting for src/checkout.rs  ⚠ blocked by Claude│
└─────────────────────────────────────────────────────────────┘
┌ File locks ──────────────────────────────────────────────────┐
│ File               Held by        Since     Goal     [Release]│
└─────────────────────────────────────────────────────────────┘
How locks work (collapsed explainer: 3 sentences, link to docs)
Hand-offs: last 5 notes passed between sessions, with "Resume" copy button
```

- Humanised: "file lock" not lease, "hand-off" not baton, "planning room" only under Advanced. Relative times
  ("40 min ago") with the absolute time on hover.
- Idle-lock alerts become a banner with **Release** (needs the human password, same flow as Safety).
- Blocked agents shown in amber with the holder's name, so a collision is understandable without reading logs.
- Coverage metric from `automemory locks --coverage` appears as a small "Locks cover 86% of edits this week" card (new
  route, section 9).
- Uses SSE/WebSocket events instead of 5 s polling; falls back to polling when the socket is down.

### 4.4 Inbox

One queue of things that need a person: proposed memories, idle locks, a failed sync. Proposals reuse `/api/proposals`.

- Left list with type icon and age; right pane shows the item. For a proposed memory: "Claude suggests we remember…",
  the text, **why Claude thinks so** (source events, confidence only if real), and Approve / Edit then approve / Reject.
- Keyboard: `J/K` move, `A` approve, `R` reject, `E` edit, `Shift+A` approve all of one type. Shown in a `?` help sheet.
- Bulk bar appears when more than 3 items; shows counts per type from `/api/proposals/summary`.
- Rejecting asks for an optional one-line reason (stored; improves later extraction).
- Empty: "Inbox zero. Agents will suggest things here when they learn something worth keeping."
- Nav badge shows the pending count; no count when zero.

### 4.5 Sync & devices

A three-step wizard when no remote is set, a status page after.

```
Step 1 Create a private git repo (any host)     [copy: git init --bare …]
Step 2 Connect this machine                      [copy: automemory sync remote <url>]
Step 3 On your other computer                    [copy: automemory sync bootstrap <url>]
```

- After setup: last sync time and result, "Sync now" with a visible result (pulled N, pushed M, or the error), encryption
  status ("Encrypted: the git host sees only ciphertext"), device list (name, last seen, project count), recovery phrase
  reminder (never displayed after first creation).
- The "Fleet / Quota / mock cloud" blocks are removed until the SaaS exists.
- Resume pack preview ("This is what a new session on your laptop would be told") from `GET /api/resume` (new).

### 4.6 Safety

- **Mode** as a three-option card group with consequences spelled out: *Relaxed* (agents are warned, nothing is blocked),
  *Balanced* (default; risky commands and clashing edits are blocked), *Strict* (every edit needs a file lock; unlisted
  commands are blocked). Maps to the existing vibe/review/strict modes.
- **Password**: set/change flow with strength hint; explains what it protects (approvals, mode, rules).
- **Rules**: table of guardrail rules with a plain-language description column, a "Test a command" box with the verdict
  and reason, add-rule form with examples. Delete uses Undo toast.
- Recent blocks list ("Blocked: git push --force, 2 h ago, Cursor") from `/api/events`.

### 4.7 More (advanced pages)

Kept as is functionally; each gets the new shell, renamed titles, empty-state actions and the shared components. Add
**Recall settings** (keyword vs hybrid, provider, indexed count: a form over `automemory embeddings`) and a
**Knowledge graph** page over the existing `/api/graph`.

### 4.8 Command palette (⌘K / Ctrl-K)

Search across memories and pages; actions: "Add a note…", "Review inbox", "Sync now", "Export instruction files",
"Switch mode…", "Go to…". Recent items first. Everything the palette does is also reachable by mouse.

## 5. Design system

Keep the dark + gold identity, fix its execution.

**Principles**: calm over dramatic (remove pulse/ping decoration except the live dot); one accent for "act on this";
sans for reading, mono only for code, ids and commands; sentence case, no forced uppercase; 4.5:1 contrast minimum.

### 5.1 Tokens (CSS variables on `:root`, mapped into `tailwind.config.js`)

| Token | Dark | Light | Use |
|---|---|---|---|
| `--bg` | #0b0c0e | #f7f7f5 | page |
| `--surface` | #14161a | #ffffff | cards |
| `--surface-2` | #1c1f24 | #f0efeb | nested, hover |
| `--border` | #2a2e35 | #dcdad3 | 1 px lines |
| `--text` | #e8e9ec | #17181a | body (contrast ≥ 12:1) |
| `--text-muted` | #9aa1ab | #5d6168 | secondary (≥ 4.5:1) |
| `--accent` | #ffc000 | #8a6100 | primary action, focus |
| `--accent-fg` | #1a1400 | #ffffff | text on accent |
| `--ok` / `--warn` / `--bad` / `--info` | #34d399 / #fbbf24 / #f87171 / #38bdf8 | darker equivalents | status, always paired with an icon and text |

Theme follows `prefers-color-scheme`, with a manual toggle stored in `localStorage` (try/catch). Existing
`dark-*`/`gold-*` colours stay as aliases during migration, then are deleted.

### 5.2 Type, space, shape

- Font: Inter (already configured) for UI; JetBrains Mono for code. Scale: 12 / 14 (body) / 16 / 20 / 24 / 32. No text
  below 12 px; the 150+ `text-[10px]/[11px]` uses are replaced.
- Spacing on a 4 px grid (4, 8, 12, 16, 24, 32). Page padding `16` on mobile, `24` tablet, `32` desktop; content max width
  1200 px.
- Radius 8 px cards and inputs, 999 px pills; borders 1 px; one shadow level for popovers.
- Icons: lucide only (remove emoji). Icon-only buttons always have an `aria-label` and tooltip.
- Motion: 150 ms ease for hover/expand; skeleton shimmer; everything off under `prefers-reduced-motion`.

### 5.3 Components to extract (`clients/web/components/`)

`AppShell`, `NavRail`, `TopBar`, `PageHeader`, `Card`, `StatCard`, `Badge` (variants ok/warn/bad/info/neutral),
`AgentChip` (icon + name + status dot), `SourceBadge`, `MemoryCard`, `EmptyState` (icon, sentence, action),
`ErrorState` (message, retry, command to run), `Skeleton`, `Button` (primary/secondary/ghost/danger, loading state),
`IconButton`, `Modal` and `Drawer` (focus trap, Escape, return focus), `Toast` + `useToast` (queue, `role="status"`,
Undo action), `CopyCommand` (monospace block + copy button), `Tabs`, `SearchBox`, `CommandPalette`,
`Checklist`, `ShortcutHelp`. Rule: no page may define its own card/badge/modal class strings.

## 6. Copy and interaction guide

**Voice**: plain, second person, present tense, no hype. Say what happened and what to do next.

| Today | Use instead |
|---|---|
| Cognitive Memories | Project brain |
| Review Proposals / Pending proposals | Inbox / Suggestions to review |
| Hot Memory Constitution, Active Constitution Payload | Project rules |
| Crystallized | Saved |
| Procedural Skills | Learned skills |
| Success Traces | What worked |
| Failures & Gotchas | Problems and fixes |
| Session Replay | Session history |
| Lease / acquire / release | File lock / lock / unlock |
| Baton | Hand-off note |
| Planning Room | Agent discussion (Advanced) |
| FTS5 BM25 search | Search |
| Sub-ms Rust (as a status) | Running (and the version in a tooltip) |
| Deltas, vault | Changes, sync repository |
| Vibe / Review / Strict | Relaxed / Balanced / Strict |
| "injected into active LLM context" | "Agents will stop using this" |

**Patterns**
- *Destructive actions*: do immediately, show "Archived. Undo" for 8 s. Only irreversible actions (delete a rule
  permanently, release another agent's lock) get a confirm dialog, written with the consequence in the title
  ("Release Claude's lock on checkout.rs?").
- *Password*: a proper masked modal (not `window.prompt`) opened by any 403 about the password, with "Wrong password,
  try again" inline; remembered for the tab; every write path (`sync/trigger`, `setMode`, add memory) goes through
  `humanFetch`. The modal says what is being approved.
- *Errors*: never an empty list for a failure. `request()` returns `{ok, data, error}`; pages render `ErrorState`; if the
  daemon is unreachable, show "AutoMemory isn't running" with `automemory serve` copy button.
- *Toasts*: queue up to 3, auto-dismiss 5 s, errors persist until dismissed, all announced via `aria-live`.
- *Keyboard*: `g h/b/a/i/s` jump (vim-style, shown in `?`), `/` focus search, `n` new note, `⌘K` palette, `Esc` close.
- *Numbers*: show only measured values; estimates are labelled "estimated"; zero shows "None yet", not "0 Active".

## 7. Live behaviour

- One shared connection (`useWsPulse`) feeds a store `events` stream; pages subscribe to what they show. Reconnect with
  backoff (1, 2, 4 … 30 s) and a "Reconnecting…" pill in the top bar; data stays visible, marked stale after 30 s.
- Updates change the item in place with a brief highlight (no list jump); new Inbox items increment the badge and add an
  entry to the activity feed rather than a toast every time.
- The activity feed (Home, and a drawer from the top bar) shows the last 50 events in sentences: "Claude recorded a
  decision: Payments", "Cursor was blocked on checkout.rs", "Sync pushed 3 changes".
- WebSocket URL derived from `location` (works behind any port), not hard-coded.

## 8. Accessibility and responsive

**WCAG 2.2 AA checklist (acceptance for every phase)**
- Contrast ≥ 4.5:1 text, 3:1 UI (verified for both themes); status never by colour alone.
- Visible 2 px focus ring on everything focusable; logical tab order; skip-to-content link; landmarks (`nav`, `main`).
- Modals/drawers trap focus, close on Escape, restore focus; palette is a combobox with correct roles.
- Every control has an accessible name; form fields have labels and error text linked with `aria-describedby`.
- Live regions for toasts and connection changes; reduced-motion respected; targets ≥ 24 px (44 px on touch).
- Works at 200% zoom and in forced-colors mode without losing content.

**Breakpoints**: 360 (single column, bottom tab bar with 5 items + More), 768 (icon rail, two columns), 1280 (full rail,
three columns on Home). Tables become stacked cards under 768. No fixed `h-screen overflow-hidden`; the page scrolls.
The mode, project picker and sync actions are reachable on mobile (top bar overflow menu).

## 9. Backend needs

Existing routes cover most screens; new ones are small and read-only except two actions.

| Screen | Existing | New |
|---|---|---|
| Home | `/api/stats`, `/api/coordination`, `/api/context`, `/api/proposals/summary`, `/api/events`, `/api/stream` | `GET /api/agents` (connected agents from the connect inventory + last event time + current locks); `GET /api/setup` (checklist: agent connected, instruction files present, password set, remote set) |
| Project brain | `/api/memories` (GET, POST add, update, archive), `/api/preferences` | `GET /api/decisions`; `pinned` flag on update; `GET /api/rules-preview` (the export block); `POST /api/rules-export` (human-gated) |
| Agents & locks | `/api/coordination` | `POST /api/locks/release` (human-gated); `GET /api/locks/coverage` (from `lock_coverage`); `seen_by` counters on memories (record agent when recalled/injected) |
| Inbox | `/api/proposals*` | reject reason field |
| Sync | `/api/sync/status`, `/api/sync/trigger` | `GET /api/resume`; sync result returned synchronously (pulled/pushed/error) instead of "scheduled"; device list from `sync_kv`/deltas |
| Safety | `/api/mode`, `/api/guardrails*` | `GET /api/password` (is set: bool), `POST /api/password` (set/change); blocks list via `/api/events` filter |
| More | `/api/traces`, `/api/failures`, `/api/skills`, `/api/sessions*`, `/api/docs`, `/api/graph`, `/api/constitution` | `GET/POST /api/recall` (mode + provider + indexed count) |

All new writes pass the existing origin/host guard and `require_human_header`. All new reads accept `project_id`.

## 10. Phased build plan

Each phase ships on its own, keeps the old pages reachable until replaced, and ends with `pnpm run generate`, copying
`dist/*` into `crates/automemory-cli/src/dashboard/assets/`, `tests/run_all.sh` green, and the manual checklist below.

| Phase | Scope | Main files | Done when |
|---|---|---|---|
| **P1 Foundation + Home** (about 3-4 days) | Tokens + light/dark, base components (section 5.3 first half), AppShell/NavRail/TopBar, new nav with 6 areas + More, `request()` error model, toast queue, masked password modal, new Home with setup checklist and agents card | `tailwind.config.js`, `assets/css`, `components/*`, `app.vue`, `pages/index.vue`, `utils/api.ts`, `stores/automemory.ts`, routes `agents.rs`, `setup.rs` | New user sees real setup state in < 30 s; a stopped daemon shows the "isn't running" state; works at 360 px; axe/Lighthouse a11y ≥ 95 on Home |
| **P2 Project brain** (3 days) | MemoryCard list, tabs, inline edit, Undo archive, pin, preview drawer, export action, decisions | `pages/brain.vue`, `MemoryCard`, `Drawer`, routes `decisions.rs`, `rules.rs` | A note is added in < 10 s; preview equals `export-rules` output byte for byte (test) |
| **P3 Agents & locks + Inbox** (3 days) | Humanised coordination page, release action, coverage card, Inbox with keyboard flow and bulk bar | `pages/agents.vue`, `pages/inbox.vue`, `locks.rs` | Collision scenario from `tests/scenarios/collision.py` is understandable from the page alone |
| **P4 Sync wizard + Safety** (2-3 days) | Three-step wizard, sync result, devices, resume preview; mode cards, password flow, rules table | `pages/sync.vue`, `pages/safety.vue`, routes `password.rs`, `resume.rs` | Setting up a remote and a second machine can be done from the page; mode change works on mobile |
| **P5 Polish** (2 days) | Command palette, shortcut help, activity feed drawer, More-section reskin, Recall settings, graph page, a11y audit fixes, remove old pages and dead classes | `CommandPalette`, `pages/more/*`, `pages/recall.vue` | WCAG checklist (section 8) passes on every page in both themes; no `font-mono` outside code; no `confirm()` left |

**Tests**: extend `tests/scenarios/dashboard.py` for each new route (shape, human gate, project filter); add
`tests/manual/UI_CHECKLIST.md` (first-run, collision, approve flow, offline daemon, 360 px, keyboard-only, screen reader
spot check, both themes). A headless screenshot pass with the in-app browser at 360/768/1280 is run before each merge.

## 11. Risks and open questions

- **Scope**: this is the largest piece of front-end work so far (~13-15 working days). P1 + P2 alone deliver most of the
  value; stop after P2 if time is short.
- **Asset size**: the binary embeds `dist/`; keep the bundle lean (no new UI framework; icons tree-shaken). Check
  `automemory` binary growth after each phase (currently ~10 MB).
- **Agent identity**: "seen by" and "connected agents" depend on reliable agent names in events; hook payloads for
  non-hook agents give none, so those agents show as "connected (no activity data)".
- **Password model**: one shared human password gates everything; per-action approval or OS keychain could be better
  later. Not in scope.
- **Light theme** doubles visual testing; if cut, keep tokens so it can be added later.
- Open: brand name for the Project brain area ("Project brain" vs "Knowledge")? Whether Inbox should also surface failed
  syncs and blocked commands (proposed: yes, as read-only items).
