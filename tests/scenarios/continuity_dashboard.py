#!/usr/bin/env python3
"""Continuity C4: the dashboard API. Turning an agent on, editing the brief and deleting history need the password;
importing and the hand-off preview are safe; nothing is read until an agent is on."""
import json
import re
import shutil
import subprocess
import sys
import time
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parents[1] / "lib"))
sys.path.insert(0, str(Path(__file__).parent))
from dashboard import free_port, http  # noqa: E402
from harness import REPO, Report, Sandbox  # noqa: E402

PW = {"X-AutoMemory-Password": "correct horse"}
FIXTURE = REPO / "tests/continuity/fixtures/claude/home/.claude/projects/-tmp-demo-shop/sess-001.jsonl"


def main():
    rep = Report("continuity dashboard")
    sb = Sandbox("contdash")
    sb.env.update({"AUTOMEMORY_SECRETS_BACKEND": "file", "AUTOMEMORY_KEY": "cd-master"})
    server = None
    try:
        root = sb.project.resolve()
        sb.mcp("project.register", slug="app", display_name="app", path=str(root))
        d = sb.home / ".claude" / "projects" / re.sub(r"[^A-Za-z0-9]", "-", str(root))
        d.mkdir(parents=True)
        shutil.copy(FIXTURE, d / "sess-001.jsonl")
        subprocess.run([sb.bin, "rule", "passwd", "--from-env"], env={**sb.env, "AUTOMEMORY_PASSWORD": "correct horse"}, cwd=sb.project, capture_output=True)
        port = free_port()
        server = subprocess.Popen([sb.bin, "serve", "--port", str(port)], cwd=sb.project, env=sb.env, stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL)
        for _ in range(50):
            try:
                http(port, "/api/stats")
                break
            except OSError:
                time.sleep(0.1)

        code, body = http(port, "/api/continuity")
        o = json.loads(body)
        cc = next((s for s in o["sources"] if s["id"] == "claude"), {})
        rep.check(code == 200 and cc.get("enabled") is False and cc.get("found") == 1, "overview: Claude is off and its chat is found", body[:200])
        code, body = http(port, "/api/continuity/import", "POST", {}, {})
        rep.check(code == 400, "import with no agent on is refused", f"{code} {body[:100]}")
        code, _ = http(port, "/api/continuity/source/claude", "PUT", {}, {"enabled": True})
        rep.check(code == 403, "Protected: turning an agent on needs the password", str(code))
        code, _ = http(port, "/api/continuity/source/claude", "PUT", PW, {"enabled": True})
        rep.check(code == 200, "turning Claude on with the password works", str(code))
        code, _ = http(port, "/api/continuity/source/nope", "PUT", PW, {"enabled": True})
        rep.check(code == 400, "an unknown agent is refused")

        code, _ = http(port, "/api/continuity/items", "POST", {}, {"kind": "next", "text": "Wire the cart page to the new route"})
        rep.check(code == 403, "Protected: adding to the brief needs the password")
        code, body = http(port, "/api/continuity/items", "POST", PW, {"kind": "next", "text": "Wire the cart page to the new route"})
        rep.check(code == 200, "adding to the brief works", f"{code} {body[:120]}")
        items = json.loads(http(port, "/api/continuity")[1])["items"]
        it = next((i for i in items if "cart page" in i["text"]), None)
        rep.check(bool(it) and "you" in it["label"].lower(), "the item shows who wrote it", json.dumps(it))
        code, _ = http(port, f"/api/continuity/items/{it['id']}", "PUT", {}, {"text": "Wire the cart page"})
        rep.check(code == 403, "Protected: editing needs the password")
        code, _ = http(port, f"/api/continuity/items/{it['id']}", "PUT", PW, {"text": "Wire the cart page to the checkout route"})
        rep.check(code == 200, "editing works")
        code, _ = http(port, f"/api/continuity/items/{it['id']}/resolve", "POST", PW, {})
        rep.check(code == 200, "marking done works")
        items = json.loads(http(port, "/api/continuity")[1])["items"]
        rep.check(not any(i["id"] == it["id"] for i in items), "a finished item leaves the brief")

        code, body = http(port, "/api/continuity/import", "POST", {}, {"agent": "claude"})
        rep.check(code == 200, "Safe: import starts without the password", f"{code} {body[:100]}")
        for _ in range(60):
            o = json.loads(http(port, "/api/continuity")[1])
            if next(s for s in o["sources"] if s["id"] == "claude")["messages"] > 0:
                break
            time.sleep(0.25)
        rep.check(next(s for s in o["sources"] if s["id"] == "claude")["messages"] >= 5, "the job imported the chat", json.dumps(o["sources"])[:200])
        code, body = http(port, "/api/continuity/search?q=sticky")
        hits = json.loads(body)["hits"]
        rep.check(code == 200 and hits, "search finds the imported chat", body[:160])
        code, body = http(port, f"/api/continuity/chat?session={hits[0]['session']}&from=0&limit=4")
        rep.check(code == 200 and json.loads(body)["messages"], "an exchange can be opened", body[:120])
        code, body = http(port, "/api/continuity/handoff", "POST", {}, {})
        rep.check(code == 200 and "preview" in json.loads(body), "Safe: Hand off now needs no password", f"{code} {body[:120]}")

        code, _ = http(port, "/api/continuity/history?agent=claude", "DELETE", {})
        rep.check(code == 403, "Protected: deleting history needs the password")
        code, body = http(port, "/api/continuity/history?agent=claude", "DELETE", PW)
        rep.check(code == 200 and json.loads(body)["removed"] >= 5, "deleting history works", body[:100])
        rep.check(http(port, "/api/continuity/search?q=sticky")[1].count('"session"') == 0, "after deleting, the search finds nothing")
    finally:
        if server:
            server.kill()
        sb.cleanup()
    rep.finish()


if __name__ == "__main__":
    main()
