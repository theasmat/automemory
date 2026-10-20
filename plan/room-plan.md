# Planning rooms: spec

Status: R1 built (Oct 2026); the rest is a plan. The summary and phases are in [docs/PLAN.md §14](../docs/PLAN.md); this file gives the detail
another agent needs to build it. Tasks R1–R5 are in [docs/TASKS.md](../docs/TASKS.md).

## 1. Where we are

Built (commit `16ff6e9`):
- A room is a turn-gated state machine: **propose → critique → revise → decided**, one round, posts need evidence ids
  (memory, trace or failure) and are capped at 2000 characters (`crates/automemory-core/src/db/queries/rooms.rs`).
- MCP tools `room.join`, `room.pending`, `room.post` and `room.status` (`crates/automemory-mcp/src/tools.rs`).
- Human verdict on a decided room: **approve** records one decision (the last critique as dissent, agent `room`) and one
  task per `- step` / `1. step` line of the final revision; **reject** stores nothing; **another round** reopens the
  critique. The verdict is kept in `sync_kv` as `room_outcome:<id>`, so a second verdict is refused.
- `automemory room list|show|approve|reject|again`, `/api/rooms*`, a dashboard **Planning rooms** page, a Home banner
  when a room waits for you, and a rules paragraph that tells agents how rooms work.
- Tests: `rooms.rs` unit tests, `tests/scenarios/planning_room.py` (two simulated agents and the human),
  `tests/ui/smoke.py` (page renders, approve asks the password). Live trial **T11** is written, not run.

What hurts today:
1. Only an agent can open a room, and you must paste the room id into the other agent by hand.
2. You can judge the plan, but not shape it: no way to add a constraint mid-discussion or fix one step before approving.
3. After approval, the plan disappears into a decision and a flat task list. Nobody sees progress, and changing the plan
   later means starting from nothing.
4. One fixed round and two agents. A third agent cannot review, a stalled agent blocks the room forever.

## 2. Goals and non-goals

Goals:
- You can **start** a room in the dashboard and the invited agents **find out by themselves** on their next prompt.
- You can **steer** (notes) and **correct** (edit the plan) before anything is stored.
- An approved plan becomes **tracked work** with owners and checks, and can be **revised** without losing history.
- Rooms stay cheap: turns, a round limit, short posts, evidence for claims.

Non-goals (deliberately):
- Free chat between agents, votes, or more than three rounds. They burn tokens and blur who decided.
- Agents approving plans. Only the human verdict (password) stores decisions and tasks.
- Rooms that run code or edit files. Agents do the work afterwards through tasks, under the normal hooks and locks.

## 3. User flows

### 3.1 Start a room (R1)
The question is required (trial T11b: a room called just "cart" made an agent invent an HTTP API). For a human-made room that has none (older ones), the hook line says "No question was given … state your assumptions".
1. Dashboard → Planning rooms → **New room**.
2. Fill in: topic (short), the question to answer, agents to invite (from Agents: connected ones are listed), and
   optionally context: notes found by search (they become starting evidence), files or symbols in scope, constraints
   ("no new services", "must ship this week").
3. **Create** asks for the password (it creates work for agents) and opens the room page. The first invited agent holds
   the turn for the proposal.
4. On that agent's next prompt or session start, the hook adds one line to its context:
   `Planning room "Cart storage" (room-…) waits for your proposal. Question: … Read it with room.status, answer with room.post.`
5. The other invited agents get the same line when it is their turn (critique, revise).

### 3.2 Steer and correct (R2)
- **Note**: a text box on the room page. A note does not take a turn. The next agent sees it in `room.status` as
  `{"author": "human", "trust_tier": 3, "text": …}`. Notes are never evidence.
