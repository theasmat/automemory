#!/usr/bin/env python3
"""Every agent starts from the same verified knowledge, through the files they already read.

One project, three agents (Claude, Cursor, Gemini): a decision and a human note are recorded once;
`export-rules` puts them into each agent's instruction file without touching the user's own text;
a session start refreshes files that already carry the block; disconnect removes only the block.
"""
import json
import subprocess
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
        # Only decisions a person recorded travel into the instruction files. This script has no terminal, so the person proves themselves
        # with the password; without it the same command is recorded as written by "shell" and stays out.
        s.env["AUTOMEMORY_PASSWORD"] = ""
        r = s.run("decision", "record", "--topic", "shellside", "--decision", "Written from a script", "--reason", "no terminal, no password")
        rep.check(r.returncode == 0 and "recorded as written by '" in r.stderr and "recorded as written by 'human'" not in r.stderr, "without a terminal or password a decision is recorded under the agent's name (or 'shell'), not as human", r.stdout + r.stderr)
        subprocess.run([s.bin, "rule", "passwd", "--from-env"], env={**s.env, "AUTOMEMORY_PASSWORD": "pw one"}, cwd=s.project, capture_output=True, text=True)
        s.env["AUTOMEMORY_PASSWORD"] = "pw one"
        r = s.run("decision", "record", "--topic", "payments", "--decision", "Use Stripe webhooks", "--reason", "polling missed events")
        rep.check(r.returncode == 0 and "recorded as written by" not in r.stderr, "with the human password it is recorded as a person's decision", r.stdout + r.stderr)
        # An agent copies `--project auto` from the hook commands onto this one: it must mean "this folder", not a project named auto.
        r = s.run("decision", "record", "--topic", "dupe", "--decision", "first", "--reason", "x", "--project", "auto")
        r = s.run("decision", "record", "--topic", "dupe", "--decision", "second", "--reason", "y", "--project", "auto")
        import sqlite3
        con = sqlite3.connect(s.home / ".automemory" / "automemory.db")
        rows = con.execute("select project_id, status from decisions where topic = 'dupe' order by created_at").fetchall()
        con.close()
        rep.check(len(rows) == 2 and all(p not in ("auto", None) for p, _ in rows) and [st for _, st in rows] == ["superseded", "active"],
                  "`--project auto` means this folder's project, so the second decision supersedes the first", str(rows))
        r = s.run("note", "Always run pnpm, never npm", "-t", "preference")
        rep.check(r.returncode == 0 and "Noted" in r.stdout, "a human note is saved", r.stdout + r.stderr)

        r = s.run("export-rules")
        rep.check(r.returncode == 0, "export-rules runs", r.stdout + r.stderr)
        agents = (p / "AGENTS.md").read_text()
        rep.check(agents.startswith("# Team rules\n\nUse tabs.") and "Use Stripe webhooks" in agents, "AGENTS.md keeps the user's text and gains the decision", agents[:300])
        rep.check("pnpm" in agents, "and the human note", agents)
        rep.check("Written from a script" not in agents, "a decision recorded as 'shell' is not exported as a person's")
        for f in ("CLAUDE.md", "GEMINI.md", ".cursor/rules/automemory.mdc"):
            t = (p / f).read_text() if (p / f).exists() else ""
            rep.check("Use Stripe webhooks" in t, f"{f} created for a detected agent", t[:200])
        rep.check("alwaysApply: true" in (p / ".cursor/rules/automemory.mdc").read_text(), "Cursor rule has front matter")
        rep.check(not (p / ".windsurf").exists() and not (p / ".kiro").exists(), "agents that are not used get no files")

        before = {f: (p / f).read_text() for f in ("AGENTS.md", "CLAUDE.md")}
        s.run("export-rules")
        rep.check(before == {f: (p / f).read_text() for f in before}, "a second export changes nothing")
        rep.check(s.run("export-rules", "--check").returncode == 0, "--check passes when current")

        # Recording a decision at the terminal refreshes the files that already carry the block at once.
        s.run("decision", "record", "--topic", "auth", "--decision", "JWT in httpOnly cookie", "--reason", "XSS")
        rep.check(s.run("export-rules", "--check").returncode == 0, "--check passes: the terminal refreshed the files itself")
        rep.check("httpOnly cookie" in (p / "AGENTS.md").read_text(), "and the new decision is in AGENTS.md")
        # A block that went stale some other way (a git pull, a hand edit) is refreshed by the next session start, and never creates new files.
        agents = (p / "AGENTS.md").read_text()
        (p / "AGENTS.md").write_text(agents.replace("JWT in httpOnly cookie", "OUT OF DATE"))
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
