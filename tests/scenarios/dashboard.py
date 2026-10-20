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

        # Tiers: Safe works without the password, Protected needs it, Terminal-only has no route.
        code, body = http(port, "/api/tools/backup", "POST", {}, {})
        rep.check(code == 200 and json.loads(body).get("started"), "Safe: backup starts without a password", f"{code} {body[:100]}")
        for _ in range(40):
            code, body = http(port, "/api/jobs")
            if any(j["kind"] == "backup" and j["state"] != "running" for j in json.loads(body)):
                break
            time.sleep(0.25)
        rep.check(any(j["kind"] == "backup" and j["state"] == "done" for j in json.loads(body)), "the job finishes and keeps its result", body[:200])
        code, body = http(port, "/api/policy")
        tiers = {r["path"]: r["tier"] for r in json.loads(body)}
        rep.check(tiers.get("/api/decisions") == "protected" and tiers.get("/api/tools/backup") == "safe", "GET /api/policy lists the tiers")
        code, _ = http(port, "/api/decisions", "POST", {}, {"topic": "t", "decision": "d"})
        rep.check(code == 403, "Protected: a decision is refused without the password", str(code))
        code, body = http(port, "/api/decisions", "POST", {"X-AutoMemory-Password": "correct horse"}, {"topic": "storage", "decision": "SQLite", "reason": "simple"})
        rep.check(code == 200, "Protected: a decision is recorded with the password", f"{code} {body[:100]}")
        code, body = http(port, "/api/decisions")
        rep.check(code == 200 and "storage" in body, "decisions can be listed", body[:100])
        mems = json.loads(http(port, "/api/memories")[1])
        if mems:
            mid = mems[0]["id"]
            for path in ("pin", "archive"):
                code, _ = http(port, f"/api/memories/{mid}/{path}", "POST", {}, {})
                rep.check(code == 403, f"Protected: {path} is refused without the password", str(code))
            code, _ = http(port, f"/api/memories/{mid}/update", "POST", {}, {"summary": "x"})
            rep.check(code == 403, "Protected: editing a memory is refused without the password", str(code))
        code, body = http(port, "/api/llm")
        lj = json.loads(body) if code == 200 else {}
        rep.check(code == 200 and lj.get("profiles", [{}])[0].get("name") == "default" and lj.get("on") is False, "GET /api/llm lists the default profile, off", f"{code} {body[:100]}")
        code, _ = http(port, "/api/llm", "PUT", {}, {"name": "cloud", "model": "m", "activate": True})
        rep.check(code == 403, "Protected: saving an LLM profile is refused without the password", str(code))
        code, body = http(port, "/api/llm", "PUT", {"X-AutoMemory-Password": "correct horse"}, {"name": "../x", "model": "m"})
        rep.check(code == 400, "a profile name that is a path is refused", f"{code} {body[:80]}")
        code, body = http(port, "/api/llm", "PUT", {"X-AutoMemory-Password": "correct horse"}, {"name": "local", "model": "m", "endpoint": "http://127.0.0.1:9/v1", "activate": True})
        lj = json.loads(http(port, "/api/llm")[1])
        rep.check(code == 200 and lj["active"] == "local" and lj["local"] is True and len(lj["profiles"]) == 2, "a profile can be saved and activated with the password", f"{code} {body[:80]}")
        code, _ = http(port, "/api/llm/profile/local", "DELETE", {})
        rep.check(code == 403, "Protected: removing a profile is refused without the password", str(code))
        http(port, "/api/llm/profile/local", "DELETE", {"X-AutoMemory-Password": "correct horse"})
        rep.check(json.loads(http(port, "/api/llm")[1])["active"] == "default", "removing the active profile falls back to default")
        code, body = http(port, "/api/value?days=7")
        v = json.loads(body) if code == 200 else {}
        rep.check(code == 200 and v.get("days") == 7 and "collisions_prevented" in v, "GET /api/value returns the figures", f"{code} {body[:80]}")
        out = subprocess.run([sb.bin, "stats", "--json"], capture_output=True, text=True, env=sb.env, cwd=sb.project).stdout
        rep.check(json.loads(out).get("days") == 7, "automemory stats --json matches", out[:80])
        code, body = http(port, "/api/memories/hygiene")
        rep.check(code == 200 and "items" in body, "GET /api/memories/hygiene answers", f"{code} {body[:80]}")
        code, _ = http(port, "/api/memories/archive_many", "POST", {}, {"ids": ["x"]})
        rep.check(code == 403, "Protected: bulk archive is refused without the password", str(code))
        code, _ = http(port, "/api/locks/stop", "POST", {}, {})
        rep.check(code == 403, "Protected: the kill switch is refused without the password", str(code))
        code, _ = http(port, "/api/agents/connect", "POST", {}, {"agent": "claude"})
        rep.check(code == 403, "Protected: connecting an agent is refused without the password", str(code))
        code, body = http(port, "/api/agents/disconnect", "POST", {}, {"agent": "--everywhere", "dry_run": True})
        rep.check(code == 400, "an agent name that is really a flag is refused (no argument injection)", f"{code} {body[:80]}")
        code, body = http(port, "/api/agents/connect", "POST", {"X-AutoMemory-Password": "correct horse"}, {"agent": "--global"})
        rep.check(code == 400, "connect refuses a flag as the agent name even with the password", f"{code} {body[:80]}")
        code, body = http(port, "/api/password", "POST", {"X-AutoMemory-Password": "correct horse"}, {"password": "x"})
        rep.check(code in (404, 405) or "<html" in body.lower(), "Terminal-only: there is no route to change the password (only the page shell answers)", f"{code} {body[:60]}")

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
