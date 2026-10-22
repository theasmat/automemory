# AutoMemory

Shared, verified memory, locks and hand-offs for coding agents. Pure Rust, single binary, local SQLite.

```bash
curl -fsSL https://raw.githubusercontent.com/theasmat/automemory-bin/main/install.sh | bash
automemory connect claude -g   # also: codex, agy
automemory doctor
automemory serve --port 4545
```

Docs: [docs/](docs/README.md) (features, architecture, decisions, roadmap, specs) · [CHANGELOG](CHANGELOG.md) · [tests](tests/README.md) · agents: [AGENTS.md](AGENTS.md).

License: MIT OR Apache-2.0.
