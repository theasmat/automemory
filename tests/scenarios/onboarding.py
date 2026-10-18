#!/usr/bin/env python3
"""Onboarding checklist, `automemory next`, and dashboard onboarding APIs.

Validates that:
1. `automemory next` points to the best next setup step with a clear reason and Try: command.
2. `automemory doctor` prints the Getting Started checklist.
3. `/api/setup` returns the shared checklist and reflects real configuration changes (e.g. sync remote added and removed).
4. `/api/onboarding` progress can be loaded, updated, and reset, and POST is human-gated.
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
    req = urllib.request.Request(
        f"http://127.0.0.1:{port}{path}",
        method=method,
        data=json.dumps(body).encode() if body is not None else None,
        headers={"Content-Type": "application/json", **(headers or {})},
    )
    try:
        with urllib.request.urlopen(req, timeout=60) as r:
            return r.status, r.read().decode()
    except urllib.error.HTTPError as e:
        return e.code, e.read().decode()


def main():
    sb = Sandbox("onboarding")
    rep = Report("first-run onboarding and setup checklist")
    port = free_port()
    server = None

    try:
        # 1. Fresh state: `automemory next` suggests connecting an agent
        r = sb.run("next")
        rep.check(r.returncode == 0, "`automemory next` exits 0 on clean sandbox")
        rep.check(
            "No agent connected yet" in r.stdout and "Try: `automemory connect" in r.stdout,
            "`automemory next` suggests connecting an agent",
            r.stdout,
        )

        # 2. `automemory doctor` includes Getting Started checklist
        r = sb.run("doctor")
        rep.check(r.returncode == 0, "`automemory doctor` exits 0")
        rep.check(
            "Getting Started Checklist" in r.stdout and "Agent connected" in r.stdout,
            "`automemory doctor` displays Getting Started checklist",
            r.stdout[-300:],
        )

        # 3. Start dashboard daemon
        server = subprocess.Popen(
            [sb.bin, "serve", "--port", str(port)],
            cwd=sb.project,
            env=sb.env,
            stdout=subprocess.DEVNULL,
            stderr=subprocess.DEVNULL,
        )
        for _ in range(50):
            try:
                http(port, "/api/stats")
                break
            except OSError:
                time.sleep(0.1)

        # 4. Check GET /api/setup
        code, body = http(port, "/api/setup")
        rep.check(code == 200, "GET /api/setup returns 200", f"{code} {body[:100]}")
        steps = json.loads(body)
        step_map = {s["id"]: s for s in steps}
        rep.check("agent" in step_map and not step_map["agent"]["done"], "setup: agent step is undone initially")
        rep.check("sync" in step_map and not step_map["sync"]["done"], "setup: sync step is undone initially")

        # 5. Check GET and POST /api/onboarding
        code, body = http(port, "/api/onboarding")
        rep.check(code == 200, "GET /api/onboarding returns 200", f"{code} {body[:100]}")
        ob_state = json.loads(body)
        rep.check(isinstance(ob_state.get("completed_steps"), list), "GET /api/onboarding has completed_steps list")

        # POST without password when no password set -> allowed with warning
        code, body = http(port, "/api/onboarding", method="POST", body={"action": "mark_done", "step_id": "test_step"})
        rep.check(code == 200, "POST /api/onboarding works with no password set", f"{code} {body[:100]}")
        code, body = http(port, "/api/onboarding")
        ob_state = json.loads(body)
        rep.check("test_step" in ob_state.get("completed_steps", []), "mark_done saved in onboarding state")

        # 6. Setup progress: Connect an agent
        # We can connect claude in this workspace
        sb.run("connect", "claude")
        r = sb.run("next")
        rep.check(r.returncode == 0, "`automemory next` runs after connect")
        rep.check(
            "export-rules" in r.stdout or "instruction_files" in r.stdout.lower() or "Instruction files" in r.stdout,
            "`automemory next` advances to instruction files after agent is connected",
            r.stdout,
        )

        # 7. Check sync remote setup and removal via /api/setup
        sb.run("sync", "remote", "https://github.com/example-user/my-memory-vault.git")
        code, body = http(port, "/api/setup")
        steps = json.loads(body)
        step_map = {s["id"]: s for s in steps}
        rep.check(step_map["sync"]["done"], "sync step is marked done after remote is configured")

        # Now clear the remote
        sb.run("sync", "remote", "--clear")
        code, body = http(port, "/api/setup")
        steps = json.loads(body)
        step_map = {s["id"]: s for s in steps}
        rep.check(not step_map["sync"]["done"], "sync step is undone after remote is cleared")

        # 8. Password gate on POST /api/onboarding
        env_with_pw = {**sb.env, "AUTOMEMORY_PASSWORD": "secret_horse_pass"}
        subprocess.run(
            [sb.bin, "rule", "passwd", "--from-env"],
            env=env_with_pw,
            cwd=sb.project,
            capture_output=True,
            text=True,
        )

        # Attempt POST without password -> 403 Forbidden
        code, body = http(port, "/api/onboarding", method="POST", body={"action": "mark_done", "step_id": "blocked"})
        rep.check(code == 403, "POST /api/onboarding denied without password header", f"{code} {body[:100]}")

        # Attempt POST with password -> 200 OK
        code, body = http(
            port,
            "/api/onboarding",
            method="POST",
            headers={"X-AutoMemory-Password": "secret_horse_pass"},
            body={"action": "reset"},
        )
        rep.check(code == 200, "POST /api/onboarding succeeded with password header", f"{code} {body[:100]}")
        code, body = http(port, "/api/onboarding")
        ob_state = json.loads(body)
        rep.check(len(ob_state.get("completed_steps", [])) == 0, "reset cleared completed_steps")

    finally:
        if server is not None:
            server.terminate()
            server.wait(timeout=5)
        sb.cleanup()

    return rep.finish()


if __name__ == "__main__":
    sys.exit(main())
