#!/usr/bin/env python3
"""Project library L4: the dashboard API. Reading and syncing the folder are safe; adding, editing, removing, restoring and choosing
files kept in place need the password; a file kept in place is edited on disk, not through the page."""
import json
import subprocess
import sys
import time
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parents[1] / "lib"))
sys.path.insert(0, str(Path(__file__).parent))
from dashboard import free_port, http  # noqa: E402
from harness import Report, Sandbox  # noqa: E402

PW = {"X-AutoMemory-Password": "correct horse"}


def main():
    rep = Report("library dashboard")
    sb = Sandbox("libdash")
    sb.env.update({"AUTOMEMORY_SECRETS_BACKEND": "file", "AUTOMEMORY_KEY": "ld-master"})
    server = None
    try:
        sb.mcp("project.register", slug="app", display_name="app", path=str(sb.project.resolve()))
        subprocess.run([sb.bin, "rule", "passwd", "--from-env"], env={**sb.env, "AUTOMEMORY_PASSWORD": "correct horse"}, cwd=sb.project, capture_output=True)
        port = free_port()
        server = subprocess.Popen([sb.bin, "serve", "--port", str(port)], cwd=sb.project, env=sb.env, stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL)
        for _ in range(50):
            try:
                http(port, "/api/stats")
                break
            except OSError:
                time.sleep(0.1)

        code, body = http(port, "/api/library")
        o = json.loads(body)
        rep.check(code == 200 and o["files"] == [] and o["folder"]["exists"] is False, "overview: empty library, no folder yet", body[:160])
        code, _ = http(port, "/api/library/file", "PUT", {}, {"path": "plans/prd.md", "content": "# PRD\n\nCarts expire.\n"})
        rep.check(code == 403, "Protected: adding a file needs the password")
        code, body = http(port, "/api/library/file", "PUT", PW, {"path": "plans/prd.md", "content": "# PRD\n\nCarts expire after 30 days.\n"})
        rep.check(code == 200 and json.loads(body)["outcome"] == "created", "adding with the password works", body[:120])
        code, body = http(port, "/api/library/file", "PUT", PW, {"path": "bad.md", "content": "key sk-abcdefghijklmnopqrstuvwxyz1234567890\n"})
        rep.check(code == 400 and "line 1" in body, "a secret is refused, naming the line", body[:140])
        code, _ = http(port, "/api/library/file", "PUT", PW, {"path": "../x.md", "content": "no"})
        rep.check(code == 400, "a path outside the library is refused")
        http(port, "/api/library/file", "PUT", PW, {"path": "plans/prd.md", "content": "# PRD\n\nCarts expire after 90 days.\n"})
        d = json.loads(http(port, "/api/library/file?path=plans/prd.md")[1])
        rep.check("90 days" in d["content"] and d["summary"].startswith("PRD:") and len(d["revisions"]) == 2 and "# PRD" in d["outline"], "the file shows summary, outline, text and two revisions", json.dumps(d)[:200])
        old = json.loads(http(port, "/api/library/revision?path=plans/prd.md&rev=1")[1])["content"]
        rep.check("30 days" in old, "an older revision can be read")
        code, _ = http(port, "/api/library/revert", "POST", {}, {"path": "plans/prd.md", "rev": 1})
        rep.check(code == 403, "Protected: restoring needs the password")
        code, _ = http(port, "/api/library/revert", "POST", PW, {"path": "plans/prd.md", "rev": 1})
        rep.check(code == 200 and "30 days" in json.loads(http(port, "/api/library/file?path=plans/prd.md")[1])["content"], "restoring revision 1 works")

        code, body = http(port, "/api/library/sync", "POST", {}, {})
        o = json.loads(http(port, "/api/library")[1])
        rep.check(code == 200 and (sb.project / ".automemory/library/plans/prd.md").exists() and o["folder"]["exists"], "Safe: sync writes the copy and INDEX.md", body[:160])
        rep.check(o["folder"]["out_of_git_status"] is True and o["problems"] == [], "the folder is out of git status and nothing needs attention", json.dumps(o["folder"]) + json.dumps(o["problems"]))

        (sb.project / "PRD-private.md").write_text("# Private\n\nIgnored by git.\n")
        (sb.project / ".gitignore").write_text("PRD-private.md\n")
        code, _ = http(port, "/api/library/link", "POST", {}, {"pattern": "PRD-private.md"})
        rep.check(code == 403, "Protected: choosing files to keep in place needs the password")
        code, _ = http(port, "/api/library/link", "POST", PW, {"pattern": "/etc/*.md"})
        rep.check(code == 400, "a pattern outside the project is refused")
        code, _ = http(port, "/api/library/link", "POST", PW, {"pattern": "PRD-private.md"})
        http(port, "/api/library/sync", "POST", {}, {})
        o = json.loads(http(port, "/api/library")[1])
        linked = [f for f in o["files"] if f["path"] == "PRD-private.md"]
        rep.check(code == 200 and linked and linked[0]["linked"] and "PRD-private.md" in o["patterns"]["global"], "the ignored file is kept in place and listed as linked")
        code, body = http(port, "/api/library/file", "PUT", PW, {"path": "PRD-private.md", "content": "# overwritten\n"})
        rep.check(code == 400 and "edit it there" in body, "a kept-in-place file is edited on disk, not through the page", body[:120])
        code, _ = http(port, "/api/library/link?pattern=PRD-private.md", "DELETE", {})
        rep.check(code == 403, "Protected: stopping a pattern needs the password")
        code, _ = http(port, "/api/library/link?pattern=PRD-private.md", "DELETE", PW)
        rep.check(code == 200 and "PRD-private.md" not in json.loads(http(port, "/api/library")[1])["patterns"]["global"], "stopping a pattern works")

        # An agent's suggested edit waits in the Inbox; only a person applies it, one at a time.
        prop = sb.mcp("library.propose", path="plans/prd.md", content="# PRD\n\nCarts expire after 45 days.\n", reason="support asked", agent="claude", project_id="app")["proposal"]
        pend = json.loads(http(port, "/api/proposals")[1])
        pend = pend if isinstance(pend, list) else pend.get("proposals", [])
        rep.check(any(p["id"] == prop and "claude suggests changing" in p["description"] for p in pend), "the suggestion shows in the Inbox with who and what", json.dumps(pend)[:200])
        code, _ = http(port, "/api/proposals/decide_all", "POST", PW, {"approved": True, "proposal_type": "all"})
        rep.check("30 days" in json.loads(http(port, "/api/library/file?path=plans/prd.md")[1])["content"], "approve-all does not apply it")
        code, _ = http(port, "/api/proposals/decide", "POST", {}, {"id": prop, "approved": True})
        rep.check(code == 403, "Protected: approving needs the password")
        code, body = http(port, "/api/proposals/decide", "POST", PW, {"id": prop, "approved": True})
        d = json.loads(http(port, "/api/library/file?path=plans/prd.md")[1])
        rep.check(code == 200 and "45 days" in d["content"] and d["author"] == "approved from claude", "approving applies it as your change", body[:120] + json.dumps(d)[:120])
        code, _ = http(port, "/api/library/file?path=plans/prd.md", "DELETE", {})
        rep.check(code == 403, "Protected: removing needs the password")
        code, _ = http(port, "/api/library/file?path=plans/prd.md", "DELETE", PW)
        rep.check(code == 200 and http(port, "/api/library/file?path=plans/prd.md")[0] == 400, "removing works and the file is gone")
    finally:
        if server:
            server.kill()
        sb.cleanup()
    rep.finish()


if __name__ == "__main__":
    main()
