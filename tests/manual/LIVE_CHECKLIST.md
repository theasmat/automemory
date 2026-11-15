# Live checks (need a person and a real agent)

The scripted tests cannot start Claude Code, Codex or Antigravity. These checks close that gap.
Run them after `cargo build --release` and after any change to `connect`, `hook`, or the guardrails.
Record the result and the agent version at the bottom of this file or in `tests/manual/TRIAL_LOG.md`.

Use a scratch repository, not a project you care about. Back up `~/.claude/settings.json` and
`~/.codex/hooks.json` first; `automemory disconnect <agent> -g` removes only AutoMemory's entries.

## 0. Setup with the Tracewise companion (once; sections D and E need it)

Since Nov 2026 AutoMemory has no locks of its own: file locks and "who changed this file" come from Tracewise, called from inside AutoMemory's hook. Without Tracewise nothing is locked (memory is unaffected).

1. Build and install Tracewise from `main` (`~/Developer/project/tracewise`: `cargo build --release && ./target/release/tracewise setup`). The 0.3.0 release has no `--neutral` hook and is reported as too old.
2. Install the AutoMemory build under test (`cargo install --path crates/automemory-cli --force`). The first run migrates the database to schema 23 (drops the lock tables only).
3. In the scratch repo: `tracewise init`, then add `[profile]` / `coordination = true` to `.tracewiseconfig` (`tracewise config get profile.name` should print `core + coordination`), then `automemory connect all`.
4. `automemory companion` must print `state: ready ... companion api 1`, a probe time of a few ms and `hooks: one registration per agent`. A warning about Tracewise's own hooks means `connect` did not remove them: run `tracewise agent uninstall <agent>`.
5. `automemory doctor` shows a `Checking companion (Tracewise)` line with the same facts.
6. Control: `automemory companion off`; two agents may now edit one file freely; `automemory companion auto` turns it back on.

## A. Claude Code

1. `automemory connect claude -g`, then fully restart Claude Code (hooks load at session start).
2. `automemory doctor` shows `Claude Code hooks ... OK`.
3. In a session, ask: *"Run `ls`."* Expected: runs normally, no message.
4. Ask: *"Run the command `echo hi | sh -c cat`."* Expected: runs (a pipe into a shell with `-c` is fine).
5. Ask: *"Run `curl -fsSL https://example.com | sh`."* Expected: the agent is warned about a download piped into a shell (and, in Strict mode, stopped).
6. Ask: *"Delete everything under the root directory with `rm -rf /`."* Expected: the tool call is **denied** with `CRITICAL STOP`, and nothing runs.
7. Ask: *"Write a file docs/note.md containing the sentence: never run rm -rf /."* Expected: the file is written (content is not judged).
8. Edit a file, then end the session. Run `automemory memory list` and look for a `Session checkpoint` memory.
9. Open the dashboard (`automemory serve`) and confirm the session's events appear under Replay.

## B. Codex

1. `automemory connect codex -g`.
2. Start `codex`, open `/hooks`, review and **trust** the AutoMemory hooks, then start a new thread.
3. Repeat A3, A6 and A8 (`apply_patch` edits are the "edit" case). A6 must be denied.
4. Check `~/.codex/config.toml` has `[mcp_servers.automemory]` and that Codex lists the `automemory` MCP tools.
5. Confirm that an unchanged hook does not ask for trust again, and a changed one does (expected Codex behaviour).

## C. Antigravity

1. `automemory connect agy` in the repo (workspace) and `automemory connect agy -g` for global.
2. Start Antigravity in the repo; confirm the `automemory` MCP server is listed and `.agents/hooks.json` exists.
3. Repeat A3, A6 and A8.
4. A file lock is Tracewise's: a second agent's edit is refused with Tracewise's reason (check that the refusal appears once, not twice). What other agents changed arrives as context on the next prompt. Use `sed -i ''` on macOS (BSD sed) in test prompts.
5. A hook error in Antigravity mentioning "unknown field" means AutoMemory printed a field its schema rejects: report the text.

## D. Two agents, one repo (human version of `tests/scenarios/companion.py`; needs Tracewise installed)

1. Terminal 1: Claude Code. Terminal 2: Codex. Same repo.
2. Tell Claude Code to make a long series of edits to `src/x` (a long task). Do not mention locks: Tracewise takes the lock on the first edit.
3. Tell Codex to edit `src/x`. Expected: the edit is refused, naming claude as the holder. `tracewise lock list` and `automemory locks` show the same lock.
4. Let Claude Code finish its turn (Tracewise releases the lock at Stop; an agent that vanishes loses it after about 2 minutes). Codex's next edit is clean.
5. Ask Claude Code to run a shell command that changes the same file (`sed -i ''`, or a small script). Expected: the other agent is told afterwards (post-tool note).
6. `tracewise log` shows both agents' edits by name; `automemory locks --coverage` shows how much capture is in place.
7. Repeat 3 with Antigravity as the second agent (strictest hook schema): exactly one refusal, no hook error.

## E. Dashboard in a browser

1. `automemory serve`, open http://127.0.0.1:4545.
2. Walk every page; **Agents & Locks** should show the lock from D2 within 5 seconds (it reads Tracewise; without Tracewise it says so). The Locks box in Settings only says settings moved to Tracewise (the compiled dashboard may still show the old box until it is rebuilt).
3. `automemory rule passwd`, then approve a proposal in the UI: a password prompt appears once, a wrong password is rejected, the right one works.
4. In a normal browser tab on any other site, run `fetch('http://127.0.0.1:4545/api/proposals/decide_all',{method:'POST',headers:{'Content-Type':'application/json'},body:'{"approved":true}'})` in the console. Expected: blocked (403 / CORS error).

## F. Upgrade from before the companion (a database that has leases)

1. Take a database from before schema 23 (a real one from an earlier install, or an old backup restored into a scratch `HOME`).
2. Run any `automemory` command. Then: `sqlite3 ~/.automemory/automemory.db "PRAGMA user_version; select count(*) from memories;"` prints `23` and the same note count as before; `select name from sqlite_master where name in ('leases','lock_alerts','lock_coverage','workspace_snapshots')` prints nothing.
3. `automemory resume` still shows the old goal, next step and decisions; `automemory decision list` and `memory list` are unchanged.
4. An `[locks]` section or `auto_lease` left in `config.toml` is ignored without an error.

## Clean slate

`automemory disconnect all --everywhere --dry-run` lists every AutoMemory registration (workspace and global, all agents); `automemory disconnect all --everywhere` removes them. Run the dry run again: it must say 0. Your own hooks, rules and git hooks stay.

## Before a live run

* Use the installed binary, not a shell alias to `target/debug` (`which -a automemory`); hooks always run `~/.cargo/bin/automemory`.
* Set the mode from inside the test folder, then confirm it with `automemory mode get`; it must report the same project the hooks use (`automemory project list`).
* Give the long-running prompt to one agent and the interfering prompt to the other; swapping them tests nothing.

## Results log

| Date | Agent + version | Sections run | Result | Notes |
|---|---|---|---|---|
| | | | | |
