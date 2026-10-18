#!/usr/bin/env python3
"""Every agent starts from the same verified knowledge, through the files they already read.

One project, three agents (Claude, Cursor, Gemini): a decision and a human note are recorded once;
`export-rules` puts them into each agent's instruction file without touching the user's own text;
a session start refreshes files that already carry the block; disconnect removes only the block.
"""
import json
import sys
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parents[1] / "lib"))
from harness import Report, Sandbox  # noqa: E402


def main():
    rep = Report("shared knowledge through instruction files")
    s = Sandbox("rules")
    try:
        p = s.project
        for d in (".cursor", ".gemini", ".claude"):
            (p / d).mkdir()
        (p / "AGENTS.md").write_text("# Team rules\n\nUse tabs.\n")

        s.mcp("project.register", slug="shop", display_name="shop", path=str(p))
        s.mcp("decision.record", topic="payments", decision="Use Stripe webhooks", reason="polling missed events",
              project_id="shop", agent="claude")
        r = s.run("note", "Always run pnpm, never npm", "-t", "preference")
        rep.check(r.returncode == 0 and "Noted" in r.stdout, "a human note is saved", r.stdout + r.stderr)

        r = s.run("export-rules")
        rep.check(r.returncode == 0, "export-rules runs", r.stdout + r.stderr)
        agents = (p / "AGENTS.md").read_text()
        rep.check(agents.startswith("# Team rules\n\nUse tabs.") and "Use Stripe webhooks" in agents, "AGENTS.md keeps the user's text and gains the decision", agents[:300])
        rep.check("pnpm" in agents, "and the human note", agents)
        for f in ("CLAUDE.md", "GEMINI.md", ".cursor/rules/automemory.mdc"):
            t = (p / f).read_text() if (p / f).exists() else ""
            rep.check("Use Stripe webhooks" in t, f"{f} created for a detected agent", t[:200])
        rep.check("alwaysApply: true" in (p / ".cursor/rules/automemory.mdc").read_text(), "Cursor rule has front matter")
        rep.check(not (p / ".windsurf").exists() and not (p / ".kiro").exists(), "agents that are not used get no files")

        before = {f: (p / f).read_text() for f in ("AGENTS.md", "CLAUDE.md")}
        s.run("export-rules")
        rep.check(before == {f: (p / f).read_text() for f in before}, "a second export changes nothing")
        rep.check(s.run("export-rules", "--check").returncode == 0, "--check passes when current")

        # A new decision makes the files stale; a session start refreshes them (no new files).
        # No project_id: the MCP server runs in the project folder and must attach the decision to it.
        s.mcp("decision.record", topic="auth", decision="JWT in httpOnly cookie", reason="XSS", agent="claude")
        rep.check(s.run("export-rules", "--check").returncode != 0, "--check fails when out of date")
        s.hook("claude", "prompt", {"session_id": "r1", "prompt": "hello"})
        rep.check("httpOnly cookie" in (p / "AGENTS.md").read_text(), "a session start refreshes the block")
        rep.check(not (p / ".windsurf").exists(), "refresh never creates new files")

        # Unverified agent claims do not travel.
        s.mcp("memory.save", summary="agent hunch about caching", content="maybe", type="fact", scope="project", project_id="shop")
        s.run("export-rules")
        rep.check("hunch" not in (p / "AGENTS.md").read_text(), "unverified agent memories stay out")

        r = s.run("disconnect", "all")
        rep.check(r.returncode == 0, "disconnect all", r.stdout[-200:] + r.stderr)
        rep.check((p / "AGENTS.md").read_text().strip() == "# Team rules\n\nUse tabs.", "AGENTS.md is back to the user's text", (p / "AGENTS.md").read_text())
        rep.check(not (p / "CLAUDE.md").exists() and not (p / ".cursor/rules/automemory.mdc").exists(), "generated files are removed")
    finally:
        s.cleanup()
    return rep.finish()


if __name__ == "__main__":
    sys.exit(main())
