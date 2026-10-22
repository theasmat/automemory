# AutoMemory documentation

| Read | For |
|---|---|
| [features](features.md) | What the product does, v1 scope, success metrics |
| [architecture/overview.md](architecture/overview.md) | How it is built: ports, hook pipeline, data model, interfaces, guardrails |
| [architecture/](architecture/) | One page per subsystem: [locking](architecture/locking.md), [retrieval and keys](architecture/retrieval.md), [sync](architecture/sync.md), [dashboard control](architecture/dashboard-control.md), [identity](architecture/identity.md) |
| [decisions](decisions.md) | Settled choices and why |
| [roadmap](roadmap.md) | Open work, gates, risks, undecided questions |
| [../CHANGELOG.md](../CHANGELOG.md) | What shipped |
| [specs/](specs/) | Feature specs: [rooms](specs/rooms.md), [continuity](specs/continuity.md), [library](specs/library.md), [dashboard UI](specs/dashboard-ui.md), [onboarding tour](specs/onboarding-tour.md) |
| [reference/chat-formats/](reference/chat-formats/) | On-disk chat formats of each agent, for the transcript readers |
| [research/](research/) | Evidence and analyses behind the decisions (including [competitive PM strategy vs. `claude-mem` & `engram`](research/2026-10-22_2146_competitive_pm_strategy.md)) |
| [../tests/README.md](../tests/README.md) | How to run the tests; live-agent checks in `tests/manual/` |

Rules for these docs: update them in place; keep counts (MCP tools, tests) in sync with the code; one topic per file; no status diaries (shipped work goes to the changelog, open work to the roadmap).
