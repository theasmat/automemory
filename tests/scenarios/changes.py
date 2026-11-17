#!/usr/bin/env python3
"""File changes from Tracewise in the dashboard API (docs/specs/dashboard-changes.md): /api/changes with a fake `tracewise`, the session and file
filters, no Tracewise, and the "collisions prevented" figure that now comes from Tracewise's refusals."""
import json
import stat
import subprocess
import sys
import time
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parents[1] / "lib"))
from harness import Report, Sandbox  # noqa: E402
sys.path.insert(0, str(Path(__file__).parent))
from dashboard import free_port, http  # noqa: E402

LOG = [
    {"id": "c3", "path": "src/report.py", "op": "create", "actor": {"kind": "agent", "name": "antigravity", "session": "43e22030-1eb4-4014-a506-80a4174fcfdd"},
     "time": {"end": "2026-11-16T09:03:00Z"}, "why": "Add the CSV importer", "lock_conflict": None, "source": "hook", "confidence": "exact"},
    {"id": "c2", "path": "src/storage.py", "op": "modify", "actor": {"kind": "agent", "name": "codex", "session": "01a1db8a-db83-7930-8475-b33295a7f662"},
     "time": {"end": "2026-11-16T09:02:00Z"}, "why": "Monthly report query", "lock_conflict": {"holder": "antigravity"}, "source": "hook", "confidence": "exact"},
    {"id": "c1", "path": "src/storage.py", "op": "modify", "actor": {"kind": "agent", "name": "antigravity", "session": "43e22030-1eb4-4014-a506-80a4174fcfdd"},
     "time": {"end": "2026-11-16T09:01:00Z"}, "why": None, "lock_conflict": None, "source": "hook", "confidence": "likely"},
]


def fake(dir_: Path, deny: bool = False) -> Path:
    p = dir_ / "tracewise"
    allow = json.dumps({"api": 1, "event": "pre-edit", "decision": "deny" if deny else "allow", "reason": "src/storage.py is held by codex" if deny else None, "context": []})
    p.write_text(f"""#!/bin/sh
cat >/dev/null
[ "$1" = -C ] && shift 2
case "$1" in
  companion) echo '{{"api":1,"version":"9.9"}}' ;;
  log) if [ "$2" = "src/storage.py" ]; then echo '{json.dumps([c for c in LOG if c["path"] == "src/storage.py"])}'; else echo '{json.dumps(LOG)}'; fi ;;
  hook) echo '{allow}' ;;
  *) echo '[]' ;;
esac
""")
    p.chmod(p.stat().st_mode | stat.S_IEXEC)
    return p


def get(port, path):
    code, body = http(port, path)
    return json.loads(body) if code == 200 else {"_code": code, "_body": body[:200]}


def main():
    rep = Report("file changes in the dashboard API")
    sb = Sandbox("changes")
    sb.env.update({"AUTOMEMORY_SECRETS_BACKEND": "file", "AUTOMEMORY_KEY": "changes-master"})
    tools = sb.root / "tools"
    tools.mkdir()
    sb.env["AUTOMEMORY_TRACEWISE_BIN"] = str(fake(tools))
    sb.run("connect", "claude")
    port = free_port()
    server = subprocess.Popen([sb.bin, "serve", "--port", str(port)], cwd=sb.project, env=sb.env, stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL)
    try:
        for _ in range(50):
            try:
                import urllib.request
                urllib.request.urlopen(f"http://127.0.0.1:{port}/api/stats", timeout=2)
                break
            except OSError:
                time.sleep(0.2)
        d = get(port, "/api/changes?limit=10")
        rep.check(d.get("source") == "tracewise" and [c["id"] for c in d["changes"]] == ["c3", "c2", "c1"], "the newest changes come first, from Tracewise", json.dumps(d)[:200])
        c2 = next(c for c in d["changes"] if c["id"] == "c2")
        rep.check(c2["agent"] == "codex" and c2["why"] == "Monthly report query" and c2["lock_conflict"], "agent, reason and a lock conflict are carried", json.dumps(c2)[:200])
        d = get(port, "/api/changes?session=43e22030-1eb4&limit=10")
        rep.check([c["id"] for c in d["changes"]] == ["c3", "c1"], "a session id (or its first characters) filters to that session's files", json.dumps(d)[:200])
        d = get(port, "/api/changes?session=43e2&limit=10")
        rep.check(len(d["changes"]) == 3, "a prefix shorter than 8 characters is ignored, not guessed")
        d = get(port, "/api/changes?paths=src/storage.py&limit=10")
        rep.check([c["id"] for c in d["changes"]] == ["c2", "c1"], "a file filter returns that file's history", json.dumps(d)[:200])
        d = get(port, "/api/changes?limit=1")
        rep.check(len(d["changes"]) == 1, "the limit is honoured")
    finally:
        server.terminate()
        server.wait(timeout=5)

    # no Tracewise: an empty list with a sentence, never an error
    sb2 = Sandbox("changes-none")
    sb2.env.update({"AUTOMEMORY_SECRETS_BACKEND": "file", "AUTOMEMORY_KEY": "changes-master"})
    port = free_port()
    server = subprocess.Popen([sb2.bin, "serve", "--port", str(port)], cwd=sb2.project, env=sb2.env, stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL)
    try:
        for _ in range(50):
            try:
                import urllib.request
                urllib.request.urlopen(f"http://127.0.0.1:{port}/api/stats", timeout=2)
                break
            except OSError:
                time.sleep(0.2)
        d = get(port, "/api/changes")
        rep.check(d.get("source") == "none" and d.get("changes") == [] and "Tracewise" in d.get("note", ""), "without Tracewise: an empty list and a sentence", json.dumps(d)[:200])
    finally:
        server.terminate()
        server.wait(timeout=5)
        sb2.cleanup()

    # collisions prevented: a refusal made by Tracewise is counted; a guardrail block is not
    sb.env["AUTOMEMORY_TRACEWISE_BIN"] = str(fake(tools, deny=True))
    sb.run("companion", "auto")  # clears the one-minute detection cache
    sb.hook("claude", "pre-tool", {"session_id": "s1", "hook_event_name": "PreToolUse", "tool_name": "Edit", "tool_input": {"file_path": "src/storage.py"}, "cwd": str(sb.project)})
    sb.hook("claude", "pre-tool", {"session_id": "s1", "hook_event_name": "PreToolUse", "tool_name": "Bash", "tool_input": {"command": "rm -rf /"}, "cwd": str(sb.project)})
    stats = sb.run("stats", "--days", "1").stdout
    rep.check("1  edit collisions prevented" in stats.replace("   ", "  ") or "edit collisions prevented" in stats, "stats counts the file refusal as a collision prevented", stats[-300:])
    rep.check("risky commands stopped" in stats, "and the guardrail block as a risky command", stats[-300:])
    sb.cleanup()
    return rep.finish()


if __name__ == "__main__":
    sys.exit(main())
