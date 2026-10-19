#!/usr/bin/env python3
"""Every agent that can be connected: connect leaves a registration, disconnect removes exactly it, and the agents
with an instruction-file target receive the managed block and lose it again. Hook-level behaviour needs recorded
payloads (tests/contract/fixtures); an agent without them stays labelled experimental.
"""
import sys
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parents[1] / "lib"))
from harness import Report, Sandbox  # noqa: E402

AGENTS = ["claude", "agy", "codex", "cursor", "windsurf", "gemini-cli", "copilot", "cline", "roo", "continue", "zed", "opencode", "goose"]
TARGET = {"cursor": ".cursor/rules/automemory.mdc", "windsurf": ".windsurf/rules/automemory.md", "gemini-cli": "GEMINI.md",
          "copilot": ".github/copilot-instructions.md", "cline": ".clinerules/automemory.md", "roo": ".roo/rules/automemory.md"}


def main():
    rep = Report("agent matrix")
    s = Sandbox("matrix")
    try:
        s.mcp("project.register", slug="app", display_name="app", path=str(s.project))
        s.run("note", "Always run pnpm, never npm")
        for a in AGENTS:
            r = s.run("connect", a)
            rep.check(r.returncode == 0, f"{a}: connect succeeds", (r.stdout + r.stderr)[-200:])
            found = s.run("disconnect", a, "--dry-run").stdout
            rep.check("0 AutoMemory registration" not in found, f"{a}: connect left a registration to remove", found[-120:])
            if a in TARGET:
                r = s.run("export-rules", "--agent", a)
                f = s.project / TARGET[a]
                rep.check(f.exists() and "automemory:begin" in f.read_text() and "pnpm" in f.read_text(), f"{a}: instruction file written with the note", r.stdout[-160:])
            r = s.run("disconnect", a)
            rep.check(r.returncode == 0, f"{a}: disconnect succeeds", (r.stdout + r.stderr)[-160:])
            after = s.run("disconnect", a, "--dry-run").stdout
            rep.check("0 AutoMemory registration" in after, f"{a}: nothing left after disconnect", after[-120:])
    finally:
        s.cleanup()
    return rep.finish()


if __name__ == "__main__":
    sys.exit(main())
