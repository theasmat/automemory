#!/usr/bin/env python3
"""The human password gate: an agent with shell or MCP access cannot do human-only actions.

Without a password everything works (fresh installs stay usable). With one set, agents get
refused over MCP, and the CLI refuses piped stdin (how agents invoke it) but accepts the
password from the environment.
"""
import subprocess
import sys
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parents[1] / "lib"))
from harness import Report, Sandbox  # noqa: E402


def main():
    sb = Sandbox("humangate")
    rep = Report("human password gate")
    try:
        r = sb.run("proposal", "approve-all")
        rep.check(r.returncode == 0, "no password set: approve-all works", r.stderr[-160:])
        out = sb.mcp("memory.proposals.decide_all", approved=False)
        rep.check("error" not in str(out).lower(), "no password set: MCP decide_all works", str(out)[:160])

        env = {**sb.env, "AUTOMEMORY_PASSWORD": "correct horse"}
        subprocess.run([sb.bin, "rule", "passwd", "--from-env"], env=env, cwd=sb.project,
                       capture_output=True, text=True)

        out = sb.mcp("memory.proposals.decide_all", approved=True)
        rep.check("human" in str(out).lower(), "password set: MCP decide_all is refused", str(out)[:160])
        out = sb.mcp("memory.proposals.decide", id="x", approved=True)
        rep.check("human" in str(out).lower(), "password set: MCP decide is refused", str(out)[:160])

        r = sb.run("proposal", "approve-all", stdin="")  # no tty, no env password: how an agent calls it
        rep.check(r.returncode != 0, "password set: CLI with piped stdin fails closed", r.stdout[-160:])
        r = subprocess.run([sb.bin, "proposal", "approve-all"], env=env, cwd=sb.project,
                           capture_output=True, text=True, stdin=subprocess.DEVNULL, timeout=60)
        rep.check(r.returncode == 0, "password set: CLI with the env password works", r.stderr[-160:])
        r = sb.run("rule", "add", "--name", "x", "--pattern", "y", stdin="")
        rep.check(r.returncode != 0, "password set: rule add fails closed for agents", r.stdout[-160:])
    finally:
        sb.cleanup()
    return rep.finish()


if __name__ == "__main__":
    sys.exit(main())
