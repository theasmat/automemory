#!/usr/bin/env python3
"""The automatic backup flushes by itself: notes saved while agent turns end reach the git remote without any manual backup, including the last ones
(the soak test of Nov 2026 found that a turn end started no backup, so the notes of the last turn waited for the next prompt). A throttled request
schedules one catch-up pass instead of being dropped."""
import json
import re
import subprocess
import sys
import time
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parents[1] / "lib"))
from harness import Report, Sandbox  # noqa: E402


def git(cwd, *args):
    return subprocess.run(["git", *args], cwd=cwd, capture_output=True, text=True, check=True).stdout


def remote_notes(sb):
    r = sb.run("backup", "git", "verify")
    m = re.search(r"restores: (\d+) note", r.stdout + r.stderr)
    return int(m.group(1)) if m else -1


def main():
    rep = Report("the automatic backup flushes by itself")
    sb = Sandbox("flush")
    sb.env.update({"AUTOMEMORY_SECRETS_BACKEND": "file"})
    sb.env.pop("AUTOMEMORY_KEY", None)
    try:
        remote = sb.root / "remote.git"
        remote.mkdir()
        git(remote, "init", "-q", "--bare", "-b", "main")
        sb.mcp("project.register", slug="app", display_name="app", path=str(sb.project))
        r = sb.run("backup", "git", "setup", str(remote), "--yes")
        rep.check(r.returncode == 0, "backup is set up", (r.stdout + r.stderr)[-200:])
        n = 8
        for i in range(1, n + 1):
            sb.mcp("memory.save", summary=f"Flush note {i}", content=f"Note {i} for the flush test", type="fact", scope="project", project_id="app", agent="claude")
            sb.hook("claude", "stop", {"session_id": "f1", "hook_event_name": "Stop", "stop_hook_active": False, "cwd": str(sb.project)})
            time.sleep(3)  # faster than the 20 s window: most turn ends are throttled
        # no manual backup: the catch-up pass the throttled turn ends scheduled has to carry the rest
        deadline = time.time() + 60
        got = -1
        while time.time() < deadline:
            got = remote_notes(sb)
            if got >= n:
                break
            time.sleep(4)
        rep.check(got >= n, f"all {n} notes reach the remote without a manual backup, the last ones included", f"remote holds {got}")
        st = json.loads(sb.run("backup", "git", "status", "--json").stdout)
        rep.check(st["state"] == "ok" and not st["gaps"], "and the status is ok", json.dumps(st)[:200])
    finally:
        sb.cleanup()
    return rep.finish()


if __name__ == "__main__":
    sys.exit(main())
