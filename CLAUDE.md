# AutoMemory

Local-first Rust daemon (SQLite WAL, MCP server, CLI, Nuxt dashboard) giving coding agents shared, verified memory, locks and handoffs.

## Docs (all in `docs/`, read before changing design)

- [docs/ARCHITECTURE.md](docs/ARCHITECTURE.md) — how it is built: ports, hook pipeline, data model, interfaces, guardrails, sync
- [docs/FEATURES.md](docs/FEATURES.md) — what it does, v1 scope, feature status
- [docs/PLAN.md](docs/PLAN.md) — phases, gates, risks, open decisions
- [docs/TASKS.md](docs/TASKS.md) — open work checklist
- [docs/research/](docs/research/) — gap analysis (what multi-agent workflows still lack)
- [plan/ui-ux.md](plan/ui-ux.md) — dashboard UI/UX redesign spec (information architecture, screens, design system, light theme, first-run tour)
- [plan/onboarding-tour-brief.md](plan/onboarding-tour-brief.md) — self-contained brief for building the first-run tour (CLI + dashboard)
- [plan/room-plan.md](plan/room-plan.md) — Planning rooms spec: start from the dashboard, human notes and plan edits, tracked tasks, revise (R1–R5)

Tests and how to run them: [tests/README.md](tests/README.md) (`tests/run_all.sh`); live-agent checks in `tests/manual/`, eval harness in `tests/eval/`.

Only these four docs, `docs/research/` and the three files in `plan/` exist. Update them in place; do not add new top-level doc files. Keep counts (tools, tests) in sync with code.

## Rules

- v1 scope: Claude Code, Codex, Antigravity adapters; Planning Room; Nuxt dashboard. Cloudflare SaaS and more agents come later.
- The daemon makes no LLM call unless the user configures `[llm]` (off by default); an LLM only suggests (Inbox proposals, candidate notes), never writes trusted memory, takes a lock or decides a guardrail. Agents never touch the DB with raw SQL (Zero-SQL rule).
- Build/test: `tests/run_all.sh` (or `cargo test --workspace`); bench: `cargo bench -p automemory-core --bench perf`.
