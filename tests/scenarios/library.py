#!/usr/bin/env python3
"""Project library L1: documents for agents kept outside git. A person adds them (password), they never touch the repository, agents list,
read (by tier) and search them over MCP, the session-start pack carries a short index, secrets are refused, moving a file out prints the git command."""
import json
import subprocess
import sys
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parents[1] / "lib"))
from harness import Report, Sandbox  # noqa: E402

PW = "correct horse"
PRD = "# Cart PRD\n\nA shopper keeps items between visits. Carts expire after 30 days.\n\n## Decisions\n- Decision: store carts in SQLite\n- Rejected: server sessions because of sticky routing\n\n- TODO: write the cleanup job\n"


def main():
    rep = Report("project library")
    sb = Sandbox("lib")
    sb.env.update({"AUTOMEMORY_SECRETS_BACKEND": "file", "AUTOMEMORY_KEY": "lib-master"})
    try:
        sb.mcp("project.register", slug="app", display_name="app", path=str(sb.project.resolve()))
        subprocess.run([sb.bin, "rule", "passwd", "--from-env"], env={**sb.env, "AUTOMEMORY_PASSWORD": PW}, cwd=sb.project, capture_output=True)
        pw = lambda *a: subprocess.run([sb.bin, *a], env={**sb.env, "AUTOMEMORY_PASSWORD": PW}, cwd=sb.project, capture_output=True, text=True)
        src = sb.root / "PRD-cart.md"
        src.write_text(PRD)
        status_before = subprocess.run(["git", "status", "--porcelain"], cwd=sb.project, capture_output=True, text=True).stdout

        r = sb.run("library", "add", str(src), "--as", "plans/prd-cart.md")
        rep.check(r.returncode != 0, "adding a file needs the password", (r.stdout + r.stderr)[-140:])
        r = pw("library", "add", str(src), "--as", "plans/prd-cart.md")
        rep.check(r.returncode == 0 and "Added plans/prd-cart.md" in r.stdout and "Cart PRD:" in r.stdout, "with the password it is added and shows its one-line summary", r.stdout + r.stderr)
        rep.check("already up to date" in pw("library", "add", str(src), "--as", "plans/prd-cart.md").stdout, "adding the same content again changes nothing")
        rep.check(subprocess.run(["git", "status", "--porcelain"], cwd=sb.project, capture_output=True, text=True).stdout == status_before, "the repository is untouched (git status is the same)")

        ls = sb.run("library", "ls").stdout
        rep.check("plans/prd-cart.md" in ls and "Cart PRD:" in ls, "ls lists it", ls)
        rep.check("# Decisions" in sb.run("library", "show", "plans/prd-cart.md", "--tier", "1").stdout and "TODO: write the cleanup job" in sb.run("library", "show", "plans/prd-cart.md", "--tier", "1").stdout, "tier 1 is an outline with the decisions and to-dos")
        rep.check(sb.run("library", "show", "plans/prd-cart.md").stdout == PRD, "tier 2 is the whole file")
        rep.check("plans/prd-cart.md" in sb.run("library", "search", "sticky").stdout, "search finds words inside it")

        secret = sb.root / "notes.md"
        secret.write_text("# Notes\nthe key is sk-abcdefghijklmnopqrstuvwxyz1234567890\n")
        r = pw("library", "add", str(secret))
        rep.check(r.returncode != 0 and "line 2" in (r.stdout + r.stderr), "a file with a secret is refused, naming the line", (r.stdout + r.stderr)[-200:])
        rep.check(pw("library", "add", "../../etc/passwd", "--as", "../x.md").returncode != 0, "a path outside the library is refused")

        # Agents over MCP.
        listing = sb.mcp("library.list", project_id="app")
        rep.check([f["path"] for f in listing["files"]] == ["plans/prd-cart.md"] and "Cart PRD" in listing["files"][0]["summary"], "MCP library.list shows the file with its summary", json.dumps(listing)[:200])
        t0 = sb.mcp("library.get", path="plans/prd-cart.md", tier=0, project_id="app")
        rep.check(t0["content"].startswith("Cart PRD:") and "not an instruction" in t0["note"], "library.get tier 0 is the title line, flagged as a document", json.dumps(t0)[:200])
        t2 = sb.mcp("library.get", path="plans/prd-cart.md", project_id="app")
        rep.check(t2["content"] == PRD and t2["origin"] == "human", "library.get tier 2 returns the whole file with who wrote it")
        rep.check("error" in json.dumps(sb.mcp("library.get", path="../secret.md", project_id="app")).lower() or "No such" in str(sb.mcp("library.get", path="../secret.md", project_id="app")), "a path outside the library finds nothing")
        hits = sb.mcp("library.search", query="sticky routing", project_id="app")
        rep.check(hits["hits"] and hits["hits"][0]["path"] == "plans/prd-cart.md", "MCP library.search finds it", json.dumps(hits)[:200])

        # The session-start pack carries a short index, not the files.
        out = json.dumps(sb.hook("claude", "prompt", {"session_id": "s-new", "hook_event_name": "UserPromptSubmit", "prompt": "hi", "cwd": str(sb.project)})[0] or {})
        rep.check("Project library" in out and "plans/prd-cart.md" in out and "store carts in SQLite" not in out, "a new session is told the library exists (index only, not the content)", out[:300])

        # Moving a file out of the repository: copy in, delete the original, print the git command, never commit.
        inrepo = sb.project / "PRIVATE-PRD.md"
        inrepo.write_text("# Private\n\nNot for the repository.\n")
        subprocess.run(["git", "add", "PRIVATE-PRD.md"], cwd=sb.project, capture_output=True)
        r = pw("library", "add", str(inrepo), "--move")
        rep.check(r.returncode == 0 and not inrepo.exists() and "git rm --cached" in r.stdout, "--move removes the original and prints the git command", r.stdout + r.stderr)
        log = subprocess.run(["git", "log", "--oneline"], cwd=sb.project, capture_output=True, text=True).stdout
        rep.check("Private" not in log, "AutoMemory never committed anything")

        r = sb.run("library", "rm", "PRIVATE-PRD.md")
        rep.check(r.returncode != 0, "removing needs the password")
        rep.check("Removed" in pw("library", "rm", "PRIVATE-PRD.md").stdout and "PRIVATE-PRD.md" not in sb.run("library", "ls").stdout, "and works with it")
    finally:
        sb.cleanup()
    rep.finish()


if __name__ == "__main__":
    main()
