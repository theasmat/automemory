#!/usr/bin/env python3
"""Y5: an agent's MCP server replaces itself with a newly installed automemory between two requests (same process, same pipes): the request that notices the update
is already answered by the new binary, the agent's name and client survive, and a broken new file never cuts the agent off."""
import json
import os
import shutil
import subprocess
import sys
import time
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parents[1] / "lib"))
from harness import Report, Sandbox  # noqa: E402

IS_UNIX = os.name == "posix"


def rpc(proc, id_, method, params=None):
    proc.stdin.write((json.dumps({"jsonrpc": "2.0", "id": id_, "method": method, "params": params or {}}) + "\n").encode())
    proc.stdin.flush()
    return json.loads(proc.stdout.readline())


def main():
    rep = Report("mcp self-restart")
    if not IS_UNIX:
        print("  (restart is Unix only)")
        rep.finish()
        return
    sb = Sandbox("restart")
    sb.env.update({"AUTOMEMORY_SECRETS_BACKEND": "file", "AUTOMEMORY_KEY": "restart-master"})
    try:
        bindir = sb.root / "bin"
        bindir.mkdir()
        exe = bindir / "automemory"
        shutil.copy2(sb.bin, exe)
        sb.mcp("project.register", slug="app", display_name="app", path=str(sb.project.resolve()))
        proc = subprocess.Popen([str(exe), "mcp", "--agent", "claude"], stdin=subprocess.PIPE, stdout=subprocess.PIPE, stderr=subprocess.PIPE, cwd=sb.project, env=sb.env)
        pid = proc.pid
        rpc(proc, 1, "initialize", {"protocolVersion": "2024-11-05", "capabilities": {}, "clientInfo": {"name": "claude-code", "version": "2"}})
        r = rpc(proc, 2, "tools/call", {"name": "decision.record", "arguments": {"topic": "t1", "decision": "before the update", "reason": "r", "agent": "codex"}})
        rep.check("error" not in r, "the server answers before the update", str(r)[:120])

        # "Install" a new version: a replaced file at the same path.
        new = bindir / "automemory.new"
        shutil.copy2(sb.bin, new)
        os.replace(new, exe)
        time.sleep(0.2)

        r = rpc(proc, 3, "tools/call", {"name": "decision.record", "arguments": {"topic": "t2", "decision": "after the update", "reason": "r", "agent": "codex"}})
        rep.check("error" not in r and proc.poll() is None, "the request that notices the update is still answered, by the same process", str(r)[:160])
        rep.check(proc.pid == pid, "same process id (it replaced itself, the agent's pipes stay open)")
        db = sb.home / ".automemory" / "automemory.db"
        import sqlite3
        rows = dict(sqlite3.connect(db).execute("select topic, agent from decisions").fetchall())
        rep.check(rows.get("t1") == "claude" and rows.get("t2") == "claude", "the name `claude` survived the restart (both decisions signed claude)", str(rows))
        proc.stdin.close()
        err = proc.stderr.read().decode()
        proc.wait(timeout=10)
        rep.check("is restarting" in err and "1 of 5" in err, "the restart is noted on stderr, once", err[-200:])

        # A broken new file: the agent keeps its running server.
        proc = subprocess.Popen([str(exe), "mcp", "--agent", "claude"], stdin=subprocess.PIPE, stdout=subprocess.PIPE, stderr=subprocess.PIPE, cwd=sb.project, env=sb.env)
        rpc(proc, 1, "initialize", {"protocolVersion": "2024-11-05", "capabilities": {}, "clientInfo": {"name": "claude-code", "version": "2"}})
        broken = bindir / "automemory.broken"
        broken.write_text("#!/bin/sh\nexit 1\n")
        broken.chmod(0o755)
        os.replace(broken, exe)
        r = rpc(proc, 2, "ping")
        rep.check(r.get("result") == {} and proc.poll() is None, "a new file that does not run is ignored: the server carries on")
        proc.stdin.close()
        err = proc.stderr.read().decode()
        proc.wait(timeout=10)
        rep.check("is restarting" not in err, "and does not claim to have restarted")
    finally:
        sb.cleanup()
    rep.finish()


if __name__ == "__main__":
    main()
