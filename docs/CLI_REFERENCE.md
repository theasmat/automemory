# CLI Command Reference

This is the comprehensive cheat sheet for the `automemory` command-line interface.

---

## Global Options
- `-h, --help`: Print help for any command or subcommand.
- `-V, --version`: Print binary version.

---

## 1. Setup & Diagnostics

### `automemory init`
Initializes the AutoMemory directory (`~/.automemory/`), sets up the encrypted SQLite database, and creates initial project configuration.
```bash
automemory init [--agent <agent>]
```

### `automemory doctor`
Runs a full health check across the SQLite database, WAL mode, FTS5 latency, encryption keys, and agent hooks.
```bash
automemory doctor
```

### `automemory connect`
Automatically configures AutoMemory into host coding agents.
```bash
# Connect an agent in current workspace:
automemory connect <agent>

# Connect globally across user configuration:
automemory connect <agent> -g

# Connect all 20 agents at once:
automemory connect all [-g]
```
Supported agents: `agy`, `claude`, `claude-desktop`, `cursor`, `windsurf`, `continue`, `cline`, `roo`, `kilo`, `zed`, `codex`, `opencode`, `goose`, `aider`, `warp`, `gemini-cli`, `devin`, `copilot`, `openclaw`, `hermes`, `all`.

---

## 2. Server & Daemons

### `automemory serve`
Starts the local background daemon, HTTP REST API, WebSocket event gateway, and embedded web console.
```bash
automemory serve [--port 4545] [--db <custom-path>]
```

### `automemory mcp`
Launches the Model Context Protocol (MCP) server over standard input and output (stdio) for coding agent integration.
```bash
automemory mcp [--db <custom-path>]
```

---

## 3. Review Proposals & Zero-SQL Governance

### `automemory proposal summary`
Displays aggregated summary metrics of all review proposals by status and type.
```bash
automemory proposal summary [--project <slug>]
```

### `automemory proposal list`
Lists proposals with optional status and type filters.
```bash
automemory proposal list [--status pending|approved|rejected|all] [-t <type>] [--limit 20] [--offset 0]
```

### `automemory proposal decide`
Approves or rejects an individual proposal.
```bash
automemory proposal decide <proposal-id> --approve
automemory proposal decide <proposal-id> --reject
```

### `automemory proposal approve-all`
Atomically approves all pending proposals and automatically archives duplicate memories.
```bash
automemory proposal approve-all [-t duplicate_merge] [--project <slug>]
```

### `automemory proposal reject-all`
Atomically rejects all pending proposals.
```bash
automemory proposal reject-all [-t <type>] [--project <slug>]
```

---

## 4. Memory Management

### `automemory memory list`
Lists active memories for a project.
```bash
automemory memory list [--project <slug>] [--limit 20]
```

### `automemory memory search`
Performs FTS5 keyword and semantic search.
```bash
automemory memory search <query> [--semantic] [--project <slug>]
```

### `automemory memory save`
Explicitly records an architectural decision, lesson, or preference.
```bash
automemory memory save "Use PostgreSQL for financial transactions" \
  --content "Chose PostgreSQL over MongoDB due to ACID requirements." \
  --type decision \
  --scope project
```

### `automemory memory archive`
Soft-deletes a memory record.
```bash
automemory memory archive <memory-id>
```

---

## 5. Autopilot Safety Modes & Guardrails

### `automemory mode get`
Shows the active autopilot mode for the workspace.
```bash
automemory mode get [--project <slug>]
```

### `automemory mode set`
Configures the autopilot mode. Requires human authorization (see `rule passwd`).
```bash
automemory mode set vibe     # Max velocity, inline warnings
automemory mode set strict   # Halts on all high-risk commands
automemory mode set review   # Queues commands for operator approval
```

### `automemory rule list`
Lists all built-in and custom guardrail rules.
```bash
automemory rule list
```

### `automemory rule add`
Adds a custom regex guardrail rule. Requires human authorization (see below).
```bash
automemory rule add --name "Protect Keys" --pattern "(?i)/id_rsa" --critical
```

