# AutoMemory

Local-first Rust daemon (SQLite WAL, MCP server, CLI, Nuxt dashboard) that gives coding agents shared, verified memory, file locks and hand-offs.

## Commands
- Build: `cargo build` (debug), `cargo build --release`
- Test everything: `tests/run_all.sh` (`--quick` skips the release build and latency run); Rust only: `cargo test --workspace`
- Bench: `cargo bench -p automemory-core --bench perf`
- Dashboard: `cd clients/web && pnpm install && pnpm run generate`, then `rsync -a --delete dist/ ../../crates/automemory-cli/src/dashboard/assets/`

## Layout
`crates/automemory-core` (domain), `-mcp` (MCP tools), `-cli` (CLI, hooks, dashboard server), `clients/web` (Nuxt), `workers/sync-gateway` (hosted-sync worker), `tests/` ([how to run](tests/README.md)), `docs/` ([index](docs/README.md)).

## Rules
- The daemon makes no LLM call unless the user configures `[llm]`; an LLM only suggests and never writes trusted memory, takes a lock or decides a guardrail.
- Never run raw SQL against the AutoMemory database; use MCP tools and CLI commands.
- Never ask for, store or log the human password. Agents cannot approve their own plans or overwrite a person's items.
- Before changing a design, read [docs/decisions.md](docs/decisions.md) and the matching page in `docs/architecture/`.
- Schema changes need a migration and a `LATEST_SCHEMA_VERSION` bump. `schema.sql` runs in full on upgrade, so it must not index or reference a column that only a migration adds.
- Keep docs in place and counts (MCP tools, tests) in sync with the code. Shipped work goes in `CHANGELOG.md`, open work in `docs/roadmap.md`. Do not add new top-level doc files.

<!-- tracewise:start -->
## Tracewise Rules
Always record reasons for file modifications:
- Declare intent before editing: `tracewise intent add --why "<why>" <files...>`
- Run commands with a reason: `tracewise run --why "<why>" -- <command>`
- Review pending changes: `tracewise pending`
- Attach a reason to your pending changes: `tracewise note --last "<why>"` (or `tracewise note <change id> "<why>"`)
<!-- tracewise:end -->

<!-- automemory:begin (generated; change it with `automemory note` / `automemory decision`) -->
## Project memory (AutoMemory)
Shared by every agent on this project. Start from this instead of re-exploring; for more call `memory.list` with `view: context` (MCP). Record non-trivial choices with `decision.record`. File locks are automatic.

_Nothing verified yet._
<!-- automemory:end -->
