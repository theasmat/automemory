#!/usr/bin/env python3
"""`connect` keeps the guidelines in an AutoMemory-written CLAUDE.md current (found in trial T11: new guidance never reached
existing projects), never touches a CLAUDE.md the user wrote, keeps the managed memory block, and is idempotent."""
import sys
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parents[1] / "lib"))
from harness import Report, Sandbox  # noqa: E402

OLD = """# AutoMemory Multi-Agent Coordination Guidelines

This workspace is integrated with **AutoMemory**.
- **Handoffs**: Pass context to peer workers using `baton.pass`. (an older version of this text)

<!-- automemory:begin (generated; change it with `automemory note` / `automemory decision`) -->
## Project memory (AutoMemory)
- **storage**: keep orders in SQLite
<!-- automemory:end -->
"""


def main():
    rep = Report("guideline refresh on connect")
    sb = Sandbox("guidelines")
    md = sb.project / "CLAUDE.md"
    try:
        r = sb.run("connect", "claude")
        rep.check(r.returncode == 0 and md.exists() and "**Planning rooms**" in md.read_text(), "a new CLAUDE.md gets the current guidelines", (r.stdout + r.stderr)[-200:])

        md.write_text(OLD)
        sb.run("connect", "claude")
        text = md.read_text()
        rep.check("**Planning rooms**" in text and "an older version of this text" not in text, "an old AutoMemory CLAUDE.md is brought up to date", text[:300])
        rep.check("keep orders in SQLite" in text and text.count("automemory:begin") == 1 and text.count("automemory:end") == 1, "the managed memory block survives, once")
        before = md.read_text()
        sb.run("connect", "claude")
        rep.check(md.read_text() == before, "a second connect changes nothing")

        mine = "# My project\n\nUse tabs. Never use the word 'synergy'.\n"
        md.write_text(mine)
        sb.run("connect", "claude")
        rep.check(md.read_text() == mine, "a CLAUDE.md the user wrote is left exactly as it is")
    finally:
        sb.cleanup()
    return rep.finish()


if __name__ == "__main__":
    sys.exit(main())
