#!/usr/bin/env python3
"""Registration hygiene and hook robustness, end to end through the real binary.

* connect for Antigravity (workspace + global), Claude Code and Codex, then `disconnect all`
  leaves nothing registered and keeps the user's own files.
* A hook registered twice answers once (the second call replays the first reply).
* A hook never fails: a corrupt database, empty or garbage input still produce one valid reply
  in the agent's schema and exit code 0.
"""
import json
import sqlite3
import subprocess
import sys
import threading
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parents[1] / "lib"))
from harness import Report, Sandbox  # noqa: E402


def main():
    sb = Sandbox("registration")
    rep = Report("registration hygiene and hook robustness")
    try:
        home, proj = sb.home, sb.project
        # User content that must survive.
        (proj / "AGENTS.md").write_text("# my notes\n")
        hook = proj / ".git" / "hooks" / "post-commit"
        hook.parent.mkdir(parents=True, exist_ok=True)
        hook.write_text("#!/bin/sh\necho user hook\n")
        (home / ".gemini" / "config").mkdir(parents=True)
        (home / ".gemini" / "config" / "hooks.json").write_text('{"mine": {"enabled": true}}')

        for agent in ("agy", "claude", "codex"):
            sb.run("connect", agent)
            sb.run("connect", agent, "-g")
        listed = sb.run("disconnect", "all", "--everywhere", "--dry-run").stdout
        rep.check("registration(s) found" in listed and "0 AutoMemory" not in listed,
                  "connect registered things", listed[-300:])
        gl = json.loads((home / ".gemini/config/hooks.json").read_text())
        rep.check("automemory" in gl and gl["mine"]["enabled"] is True, "Antigravity global hooks added beside the user's")
        rep.check(not (proj / ".agents/hooks.json").exists(),
                  "workspace connect after global adds no second copy of the Antigravity hooks")
        doctor = sb.run("doctor").stdout
        rep.check("Registrations:" in doctor, "doctor reports registrations", doctor[-300:])

        sb.run("disconnect", "all", "--everywhere")
        left = sb.run("disconnect", "all", "--everywhere", "--dry-run").stdout
        rep.check("0 AutoMemory registration(s) found" in left, "disconnect all --everywhere leaves nothing", left[-400:])
        rep.check((proj / "AGENTS.md").read_text() == "# my notes\n", "user's AGENTS.md untouched")
        rep.check(hook.read_text() == "#!/bin/sh\necho user hook\n", "user's git post-commit hook restored exactly")
        gl = json.loads((home / ".gemini/config/hooks.json").read_text())
        rep.check("automemory" not in gl and gl["mine"]["enabled"] is True, "user's Antigravity hook kept")

        # A hook registered twice answers once.
        payload = {"conversationId": "dup1", "stepIdx": 7,
                   "toolCall": {"name": "run_command", "args": {"CommandLine": "ls"}}}
        outs = []

        def call():
            _, raw, _ = sb.hook("agy", "pre-tool", payload)
            outs.append(raw)

        threads = [threading.Thread(target=call) for _ in range(2)]
        [t.start() for t in threads]
        [t.join() for t in threads]
        rep.check(len(outs) == 2 and outs[0] == outs[1] and json.loads(outs[0]).get("decision"),
                  "two identical calls get the same reply", str(outs))
        db = sqlite3.connect(home / ".automemory" / "automemory.db")
        n = db.execute("select count(*) from tool_events where session_id='dup1' and event_type like '%pre-tool'").fetchone()[0]
        rep.check(n == 1, "and the event is recorded once", f"recorded {n}")
        db.close()

        # A hook never fails.
        for label, stdin in (("empty input", ""), ("garbage input", "not json {{"), ("a JSON array", "[1,2]")):
            r = sb.run("hook", "--agent", "agy", "--event", "pre-tool", "--project", "auto", stdin=stdin)
            ok = r.returncode == 0 and len(r.stdout.strip().splitlines()) == 1 and "decision" in json.loads(r.stdout)
            rep.check(ok, f"agy pre-tool survives {label}", f"{r.returncode} {r.stdout!r} {r.stderr[-120:]!r}")

        (home / ".automemory" / "automemory.db").write_bytes(b"this is not a database" * 100)
        for agent, event, want in (("agy", "pre-tool", {"decision": "allow"}), ("agy", "stop", {"decision": "allow"}),
                                   ("agy", "post-tool", {}), ("claude", "pre-tool", {})):
            r = sb.run("hook", "--agent", agent, "--event", event, "--project", "auto", stdin="{}")
            try:
                got = json.loads(r.stdout)
            except ValueError:
                got = r.stdout
            rep.check(r.returncode == 0 and got == want, f"{agent} {event} with a corrupt database fails open",
                      f"{r.returncode} {r.stdout!r} {r.stderr[-120:]!r}")
    finally:
        sb.cleanup()
    return rep.finish()


if __name__ == "__main__":
    sys.exit(main())
