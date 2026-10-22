# Agent identity

Who wrote a record must not depend on a name the caller chooses. Provenance labels, the rule "an agent cannot change a person's item" and the room turn rules all rest on it.

## How it works (built)

| Piece | Behaviour |
|---|---|
| Stamped name | `connect` writes the agent into its own MCP entry (`automemory mcp --agent claude`, `--agent codex`, `--agent agy`). The server stamps that name on every write and ignores a different `agent` argument; the mismatch is audited as `mcp.agent_mismatch`. |
| Unconfigured server | An entry written by an older version keeps trusting the caller's name until `connect` is run again. A server of a known agent (Claude, Antigravity, Codex, Cursor) fills the name from its MCP client name when the call gives none. |
| Connection id | The server keeps one connection id per running server and uses it, not the caller's `session_id`, for "same author" checks. A room proposal cannot be critiqued by the connection that made it, whatever session id the call uses. |
| Client check | The MCP `initialize` client name is recorded next to the stamped name. `doctor` ("Checking agent identities") flags another program on an entry and entries with no agent name. |
| Display | The dashboard and `continuity show` show the stamped name. The free `agent` argument is still accepted (old agents send it) but decides nothing. |

## Limit

This stops mix-ups and casual spoofing, not a hostile agent with a shell: it can start its own server with any `--agent`. Anything that must not be faked stays behind the human password.

Code: `crates/automemory-mcp/src/tools.rs` (stamping, connection id), `crates/automemory-cli/src/commands/doctor.rs` (identity check). Tests: `tests/scenarios/agent_identity.py`.
