# AutoMemory

Local-first Rust daemon (SQLite WAL, MCP server, CLI, Nuxt dashboard) giving coding agents shared, verified memory, locks and handoffs.

## Docs (all in `docs/`, read before changing design)

- [docs/ARCHITECTURE.md](docs/ARCHITECTURE.md) — how it is built: ports, hook pipeline, data model, interfaces, guardrails, sync
- [docs/FEATURES.md](docs/FEATURES.md) — what it does, v1 scope, feature status
- [docs/PLAN.md](docs/PLAN.md) — phases, gates, risks, open decisions
- [docs/TASKS.md](docs/TASKS.md) — open work checklist
- [docs/research/](docs/research/) — gap analysis (what multi-agent workflows still lack)
- [plan/ui-ux.md](plan/ui-ux.md) — dashboard UI/UX redesign spec (information architecture, screens, design system, build phases)

Tests and how to run them: [tests/README.md](tests/README.md) (`tests/run_all.sh`); live-agent checks in `tests/manual/`, eval harness in `tests/eval/`.

Only these four docs, `docs/research/` and `plan/ui-ux.md` exist. Update them in place; do not add new top-level doc files. Keep counts (tools, tests) in sync with code.

## Rules

- v1 scope: Claude Code, Codex, Antigravity adapters; Planning Room; Nuxt dashboard. Cloudflare SaaS and more agents come later.
- The daemon makes no LLM calls. Agents never touch the DB with raw SQL (Zero-SQL rule).
- Build/test: `tests/run_all.sh` (or `cargo test --workspace`); bench: `cargo bench -p automemory-core --bench perf`.