### `automemory rule toggle`
Enables or disables an existing rule. Requires human authorization (see below).
```bash
automemory rule toggle <rule-id> --enable
automemory rule toggle <rule-id> --disable
```

### `automemory rule remove`
Removes a custom guardrail rule. Requires human authorization (see below).
```bash
automemory rule remove <rule-id>
```

### `automemory rule passwd`
Sets the human password guarding safety writes (`rule add/remove/toggle`, `hooks enable/disable/move`, `mode set`, dashboard rule/mode edits). Agents cannot satisfy it: it accepts only the `AUTOMEMORY_PASSWORD` env var or an interactive terminal prompt — never piped stdin, never a CLI flag (flags leak to shell history).
```bash
automemory rule passwd
automemory rule passwd --from-env   # read from AUTOMEMORY_PASSWORD instead
export AUTOMEMORY_PASSWORD=<password>  # non-interactive human scripts
```
With no password set, safety writes are allowed once with a warning pointing at `rule passwd`. Passwords are stored as PBKDF2-HMAC-SHA256 hashes, never plaintext.

---

## 6. Zero-Knowledge Synchronization & Vault

### `automemory sync pair`
Generates a 12-word BIP-39 mnemonic recovery phrase and terminal QR code.
```bash
automemory sync pair
automemory sync pair --join "word1 word2 ... word12"
```

### `automemory sync push`
Pushes git-ignored project plans to your private GitHub vault and encrypted deltas to Cloudflare Edge.
```bash
automemory sync push [--project <slug>]
```

### `automemory sync pull`
Pulls specification files from your private vault and decrypts edge memory updates.
```bash
automemory sync pull [--project <slug>]
```

### `automemory sync watch`
Runs a background file watcher daemon for continuous real-time synchronization.
```bash
automemory sync watch [--interval 5] [--project <slug>]
```

### `automemory sync status`
Inspects local uncommitted plans, sync ledger revisions, and cloud connectivity.
```bash
automemory sync status
```

---

## 7. Multi-Agent Coordination, Plugins & Eval

### `automemory disconnect`
Removes exactly what `connect` wrote for one agent (MCP server keys, hooks key, plugin dir). Third-party entries and edited files are left intact.
```bash
automemory disconnect cursor
automemory disconnect agy -g
```

### `automemory adapter`
Lists the 20 registered adapters with hook coverage, or re-runs an adapter's installer.
```bash
automemory adapter list
automemory adapter update cursor [-g]
```

### `automemory hooks`
Inspects and dry-runs the hook pipeline (`hooks.toml` layers).
```bash
automemory hooks list
echo '{"tool":"edit","paths":["src/a.ts"]}' | automemory hooks test --event pre-tool --input "$(cat)"
automemory hooks enable <id> | automemory hooks disable <id> | automemory hooks move <id> <priority>
```

### `automemory plugin`
Lists the 16 versioned ports and installed manifests; installs/updates with api_version gating, pins versions, rolls back.
```bash
automemory plugin list
automemory plugin install ./my-plugin.toml
automemory plugin pin <name> | automemory plugin rollback <name>
```

### `automemory eval`
Runs the local eval suites (retrieval, stale flags, leases, hook latency); exit 0 iff all pass.
```bash
automemory eval run [--json]
```

### `automemory stop`
Kill switch: releases every active lease, halting multi-agent work.
```bash
automemory stop [--project <slug>]
```

### `automemory locks` / `automemory unlock`
```bash
automemory locks [--project <slug>]
automemory unlock <scope> [--project <slug>] [--force]
```

### `automemory decision`
```bash
automemory decision record --topic auth --decision "..." --reason "..."
automemory decision get <topic>
automemory decision list
```

### `automemory pin` / `veto` / `revoke`
```bash
automemory pin <memory-id>
automemory veto <memory-id> [--reason "..."]
automemory revoke [--id <id>] [--session <id>] [--agent <name>]
```

### `automemory baton`
```bash
automemory baton pass --session <id> [--to <id>] [--note "..."]
automemory baton pickup [--session <id>]
```

### `automemory memory get` / `digest`
```bash
automemory memory get <memory-id>
automemory memory digest [--span 1h] [--save]
```
