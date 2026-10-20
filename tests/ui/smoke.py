#!/usr/bin/env python3
"""Opens every dashboard page in a real headless browser against a live server with sample data.

Fails on any uncaught exception, console error or failed API call, on a page that renders (almost) nothing, and when
a Protected action does not ask for the password. Needs Playwright (tests/ui/.venv, see tests/README.md); without it
the test is skipped with a message, never silently passed.
"""
import subprocess
import sys
import time
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parents[1] / "lib"))
from harness import Report, Sandbox  # noqa: E402

sys.path.insert(0, str(Path(__file__).parent))
try:
    from playwright.sync_api import sync_playwright
except ImportError:
    print("SKIPPED: Playwright is not installed (python3 -m venv tests/ui/.venv && tests/ui/.venv/bin/pip install playwright && tests/ui/.venv/bin/playwright install chromium)")
    sys.exit(0)

sys.path.insert(0, str(Path(__file__).resolve().parents[1] / "scenarios"))
from dashboard import free_port  # noqa: E402

PAGES = ["/", "/brain", "/agents", "/inbox", "/tools", "/recall", "/safety", "/sync", "/settings", "/memories",
         "/proposals", "/coordination", "/guardrails", "/skills", "/traces", "/replay", "/failures", "/docs",
         "/constitution", "/graph", "/overview", "/activity", "/rooms"]


def main():
    rep = Report("dashboard pages in a browser")
    sb = Sandbox("smoke")
    sb.env.update({"AUTOMEMORY_SECRETS_BACKEND": "file", "AUTOMEMORY_KEY": "smoke-master"})
    sb.run("demo")
    # A decided planning room waiting for the human verdict.
    sb.mcp("project.register", slug="app", display_name="app", path=str(sb.project))
    ev = sb.mcp("memory.save", summary="Routing note", content="No sticky sessions", type="fact", scope="project", project_id="app")
    ev = ev.get("id") or ev.get("memory", {}).get("id")
    rid = sb.mcp("room.join", session_id="agent-a", topic="auth strategy", project_id="app")["room"]["id"]
    sb.mcp("room.join", session_id="agent-b", room_id=rid)
    for who, kind, body in (("agent-a", "propose", "Server sessions"), ("agent-b", "critique", "Needs sticky routing"),
                            ("agent-a", "revise", "Use JWT in a cookie\n- Add token table\n- Write login route")):
        sb.mcp("room.post", room_id=rid, session_id=who, kind=kind, body=body, evidence=[ev])
    # The password prompt only appears once a human password exists.
    subprocess.run([sb.bin, "rule", "passwd", "--from-env"], env={**sb.env, "AUTOMEMORY_PASSWORD": "smoke pw"}, cwd=sb.project, capture_output=True)
    port = free_port()
    server = subprocess.Popen([sb.bin, "serve", "--port", str(port)], cwd=sb.project, env=sb.env,
                              stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL)
    base = f"http://127.0.0.1:{port}"
    try:
        for _ in range(50):
            try:
                import urllib.request
                urllib.request.urlopen(base + "/api/stats", timeout=2)
                break
            except OSError:
                time.sleep(0.2)
        with sync_playwright() as p:
            browser = p.chromium.launch()
            for theme in ("light", "dark"):
                page = browser.new_page(color_scheme=theme)
                problems = []
                page.on("pageerror", lambda e: problems.append(f"exception: {e}"))
                page.on("console", lambda m: m.type == "error" and problems.append(f"console: {m.text[:160]}"))
                page.on("response", lambda r: r.status >= 500 and problems.append(f"{r.status} {r.url}"))
                for path in PAGES:
                    problems.clear()
                    page.goto(base + path, wait_until="networkidle")
                    page.wait_for_timeout(300)
                    text = page.inner_text("body")
                    rep.check(not problems, f"{theme} {path}: no errors", "; ".join(problems[:3]))
                    rep.check(len(text.strip()) > 40, f"{theme} {path}: renders content", repr(text[:60]))
                page.close()
            # Lock icons come from /api/policy: Protected buttons have one, Safe buttons do not.
            page = browser.new_page()
            problems = []
            page.on("console", lambda m: m.type == "error" and problems.append(m.text[:160]))
            page.goto(base + "/brain", wait_until="networkidle")
            rep.check(page.get_by_role("button", name="Record a decision").locator("svg[aria-label]").count() == 1, "a Protected button shows the lock")
            page.goto(base + "/tools", wait_until="networkidle")
            rep.check(page.get_by_role("button", name="Back up").locator("svg[aria-label]").count() == 0, "a Safe button shows no lock")
            rep.check(not problems, "every lock path exists in /api/policy", "; ".join(problems))
            page.close()
            # Planning rooms: the discussion and plan show, and approving (asks the password) records the verdict.
            page = browser.new_page()
            page.goto(base + "/rooms", wait_until="networkidle")
            page.wait_for_timeout(500)
            body = page.inner_text("main")
            rep.check("auth strategy" in body and "Needs sticky routing" in body and "Add token table" in body, "the room page shows the discussion and the plan", body[:200])
            page.get_by_role("button", name="New room").click()
            page.wait_for_timeout(400)
            dialog = page.inner_text("body")
            rep.check("New planning room" in dialog and ("Claude Code" in dialog or "No agent is connected" in dialog),
                      "the New room dialog opens and lists agents to invite (or says none is connected)", dialog[-300:])
            page.keyboard.press("Escape")
            page.wait_for_timeout(200)
            page.get_by_role("button", name="Approve").click()
            page.locator("input[type=password]").fill("smoke pw")
            page.keyboard.press("Enter")
            page.wait_for_timeout(1500)
            rep.check("Approved. The decision is recorded and 2 tasks" in page.inner_text("main"), "approving shows the verdict and the task count", page.inner_text("main")[-200:])
            page.close()
            # A Protected action asks for the password before anything is sent.
            page = browser.new_page()
            page.goto(base + "/tools", wait_until="networkidle")
            rep.check(page.locator("input[type=password]").count() == 0, "no password field before an action")
            page.get_by_role("button", name="Turn on").click()
            page.wait_for_timeout(500)
            rep.check(page.locator("input[type=password]").count() == 1, "a Protected action opens the password prompt")
            browser.close()
    finally:
        server.terminate()
        server.wait(timeout=5)
        sb.cleanup()
    return rep.finish()


if __name__ == "__main__":
    sys.exit(main())
