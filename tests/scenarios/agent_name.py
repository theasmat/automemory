#!/usr/bin/env python3
"""Who wrote it: a command run from an agent's shell is named after the agent (found from the process chain), never `human`; a person is proven
by a terminal or the password. Nothing here types a name."""
import sqlite3
import subprocess
import sys
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parents[1] / "lib"))
from harness import Report, Sandbox  # noqa: E402


def main():
    rep = Report("agent name from the process chain")
    sb = Sandbox("agentname")
    for var in ("CLAUDECODE", "CLAUDE_CODE_SESSION_ID", "CODEX_SANDBOX", "CODEX_THREAD_ID", "CODEX_SANDBOX_NETWORK_DISABLED", "GEMINI_CLI"):
        sb.env.pop(var, None)
    sb.env["AUTOMEMORY_PASSWORD"] = ""
    try:
        root = sb.project.resolve()
        sb.mcp("project.register", slug="shop", display_name="shop", path=str(root))

        def via_agent(frag, *args, env=None):
            """Runs automemory under a shell whose command line names an agent, the way an agent's shell tool does (stdin is not a terminal)."""
            cmd = " ".join(["'%s'" % sb.bin, *["'%s'" % a for a in args]]) + " ; :"
            return subprocess.run(["/bin/sh", "-c", cmd, frag], capture_output=True, text=True, cwd=sb.project, env=env or sb.env, stdin=subprocess.DEVNULL, timeout=60)

        db = lambda: sqlite3.connect(sb.home / ".automemory" / "automemory.db")  # noqa: E731

        r = via_agent("/opt/tools/codex/bin/codex", "decision", "record", "--topic", "storage", "--decision", "SQLite", "--reason", "concurrent writes")
        who = db().execute("select agent from decisions where topic = 'storage'").fetchone()
        rep.check(who and who[0] == "codex", "a decision run under a codex process is written by codex", f"{who} {r.stderr}")
        rep.check("recorded as written by 'codex'" in r.stderr, "and says so")

        r = via_agent("/Applications/Antigravity.app/Contents/MacOS/Antigravity", "decision", "record", "--topic", "cli", "--decision", "argparse", "--reason", "stdlib")
        who = db().execute("select agent from decisions where topic = 'cli'").fetchone()
        rep.check(who and who[0] == "agy", "under an Antigravity process it is agy", f"{who} {r.stderr}")

        # A note from an agent's shell is an unverified claim by that agent, not a person's trusted text.
        r = via_agent("/opt/tools/codex/bin/codex", "note", "Never run the seed script on staging", "-t", "warning")
        row = db().execute("select source_kind, confidence, json_extract(metadata, '$.saved_by') from memories where summary like 'Never run the seed%'").fetchone()
        rep.check(row and row[0] == "agent" and row[1] < 0.95 and row[2] == "codex", "a note from an agent's shell is an unverified note written by that agent", f"{row} {r.stderr}")
        # Antigravity runs its shell commands in a pseudo-terminal: a terminal on stdin must not turn the agent into a person
        import os
        import pty
        master, slave = pty.openpty()
        cmd = "'%s' note 'Reports sorted by customer name' ; :" % sb.bin
        subprocess.run(["/bin/sh", "-c", cmd, "/Applications/Antigravity.app/Contents/MacOS/Antigravity"], stdin=slave, capture_output=True, text=True, cwd=sb.project, env=sb.env, timeout=60)
        os.close(slave)
        os.close(master)
        row = db().execute("select source_kind, json_extract(metadata, '$.saved_by') from memories where summary like 'Reports sorted by customer%'").fetchone()
        rep.check(row and row[0] == "agent" and row[1] == "agy", "a note from Antigravity's pseudo-terminal is agy's, not a person's", str(row))
        (sb.project / "AGENTS.md").write_text("# Rules\n")
        sb.run("export-rules")
        rep.check("seed script" not in (sb.project / "AGENTS.md").read_text(), "and it is not exported into the instruction files")

        # A person: the password in the environment (the test has no terminal) makes the same commands a person's.
        subprocess.run([sb.bin, "rule", "passwd", "--from-env"], env={**sb.env, "AUTOMEMORY_PASSWORD": "pw one"}, cwd=sb.project, capture_output=True, text=True)
        env = {**sb.env, "AUTOMEMORY_PASSWORD": "pw one"}
        via_agent("/opt/tools/codex/bin/codex", "decision", "record", "--topic", "human-one", "--decision", "Tabs", "--reason", "team style", env=env)
        who = db().execute("select agent from decisions where topic = 'human-one'").fetchone()
        rep.check(who and who[0] == "human", "with the human password it is a person's, even under an agent process", str(who))
        via_agent("/opt/tools/codex/bin/codex", "note", "Always run pnpm, never npm", "-t", "preference", env=env)
        row = db().execute("select source_kind, confidence from memories where summary like 'Always run pnpm%'").fetchone()
        rep.check(row and row[0] == "user" and row[1] == 1.0, "and so is a note", str(row))
    finally:
        sb.cleanup()
    return rep.finish()


if __name__ == "__main__":
    sys.exit(main())