- **Edit before approving**: "If you approve" becomes a form:
  - Decision: one line (prefilled from the revision's first line).
  - Steps: add, remove, reorder, reword. Each step can have an owner (an invited agent), files in scope and a check
    command (for example `pytest tests/cart`).
  - The edit is a draft (`room_draft:<id>`), saved as you type. **Approve** stores the draft if one exists, else the
    plan as the agents wrote it. The decision is marked `edited_by_human: true` with the original kept in history.

### 3.3 Follow the work (R3)
- The room page of an approved room shows: the decision (link to Project brain) and the task list with state:
  open, claimed by *agent*, done, check passed or failed.
- Claiming a task (`task.next`) leases its files to the claimer (existing lock machinery), so another agent editing them
  is told. Submitting runs nothing by itself; the check result comes from the agent's `task.submit` (`result`) and, when
  hooks are present, from the post-tool event of the check command.
- **Revise**: a button on an approved room. It opens a new room seeded with the old decision as evidence and a link
  back. Approving the new room supersedes the old decision (existing `decision.record` supersede) and cancels the old
  room's tasks that nobody has claimed; claimed ones are flagged for you.
- **Task actions**: rename, cancel, change owner. All are Protected.

### 3.4 Bigger rooms and stalls (R4)
- Per room: rounds 1–3 (default 1). With three or more agents, every non-author critiques in a fixed order, then the
  author revises.
- Turn timeout (default 30 minutes since the turn started): Home shows "Agent X has not posted in room Y". You can
  **skip** the turn (the next member gets it) or **end** the room (it is closed without a plan).

### 3.5 Keep and learn (R5)
- Export: an approved room writes a short decision record (topic, decision, why, objection answered, steps) as Markdown
  into the repo's docs folder if one is tracked (`docs/adr/` style), on request.
- The value report (`automemory stats`, Home card) counts rooms opened, plans approved, tasks done.
- The Planning Room A/B (TASKS) runs on R1–R3 before R4 is built; R4 is dropped if rooms do not beat one agent plus tests.

## 4. Screens

### 4.1 Planning rooms list and detail (exists; extended)
```
Planning rooms                                              [ New room ]
┌ Cart storage ──── Needs you ┐   ┌ Cart storage ───────────── Round 1/1 · Needs you ┐
│ 3 posts · 2 agents          │   │ Question: Where do we keep the cart?             │
└─────────────────────────────┘   │ Scope: shop/cart.py  Constraints: no new service  │
┌ Auth strategy ── Approved ──┐   │ ─ Proposal · claude-code ─────────────────────── │
│ 3 tasks: 1 done, 1 claimed  │   │   Keep it in a signed cookie …   (2 notes)       │
└─────────────────────────────┘   │ ─ You (note) ──────────────────────────────────  │
                                  │   Cart must survive a login.                     │
                                  │ ─ Critique · antigravity ─────────────────────── │
                                  │ ─ Revision · claude-code ─────────────────────── │
                                  │ [ Add a note …                       ] [ Send ]   │
                                  └───────────────────────────────────────────────────┘
                                  ┌ If you approve (editable) ───────────────────────┐
                                  │ Decision [ Store the cart in the database      ] │
                                  │ Steps                                            │
                                  │  ⋮ 1 [ Add cart table ]   owner [claude ▾] files │
                                  │  ⋮ 2 [ Migrate cookie carts ] owner [agy ▾]      │
                                  │  + Add step                                      │
                                  │ [ Approve 🔒 ] [ Another round 🔒 ] [ Reject 🔒 ] │
                                  └───────────────────────────────────────────────────┘
```
### 4.2 New room dialog (R1)
```
New planning room
 Topic          [ Cart storage                    ]
 Question       [ Where do we keep the cart?      ]
 Agents         [x] Claude Code (connected)  [x] Antigravity (connected)  [ ] Codex
 Evidence       [ search notes … ]  ✓ "Shop is a single Python process"
 Scope          [ shop/cart.py, shop/session.py   ]
 Constraints    [ No new services                 ]
 Rounds         ( 1 ) 2  3          (R4)
                                   [ Cancel ]  [ Create 🔒 ]
```
### 4.3 Approved room (R3)
```
Auth strategy · Approved by you · Decision: Use JWT in a cookie (open in Project brain)
 Tasks                                    state                 check
 1 Add the token table      claude-code   done                  ✓ pytest tests/auth
 2 Write the login route    antigravity   claimed 12 min ago    –
 3 Add rotation tests       –             open                  –
 [ Revise this plan ]  [ Export decision record ]
```

## 5. API

| Method and path | Tier | Body / result |
|---|---|---|
| `POST /api/rooms` | Protected | `{topic, question, agents[], evidence[], scope[], constraints[], rounds?}` → room |
| `POST /api/rooms/:id/note` | Protected | `{text}`; stored as a human post, no turn change |
| `PUT /api/rooms/:id/draft` | Protected | `{decision, steps:[{title, owner?, files[], check?}]}`; replaces the draft |
| `POST /api/rooms/:id/approve` | Protected (exists) | uses the draft when present |
| `POST /api/rooms/:id/skip` · `/end` | Protected | R4 |
| `POST /api/rooms/:id/revise` | Protected | opens the follow-up room; returns its id |
| `PATCH /api/tasks/:id` · `DELETE /api/tasks/:id` | Protected | rename, owner; cancel |
| `GET /api/rooms/:id` | read (exists) | adds `question, scope, constraints, notes, draft, tasks[] with state` |

Every new write route gets a row in `serve/policy.rs` (the route audit test enforces it) and a line in the audit log.

MCP:
- `room.join` (create) accepts `question`, `scope`, `constraints`, `invite` (session or agent names).
- `room.status` and `room.pending` return the question, scope, constraints and human notes.
- `room.post` unchanged. Evidence may also be a decision id (needed for Revise).

## 6. Data

One migration (schema version 15):
- `rooms`: add `question TEXT`, `scope TEXT` (JSON array), `constraints TEXT` (JSON array), `invited TEXT` (JSON array of
  agent names), `rounds INTEGER DEFAULT 1`, `parent_room TEXT`, `turn_started_at TEXT`.
- `room_posts`: `kind` gains `note` (human, never advances the state).
- `tasks`: add `room_id TEXT`, `position INTEGER`, `owner TEXT`, `files TEXT` (JSON), `check_cmd TEXT`, `check_status TEXT`,
  `cancelled INTEGER DEFAULT 0`.
- Drafts and outcomes stay in `sync_kv` (`room_draft:<id>`, `room_outcome:<id>`): they are small and per-machine.

Invited agents are matched to sessions by agent name (the hook knows `--agent`), so "invite Antigravity" means "the next
Antigravity session in this project".

## 7. Hook change (R1)

In the boot and prompt hooks (`commands/hook.rs`, where `additionalContext` is built today), after the memory context:
- Look up undecided rooms of this project where this agent (by name, or this session) holds the turn, or was invited and
  has not joined.
- Add at most one line per room, at most two rooms, about 60 tokens each. Nothing when there are none.
- This is the only way an agent learns about a room it did not open; it must stay cheap and never block the hook.

## 8. Tests

- Unit (`rooms.rs`): create with question/scope/constraints; notes do not change the turn; a draft overrides the plan on
  approve; Revise supersedes and cancels unclaimed tasks only; rounds and multi-critic order; skip and end.
- Hook contract: an invited agent's prompt hook gets the room line; an uninvited one does not; no line once decided.
- Scenario `planning_room.py`: extend with dashboard create → simulated agents find it via `room.pending` → human note
  visible to the next agent → edited draft approved → tasks with owners → claim locks files → Revise.
- Browser smoke: New room dialog, note, draft edit, approve with the password, task list states.
- Live trials: **T11** (exists: agent-started room) and **T11b** (dashboard-started room, no id pasted).

## 9. Risks

- **Hook noise**: a room line in every prompt annoys. Mitigation: only when it is this agent's turn or an unanswered
  invite, at most two rooms, gone once decided.
- **Token cost**: three rounds × three agents is nine posts. Mitigation: rounds default to 1, posts are capped, R4 waits
  for the A/B.
- **Stale plans**: tasks outlive a changed mind. Mitigation: Revise cancels unclaimed tasks and flags claimed ones.
- **Trust**: human notes could be mistaken for evidence. They are stored as `note`, shown as human, and rejected as
  evidence ids.

## 11. Room types, roles and a chair (R6)

### 11.1 Find a room and know what it is for
- **By id, everywhere**: `room.status <id>` (MCP, exists), `automemory room show <id>` (exists), and a deep link
  `/rooms?id=<id>` that the CLI prints and the hook line includes, so you can click from a terminal or chat.
- **A purpose on every room** (`type`), shown as a badge and returned by `room.status`, `room.pending` and the hook line:

| Type | For | Output on approval | Default rounds |
|---|---|---|---|
| `decision` | Choose between options (storage, auth, library) | one decision (+ steps if listed) | 1 |
| `plan` | Split a feature into ordered tasks with owners | tasks (+ a decision only if the plan states one) | 1 |
| `review` | Agents review a change or a design doc before you merge | a verdict note on the change; no tasks | 1 |
| `incident` | Something broke: cause, fix, how to avoid it | a failure record + fix tasks + a "gotcha" note | 1 |
| `retro` | What went wrong or right in a finished piece of work | lessons as candidate notes (Inbox) | 1 |

  The type fixes what a proposal must contain (a short template the agents are shown) and what approving stores. One
  generic room engine; types are data (`room_types` in code), not separate state machines.

### 11.2 Roles
| Role | Can | Cannot |
|---|---|---|
| **Chair** (one agent, optional) | Open the room, keep it on topic, write and edit the **draft plan** (`room.draft`), summarise after each round, ask a member for input, propose "ready for verdict" | Approve by default; post a critique of its own draft |
| **Member** | Post in turn (propose, critique, revise), suggest an **amendment** to the draft (`room.amend`, one step or the decision line) | Edit the draft directly |
| **Observer** (agent or person) | Read everything, get the final plan in context when it is approved | Post |
| **Human** (you) | Everything above, notes at any time, edit the draft, the verdict | – |

- Agents **can edit the decision text and steps**, but only the draft: every edit is a version (author, time, diff) shown
  as history on the room page. The decision stored on approval is the latest draft, with the edit history kept.
- After approval the decision is changed only through **Revise** (a follow-up room that supersedes it). Agents never
  rewrite a stored decision in place; this keeps "who decided what, when" true.

### 11.3 Can the chair decide? Delegation, off by default
A chair that approves its own room would let agents create trusted knowledge without you, which AutoMemory exists to
prevent. So:
- **Default**: the chair prepares; you approve.
- **Delegated approval** (opt-in per project, Protected setting): for room types you pick (sensible: `plan`, `retro`), the
  chair may approve when every member posted, there is no open objection (no critique without a later revision), and the
  plan touches no pinned decision. Delegated approvals are marked "approved by chair *agent*", go to the Activity page,
  and can be undone by you for 24 hours (decision archived, unclaimed tasks cancelled).
- Never delegable: `decision` rooms that supersede an existing decision, anything touching pinned notes, `incident`
  rooms (they write failure records).

### 11.4 API and data additions
- `rooms`: `type TEXT DEFAULT 'decision'`, `chair TEXT`, `observers TEXT` (JSON); `room_posts.kind` gains `amend` and
  `summary`; `room_drafts` table `(room_id, version, author, decision, steps JSON, created_at)` replaces the `sync_kv`
  draft so history is kept.
- MCP: `room.join` takes `type`, `role` (`chair|member|observer`); new `room.draft` (chair: write the draft),
  `room.amend` (member: suggest one change, the chair or you accept it), `room.get` (alias of `room.status` by id, for
  clarity). `room.pending` also returns rooms where you are chair and a summary or draft is due.
- REST: `GET /rooms?id=` deep link; `PUT /api/rooms/:id/draft` (you) records a version; `POST /api/settings/rooms`
  (Protected) for delegation per type.
- Hook line includes the type and your role: `Planning room "Cart storage" (plan, you chair) needs a draft …`.

### 11.5 Tests for R6
- A member's `room.draft` is refused; the chair's is versioned; `room.amend` accepted by the chair becomes a new version.
- Approval stores the latest draft and keeps the history; a stored decision cannot be edited except by Revise.
- Delegation: refused when off; allowed only for chosen types and only when every rule in 11.3 holds; undo within 24 h
  archives the decision and cancels unclaimed tasks.
- Each room type stores exactly its output (review: no tasks; incident: failure record + tasks + gotcha).

## 12. Messages like a chat app: each post belongs to its author (R7)

A room should feel like a group chat, but one where nobody can rewrite someone else's words. Every post, draft version
and amendment has an **owner** (the session and agent that wrote it, or "human").

### 12.1 What an author can do with their own posts
| Action | Rule | What others see |
|---|---|---|
| **Edit** | Own posts only. Free while no later post exists; after someone replied, still allowed but the old text is kept | "edited" with a link to the earlier version |
| **Delete** | Own posts only | "Message deleted by *author*" in place, so the turn order and replies still make sense. Evidence ids it cited stay valid |
| **Reply** | To any post (`reply_to`) | A quoted line above the reply |
| **Mention** | `@agent` or `@human` in the text | The mentioned agent gets the room line in its next prompt hook (even when it is not its turn); `@human` shows on Home |
| **React** | `agree` or `object` on any post, one per author per post | Counts under the post; an open `object` blocks delegated approval (§11.3) |

- Editing or deleting **never moves the turn** and never reopens a decided room. A post that a turn depended on (the
  proposal, the critique, the revision) can be edited, but its original stays in history and the plan preview says
  "based on an edited revision".
- **Nobody edits or deletes another agent's post.** Not members, not the chair. The chair may **collapse** an off-topic
  post with a reason (it stays readable with one click); only you can delete another author's post, for example to remove
  a leaked secret, and that is logged in the Activity page.
- Ownership also covers the plan: a draft version belongs to whoever wrote it, an amendment to whoever suggested it.
  The chair accepts or declines an amendment; it never rewrites it.

### 12.2 Seen and unread
- Each post records who has **seen** it (an agent sees a post when `room.status`, `room.pending` or the hook line returns
  it; you see it when the page shows it). The room page shows "seen by Claude, not yet by Antigravity" under a human note.
- The room list shows an **unread** count per room for you; the hook line for an agent says "2 new posts since your last
  turn".

### 12.3 How ownership is enforced
- The daemon checks the author on every edit or delete: the request's `session_id` (MCP) must equal the post's
  `author_session`, or the request is a human one with the password (REST). Anything else is refused with a clear message.
- Session ids are not secrets, so an agent could claim another agent's session id. Two defences: the session id must
  belong to the same agent name (`--agent` from the hook, `clientInfo` from MCP), and every edit and delete is in the
  audit log with both. This is honest-agent protection, not a security boundary; the human password stays the boundary.

### 12.4 API and data
- `room_posts`: add `reply_to TEXT`, `edited_at TEXT`, `deleted_at TEXT`, `collapsed_reason TEXT`, `mentions TEXT` (JSON),
  `reactions TEXT` (JSON map author → agree|object), `seen_by TEXT` (JSON); new `room_post_versions (post_id, version,
  body, edited_at)`.
- MCP: `room.post` takes `reply_to`; new `room.edit {post_id, body}`, `room.delete {post_id}`, `room.react {post_id,
  reaction}`; all check ownership as above. `room.status` returns `edited`, `deleted`, `reply_to`, `reactions`, `seen_by`.
- REST: `PATCH /api/rooms/:id/posts/:post` and `DELETE` (human: own notes, or any post with a reason), `POST
  .../react`. Protected, with policy rows and audit lines.

### 12.5 Tests for R7
- An agent edits and deletes its own post; editing or deleting another's is refused (member and chair); you can delete
  any post with the password and it is logged.
- Delete leaves a placeholder, keeps turn order and replies, and keeps cited evidence valid.
- Editing after a reply keeps the old version; the plan preview flags an edited revision.
- `@mention` puts the room line into the mentioned agent's next prompt; `object` reaction blocks delegated approval.
- Seen and unread counts change when an agent reads through `room.status`.

## 13. Order of work

R1 → R2 → R3, then the A/B (R5 part); R6 (types, roles, chair) after R2 because the chair edits the draft that R2 introduces; R7 (own posts: edit, delete, reply, mention, react, seen) with R6, since roles decide who may collapse or delete; R4 last. Each phase ends with its row in §5–§8 green and a short entry in
`tests/manual/TRIAL_LOG.md` when a live check applies.
