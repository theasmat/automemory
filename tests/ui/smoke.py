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
from changes import fake as fake_tracewise  # noqa: E402

PAGES = ["/", "/brain", "/agents", "/inbox", "/tools", "/models", "/models?tab=helper", "/safety", "/sync", "/settings",
         "/activity", "/activity?tab=sessions", "/activity?tab=traces", "/activity?tab=problems", "/activity?tab=skills",
         "/rooms", "/continuity", "/library"]


def main():
    rep = Report("dashboard pages in a browser")
    sb = Sandbox("smoke")
    sb.env.update({"AUTOMEMORY_SECRETS_BACKEND": "file", "AUTOMEMORY_KEY": "smoke-master"})
    (sb.root / "tools").mkdir()
    sb.env["AUTOMEMORY_TRACEWISE_BIN"] = str(fake_tracewise(sb.root / "tools"))  # canned file changes for the change lists
    sb.run("demo")
    sb.run("connect", "claude")  # an agent to invite in the New room dialog
    # A decided planning room waiting for the human verdict.
    sb.mcp("project.register", slug="app", display_name="app", path=str(sb.project))
    ev = sb.mcp("memory.save", summary="Routing note", content="No sticky sessions", type="fact", scope="project", project_id="app")
    ev = ev.get("id") or ev.get("memory", {}).get("id")
    rid = sb.mcp("room.join", session_id="agent-a", topic="auth strategy", project_id="app")["room"]["id"]
    sb.mcp("room.join", session_id="agent-b", room_id=rid)
    for who, kind, body in (("agent-a", "propose", "Server sessions (with enough detail to count)"), ("agent-b", "critique", "Needs sticky routing (with enough detail to count)"),
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
            # Continuity: seeded through the same API the page uses (the dashboard's own project, whichever it picks).
            import json as _j, urllib.request as _u
            for it in ({"kind": "goal", "text": "Ship the cart"}, {"kind": "rejected", "text": "Server-side sessions", "reason": "no sticky routing"}):
                _u.urlopen(_u.Request(base + "/api/continuity/items", data=_j.dumps(it).encode(), method="POST",
                                      headers={"Content-Type": "application/json", "X-AutoMemory-Password": "smoke pw"}), timeout=60)
            # Continuity: the brief with provenance, the preview, opt-in rows with locks, and Hand off now (Safe).
            page = browser.new_page()
            problems = []
            page.on("console", lambda m: m.type == "error" and problems.append(m.text[:160]))
            page.goto(base + "/continuity", wait_until="networkidle")
            page.wait_for_timeout(500)
            body = page.inner_text("main")
            rep.check("Ship the cart" in body and "Server-side sessions because no sticky routing" in body and "you" in body, "the Continuity page shows the brief with who wrote it", body[:300])
            rep.check("Claude Code" in body and "Antigravity" in body and "Off" in body and "Codex" in body, "every agent is listed, off by default, and agents without a reader are named")
            rep.check(page.get_by_role("button", name="Turn on").first.locator("svg[aria-label]").count() == 1, "turning an agent on shows the lock")
            page.get_by_role("button", name="Hand off now").click()
            page.wait_for_timeout(1500)
            rep.check("Hand-off written" in page.inner_text("main"), "Hand off now works without a password", page.inner_text("main")[:200])
            rep.check(not problems, "the Continuity page uses only known policy paths", "; ".join(problems))
            page.close()
            # Old links keep working: merged pages forward to their new home.
            page = browser.new_page()
            for old, now in (("/replay", "tab=sessions"), ("/skills", "tab=skills"), ("/failures", "tab=problems"), ("/memories", "/brain"), ("/docs", "/library"), ("/constitution", "/brain"), ("/graph", "/brain")):
                page.goto(base + old, wait_until="networkidle")
                page.wait_for_timeout(300)
                rep.check(now in page.url, f"{old} forwards to its new home", page.url)
            page.goto(base + "/activity", wait_until="networkidle")
            rep.check("What worked" in page.inner_text("main") and "Learned skills" in page.inner_text("main"), "Activity has the tabs for sessions, what worked, problems and skills")
            page.close()
            # Library: seeded through the page's own API; tree, tiers, lock icons, no unknown policy paths.
            _u.urlopen(_u.Request(base + "/api/library/file", data=_j.dumps({"path": "plans/prd.md", "content": "# Cart PRD\n\nCarts expire after 30 days.\n\n## Decisions\n- Decision: SQLite\n"}).encode(), method="PUT",
                                  headers={"Content-Type": "application/json", "X-AutoMemory-Password": "smoke pw"}), timeout=60)
            page = browser.new_page()
            problems = []
            page.on("console", lambda m: m.type == "error" and problems.append(m.text[:160]))
            page.goto(base + "/library", wait_until="networkidle")
            page.wait_for_timeout(500)
            body = page.inner_text("main")
            rep.check("plans" in body and "prd.md" in body, "the Library page shows the file tree", body[:200])
            page.get_by_role("button", name="prd.md").click()
            page.wait_for_timeout(500)
            rep.check("Cart PRD: Carts expire after 30 days." in page.inner_text("main"), "a file opens at the summary tier")
            page.get_by_role("button", name="Outline").click()
            rep.check("Decision: SQLite" in page.inner_text("main"), "the outline tier keeps the decision line")
            page.get_by_role("button", name="Whole file").click()
            rep.check(page.get_by_role("button", name="Edit").locator("svg[aria-label]").count() == 1, "Edit shows the lock")
            rep.check(page.get_by_role("button", name="Sync folder").locator("svg[aria-label]").count() == 0, "Sync folder is Safe: no lock")
            page.get_by_role("button", name="Sync folder").click()
            page.wait_for_timeout(1200)
            rep.check("Synced:" in page.inner_text("main"), "Sync folder reports what it did", page.inner_text("main")[:200])
            rep.check(not problems, "the Library page uses only known policy paths", "; ".join(problems))
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
            page.fill("input[placeholder='Cart storage']", "Topic only")
            if page.locator("fieldset input[type=checkbox]").count():
                page.locator("fieldset input[type=checkbox]").first.check()
                rep.check(not page.get_by_role("button", name="Create").is_enabled(), "Create stays disabled until the question is filled in")
                page.fill("input[placeholder^='Where do we keep']", "Where?")
                rep.check(page.get_by_role("button", name="Create").is_enabled(), "and enables once it is")
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
            page.goto(base + "/models?tab=helper", wait_until="networkidle")
            rep.check(page.locator("input[type=password]").count() == 0, "no password field before an action")
            page.get_by_role("button", name="Turn on").click()
            page.wait_for_timeout(500)
            rep.check(page.locator("input[type=password]").count() == 1, "a Protected action opens the password prompt")
            # File changes from Tracewise: the list on Agents & Locks and the files of a session
            page = browser.new_page(viewport={"width": 1280, "height": 800})
            page.goto(base + "/agents", wait_until="networkidle")
            page.wait_for_timeout(500)
            main_text = page.inner_text("main")
            rep.check("Recent file changes" in main_text and "src/storage.py" in main_text and "Monthly report query" in main_text and "codex" in main_text, "Agents & Locks lists the recent file changes with agent, file and reason", main_text[:300])
            rep.check("Edited while held" in main_text, "and marks an edit made while another agent held the file")
            page.goto(base + "/", wait_until="networkidle")
            page.wait_for_timeout(600)
            rep.check("Latest file changes" in page.inner_text("main") and "src/storage.py" in page.inner_text("main"), "Home shows the latest file changes when Tracewise answers")
            page.goto(base + "/activity?tab=sessions", wait_until="networkidle")
            page.wait_for_timeout(500)
            first = page.locator("button", has_text="events").first
            if first.count():
                first.click()
                page.wait_for_timeout(600)
                rep.check("Files this session changed" in page.inner_text("main"), "a replayed session shows the files Tracewise recorded for it")
            else:
                print("  SKIP  no recorded session in the smoke data")
            page.close()
            # Git backup wizard on the Sync page: provider hints, set up with the password, the recovery phrase once, then the status
            vault = sb.root / "vault-smoke.git"
            vault.mkdir()
            subprocess.run(["git", "init", "-q", "--bare", "-b", "main"], cwd=vault, check=True)
            page = browser.new_page(viewport={"width": 1280, "height": 800})
            page.goto(base + "/sync", wait_until="networkidle")
            main_text = page.inner_text("main")
            rep.check("Git backup" in main_text and "Off" in main_text and "GitHub" in main_text and "Bitbucket" in main_text, "Sync shows the git backup card with the providers when it is off", main_text[:300])
            page.get_by_role("button", name="GitLab").click()
            rep.check("glab repo create" in page.inner_text("main"), "picking a provider shows how to create the repository there")
            page.get_by_label("2. Paste its address").fill(str(vault))
            page.get_by_role("button", name="Set up backup").click()
            page.wait_for_selector("input[type=password]", timeout=10000)
            page.locator("input[type=password]").fill("smoke pw")
            page.keyboard.press("Enter")
            page.get_by_role("heading", name="Your recovery phrase").wait_for(timeout=60000)
            rep.check(page.locator("ol li").count() == 24, "setup shows the 24-word recovery phrase", str(page.locator("ol li").count()))
            done = page.get_by_role("button", name="Done")
            rep.check(done.is_disabled(), "Done stays disabled until the phrase is confirmed saved")
            page.get_by_label("I saved the recovery phrase").check()
            done.click()
            page.get_by_role("button", name="Back up now").wait_for(timeout=10000)
            main_text = page.inner_text("main")
            rep.check("Working" in main_text and "Last backed up" in main_text and "vault-smoke.git" in main_text, "then the card shows it working, with the remote and the last backup", main_text[:300])
            page.get_by_role("button", name="Check that it restores").click()
            page.wait_for_selector("text=The copy on the remote restores", timeout=60000)
            rep.check(True, "Check that it restores proves the copy on the remote")
            page.close()
            page = browser.new_page(viewport={"width": 1280, "height": 800})
            page.goto(base + "/", wait_until="networkidle")
            rep.check("backup is failing" not in page.inner_text("main"), "Home has no backup warning while it works")
            page.close()
            # data the backup cannot carry is shown, with the fix, on the card and on Home
            sb.mcp("memory.save", summary="Orphaned smoke note", content="filed under an id that is no project", type="gotcha", scope="project", project_id="auto")
            page = browser.new_page(viewport={"width": 1280, "height": 800})
            page.goto(base + "/sync", wait_until="networkidle")
            page.wait_for_timeout(500)
            txt = page.inner_text("main")
            rep.check("Partly backed up" in txt and "Not everything on this computer is backed up" in txt and "project adopt auto" in txt, "the Sync card says what is not backed up and how to fix it", txt[:400])
            page.goto(base + "/", wait_until="networkidle")
            page.wait_for_timeout(500)
            rep.check("not backed up" in page.inner_text("main").lower(), "and Home says part of the memory is not backed up")
            page.close()
            # Responsive: no page scrolls sideways at phone, tablet or desktop width; the shell adapts (docs/specs/dashboard-responsive.md)
            for width, height in ((375, 812), (768, 1024), (1280, 800)):
                wide = []
                for path in PAGES:
                    page = browser.new_page(viewport={"width": width, "height": height})
                    page.goto(base + path, wait_until="networkidle")
                    if page.evaluate("document.documentElement.scrollWidth > document.documentElement.clientWidth + 1"):
                        wide.append(path)
                    page.close()
                rep.check(not wide, f"no page scrolls sideways at {width}px", ", ".join(wide))
            page = browser.new_page(viewport={"width": 375, "height": 812})
            page.goto(base + "/brain", wait_until="networkidle")
            nav = page.locator("nav[aria-label='Mobile navigation']")
            rep.check(nav.locator("a, button").count() == 5, "the phone bottom bar has five slots", str(nav.locator("a, button").count()))
            rep.check(page.locator("#project-select").is_visible() and page.get_by_role("button", name="Search or command (⌘K)").is_visible(), "the phone top bar keeps the project picker and search")
            tops = [b for b in page.locator("header button, header select").all() if b.is_visible()]
            boxes = [b.bounding_box() for b in tops]
            overlap = any(a is not None and c is not None and i < j and a["x"] < c["x"] + c["width"] - 1 and c["x"] < a["x"] + a["width"] - 1 for i, a in enumerate(boxes) for j, c in enumerate(boxes))
            rep.check(not overlap, "and nothing in the phone top bar overlaps")
            small = [b for b in nav.locator("a, button").all() if b.bounding_box()["height"] < 44]
            rep.check(not small, "phone bottom bar targets are at least 44px tall")
            nav.get_by_role("button", name="More").click()
            page.wait_for_timeout(400)
            rep.check(page.get_by_role("link", name="Sync & devices").is_visible() and page.get_by_role("link", name="Safety").is_visible(), "More lists Safety and Sync & devices on a phone")
            page.close()
            # Memory cards: a click opens the full details, the buttons on the card do not, Edit shows the form, Escape closes
            for width, height in ((1280, 800), (375, 812)):
                page = browser.new_page(viewport={"width": width, "height": height})
                page.goto(base + "/brain", wait_until="networkidle")
                title = page.get_by_role("button", name="Open details:").first
                summary = title.inner_text().strip()
                page.get_by_role("button", name="Pin to top").first.click()
                page.wait_for_timeout(300)
                rep.check(page.get_by_text("Note details").count() == 0, f"a card's own buttons do not open the details ({width}px)")
                page.keyboard.press("Escape")  # pinning asks for the password: close that prompt
                page.wait_for_timeout(300)
                title.click()
                page.wait_for_timeout(400)
                dialog = page.get_by_role("dialog").filter(has_text="Note details")
                rep.check(dialog.count() == 1 and summary[:30] in dialog.inner_text(), f"clicking a card opens its full details ({width}px)", dialog.inner_text()[:120] if dialog.count() else "no dialog")
                text = dialog.inner_text()
                rep.check(all(w in text for w in ("Confidence", "Created", "ID", "Edit")), "the details show the facts, the id and an Edit button", text[:300])
                dialog.get_by_role("button", name="Edit").click()
                page.wait_for_timeout(300)
                rep.check(page.get_by_label("Title").is_visible() and page.get_by_label("Full text").is_visible(), "Edit shows the title and full-text fields")
                page.keyboard.press("Escape")
                page.wait_for_timeout(300)
                rep.check(page.get_by_text("Note details").count() == 0 and page.get_by_text("Edit note").count() == 0, "Escape closes the dialog")
                page.close()
            page = browser.new_page(viewport={"width": 768, "height": 1024})
            page.goto(base + "/brain", wait_until="networkidle")
            rep.check(page.locator("aside[data-tour='nav-rail']").bounding_box()["width"] < 100, "on a tablet the sidebar is an icon rail")
            page.close()
            page = browser.new_page(viewport={"width": 1280, "height": 800})
            page.goto(base + "/brain", wait_until="networkidle")
            rep.check(page.locator("aside[data-tour='nav-rail']").bounding_box()["width"] > 200, "on a desktop the sidebar shows its labels")
            page.close()
            browser.close()
    finally:
        server.terminate()
        server.wait(timeout=5)
        sb.cleanup()
    return rep.finish()


if __name__ == "__main__":
    sys.exit(main())
