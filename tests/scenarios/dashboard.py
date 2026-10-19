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
    sb.env.update({"AUTOMEMORY_SECRETS_BACKEND": "file", "AUTOMEMORY_KEY": "dash-master"})
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

        # Recall settings: profiles and a write-only key.
        code, body = http(port, "/api/recall")
        rc = json.loads(body) if code == 200 else {}
        rep.check(code == 200 and rc.get("mode") == "keyword" and len(rc.get("presets", [])) >= 8, "GET /api/recall lists mode and presets", f"{code} {body[:120]}")
        code, body = http(port, "/api/recall/profile", "POST", {}, {"name": "cloud", "preset": "openai-small", "allow_network": True})
        rep.check(code == 200, "a cloud profile can be saved", f"{code} {body[:120]}")
        planted = "sk-PLANTED-dashboard-5544"
        code, body = http(port, "/api/recall/key", "PUT", {}, {"profile": "cloud", "key": planted})
        rep.check(code == 200 and planted not in body, "the key is stored and not echoed", f"{code} {body[:120]}")
        code, body = http(port, "/api/recall")
        rc = json.loads(body)
        cloud = next((p for p in rc["profiles"] if p["name"] == "cloud"), {})
        rep.check(cloud.get("key", {}).get("set") is True and planted not in body, "GET /api/recall shows the key as set and never returns it", body[:200])
        code, body = http(port, "/api/recall", "PUT", {}, {"active": "cloud", "mode": "hybrid"})
        code, body = http(port, "/api/recall")
        rc = json.loads(body)
        rep.check(rc.get("active") == "cloud" and rc.get("mode") == "hybrid", "the active profile and mode can be changed", body[:160])
        leaked = [str(f) for f in sb.home.rglob("*") if f.is_file() and planted.encode() in f.read_bytes()]
        rep.check(not leaked, "the key is in no file under the home folder", str(leaked))
        code, body = http(port, "/api/recall/key?profile=cloud", "DELETE", {})
        code, body = http(port, "/api/recall")
        rep.check(not next(p for p in json.loads(body)["profiles"] if p["name"] == "cloud")["key"]["set"], "the key can be removed")

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

        # Guessing is throttled: five wrong passwords lock the gate, even for the right one, for a minute.
        for _ in range(5):
            code, _ = http(port, "/api/proposals/decide_all", "POST",
                           {"X-AutoMemory-Password": "wrong guess"}, {"approved": False})
            rep.check(code == 403, "a wrong password is refused", str(code))
        code, _ = http(port, "/api/proposals/decide_all", "POST",
                       {"X-AutoMemory-Password": "correct horse"}, {"approved": False})
        rep.check(code == 429, "after five wrong guesses even the right password waits", str(code))
    finally:
        server.terminate()
        server.wait(timeout=5)
        sb.cleanup()
    return rep.finish()


if __name__ == "__main__":
    sys.exit(main())
