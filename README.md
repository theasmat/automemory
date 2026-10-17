# AutoMemory

Shared, verified memory, locks and handoffs for coding agents. Pure Rust, single binary, local SQLite.

```bash
curl -fsSL https://raw.githubusercontent.com/theasmat/automemory-bin/main/install.sh | bash
automemory connect claude -g   # also: codex, agy
automemory doctor
automemory serve --port 4545
```

Docs live in [`docs/`](docs/): [Architecture](docs/ARCHITECTURE.md) · [Features](docs/FEATURES.md) · [Plan](docs/PLAN.md) · [Tasks](docs/TASKS.md).

License: MIT OR Apache-2.0.
