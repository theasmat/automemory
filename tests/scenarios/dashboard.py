#!/usr/bin/env python3
"""Starts `automemory serve` in a sandbox and exercises the dashboard API.

Checks: the UI is served, read endpoints answer, live leases appear on /api/coordination,
foreign Origin and rebinding Host headers get 403, and with a human password set the
proposal-decision endpoints require it.
"""
import json
import socket
import subprocess
import sys
import time
import urllib.error
import urllib.request
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parents[1] / "lib"))
from harness import Report, Sandbox  # noqa: E402


def free_port():
    with socket.socket() as s:
        s.bind(("127.0.0.1", 0))
        return s.getsockname()[1]


def http(port, path, method="GET", headers=None, body=None):
    req = urllib.request.Request(f"http://127.0.0.1:{port}{path}", method=method,
                                 data=json.dumps(body).encode() if body is not None else None,
                                 headers={"Content-Type": "application/json", **(headers or {})})
    try:
        with urllib.request.urlopen(req, timeout=60) as r:  # password check is PBKDF2 (600k rounds): slow in debug builds
            return r.status, r.read().decode()
    except urllib.error.HTTPError as e:
        return e.code, e.read().decode()


def main():
    sb = Sandbox("dashboard")
    rep = Report("dashboard API")
    port = free_port()
    server = subprocess.Popen([sb.bin, "serve", "--port", str(port)], cwd=sb.project, env=sb.env,
                              stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL)
    try:
        for _ in range(50):
            try:
                http(port, "/api/stats")
                break
            except OSError:
                time.sleep(0.1)

        code, body = http(port, "/")
        rep.check(code == 200 and "<html" in body.lower(), "dashboard UI is served", f"{code}")
        code, body = http(port, "/api/stats")
        rep.check(code == 200 and "active_memories" in body, "GET /api/stats", f"{code} {body[:80]}")

        sb.mcp("lock.acquire", scope="src/live.rs", goal="dashboard test", agent="claude-code",
               session_id="dash1")
        code, body = http(port, "/api/coordination")
        data = json.loads(body) if code == 200 else {}
        rep.check(any(l.get("scope") == "src/live.rs" for l in data.get("leases", [])),
                  "/api/coordination lists the live lease", f"{code} {body[:160]}")
        rep.check({"leases", "rooms", "batons"} <= set(data), "/api/coordination has rooms and batons")

        code, _ = http(port, "/api/proposals/decide_all", "POST",
                       {"Origin": "https://evil.example"}, {"approved": True})
        rep.check(code == 403, "foreign Origin POST is rejected", str(code))
        code, _ = http(port, "/api/stats", headers={"Host": "evil.example"})
        rep.check(code == 403, "rebinding Host is rejected", str(code))

        # With a human password set, deciding proposals needs it.
        import os
        pw_env = {**sb.env, "AUTOMEMORY_PASSWORD": "correct horse"}
        subprocess.run([sb.bin, "rule", "passwd", "--from-env"], env=pw_env, cwd=sb.project,
                       capture_output=True, text=True)
        code, _ = http(port, "/api/proposals/decide_all", "POST", {}, {"approved": False})
        rep.check(code == 403, "decide_all without the password is rejected", str(code))
        code, _ = http(port, "/api/proposals/decide_all", "POST",
                       {"X-AutoMemory-Password": "correct horse"}, {"approved": False})
        rep.check(code == 200, "decide_all with the password works", str(code))
    finally:
        server.terminate()
        server.wait(timeout=5)
        sb.cleanup()
    return rep.finish()


if __name__ == "__main__":
    sys.exit(main())
