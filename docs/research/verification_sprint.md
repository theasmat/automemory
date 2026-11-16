# Verification sprint (Nov 2026)

The gap-analysis research read AutoMemory's code against its docs and listed claims that looked wrong. Each was turned into a test
that states what the docs or rules say should hold, and run. Tests: `crates/automemory-mcp/tests/verification_sprint.rs` (run the
failing ones with `cargo test -p automemory-mcp --test verification_sprint -- --include-ignored`) and
`the_mcp_gateway_is_behind_the_origin_and_host_guard` in `crates/automemory-cli/src/commands/serve/mod.rs`.

## Results

| # | Claim | Result | How checked |
|---|---|---|---|
| V1 | An agent cannot raise its own trust tier | **Confirmed bug, fixed.** `memory.save` copied the caller's `metadata` into the record, so `{"trust_tier": 1}` makes an agent's note Pinned (Tier 1) and `{"trust_tier": 3}` makes it count as a person's. Also makes `is_person_authored` true, so the person-authority guards protect the agent's note | ran (ignored test) |
| V1b | A self-reported confidence does not verify a note | **Confirmed bug, fixed.** `confidence >= 0.95` alone gave Tier 2 (Verified); an agent's confidence is now capped at 0.94 | ran (ignored test) |
| V2 | Agent-recorded decisions do not reach AGENTS.md / CLAUDE.md | **Confirmed, fixed.** `decision.record` from an agent was exported under "Decisions in force". Memories are protected (the export requires a person as source); decisions are not | ran (ignored test) |
| V3 | `/mcp` is unauthenticated | **Refuted for web pages.** The merged `/mcp` route is behind the Origin/Host guard (a foreign Origin or rebinding Host gets 403). Still true: any local process without an Origin can call it, with no agent identity (loopback by design) | ran (passing test) |
| V4 | Recall labels `changed` notes | **Confirmed bug, fixed.** A note flipped to `changed` after a commit disappeared from recall (search filters `status = 'active'`), so it is neither returned nor labelled | ran (ignored test) |
| V5 | Symbol hashes verify memories | **Confirmed gap; docs corrected, wiring still open.** `CodeIntel::symbol_hash` has no caller in the verifier; verification is by status, age and file hash | read; a passing test fails when someone wires it |
| V6 | A pre-commit gate protects commits | **Confirmed gap; docs corrected, wiring still open.** The pipeline event exists but `connect` installs only a post-commit block; installing a git pre-commit hook adds a gate on people's commits, so it needs your go-ahead | read, not run |
| V7 | Only a person decides proposals | **By design, opt-in.** Agents are refused once a human password is set; with no password set they can decide | read |

Read only, not yet run: the unchecked claims listed in the research notes (recall never marks notes as recalled so the 60-day fade
never starts; hybrid vector mode never reaches `memory.recall`; the PBKDF2 round counts in the docs; the `sync.md` gap list and the
CHANGELOG record kinds that do not exist; the stale test counts).

## What was fixed (Nov 2026)

- **V1/V1b:** `memory.save` drops a caller-supplied `metadata.trust_tier` and caps an agent's own confidence at 0.94, so the tier comes only from the source, a check or a person.
- **V2:** the instruction-file export includes only decisions a person recorded (agent `human`: the CLI, the dashboard, an approved planning room). To publish an agent's decision, record it yourself; that supersedes it.
- **V4:** `memory.recall` returns `changed` and `stale` notes ranked below live ones, with their status; pinned invariants are returned too (they were filtered out with `status = 'active'`).
- **Also found while fixing:** `memory.recall` never marked notes as recalled, so the 60-day fade would bury notes agents use every day. It now does.
- **Docs corrected:** symbol hash and pre-commit gate (not wired), the sync key derivation (24-word phrase is the key; 12-word is HMAC-expanded; PBKDF2 is for the password), the test count, the synced record kinds, and the `doctor` text.

## Also fixed (Nov 2026, second pass)

- **V5:** PostCommit now uses `symbol_hash`. `memory.save` takes `symbols`; with the code index present it records their hashes (never taken from the caller), and PostCommit flags such a memory only when a hash changed. Without hashes or without the engine it behaves as before (any edit to a linked file).
- **V6:** `connect` installs a git pre-commit block next to the post-commit one. It shows the hook's warning in the commit output and blocks only when the hook answers deny (Strict mode); it fails open; `disconnect` removes it and leaves a user's own hook. Checked live in a sandbox repo: Vibe warns and commits, Strict blocks with "Collision prevented".
- **Hybrid recall:** `memory.recall` now fuses keyword and vector results when `[retrieval] mode = "hybrid"` (it was keyword-only).
- **`doctor`:** the Claude Code and Codex hook lines see this folder's hooks; the registration count equals `disconnect all --everywhere --dry-run`; the identity hint names a command that exists.

## Closed in the third pass

- **Decisions record symbol hashes.** `decision.record` stores the hashes of its `symbols` on the memory that carries the decision to recall (when the code index is present); PostCommit marks that memory `changed` when a symbol's hash moved, even though no file is linked to it. Hook-captured notes are left as they are: hook payloads name files, not symbols, so there is nothing to hash and the file-level link stays.
- **`memory_links`** had no reader or writer anywhere; schema v22 drops it.
- **Planning rooms.** The critique round now waits for every invited agent, not only those already joined (in the live run Antigravity had not joined when Codex critiqued, so the room went straight to the revision). The turn is left open for the missing agent; the human can Skip turn if it never comes. `automemory room show` and `room list` label sessions with the agent's name (`claude/b3d66002`).

## Still open

- Nothing from the sprint. Hook payloads do not carry symbols; a way to derive them from a file would be new work, not a fix.
