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
        hits = sb.mcp("memory.search", scope="library", query="sticky routing", project_id="app")
        rep.check(hits["hits"] and hits["hits"][0]["path"] == "plans/prd-cart.md", "MCP memory.search (scope library) finds it", json.dumps(hits)[:200])

        rep.check("memory.search with scope=library" in str(sb.mcp("library.search", query="x", project_id="app")), "the removed name library.search points to memory.search")
        # The session-start pack carries a short index, not the files.
        out = json.dumps(sb.hook("claude", "prompt", {"session_id": "s-new", "hook_event_name": "UserPromptSubmit", "prompt": "hi", "cwd": str(sb.project)})[0] or {})
        rep.check("Project library" in out and "plans/prd-cart.md" in out and "store carts in SQLite" not in out, "a new session is told the library exists (index only, not the content)", out[:300])

        # L5: agents write their own files, suggest edits to yours, and cite library files.
        put = sb.mcp("library.put", path="notes/agent-notes.md", content="# Agent notes\n\nThe checkout retries three times.\n", agent="claude", project_id="app")
        rep.check(put.get("outcome") == "created", "an agent saves its own note in the library", json.dumps(put)[:160])
        got = sb.mcp("library.list", project_id="app")
        mine = [f for f in got["files"] if f["path"] == "notes/agent-notes.md"]
        rep.check(mine and mine[0]["origin"] == "agent" and mine[0]["by"] == "claude", "it is listed as agent-written")
        r = sb.mcp("library.put", path="plans/prd-cart.md", content="# overwritten\n", agent="claude", project_id="app")
        rep.check("written by a person" in str(r) and "library.propose" in str(r), "an agent cannot overwrite your file and is told to propose", str(r)[:200])
        r = sb.mcp("library.put", path="notes/leak.md", content="token sk-abcdefghijklmnopqrstuvwxyz1234567890\n", agent="claude", project_id="app")
        rep.check("looks like a secret" in str(r), "an agent's secret is refused")
        prop = sb.mcp("library.propose", path="plans/prd-cart.md", content=PRD.replace("30 days", "90 days"), reason="support asked", agent="claude", project_id="app")
        rep.check(str(prop.get("proposal", "")).startswith("libprop-"), "an agent's suggested edit is filed", json.dumps(prop)[:160])
        rep.check(sb.mcp("library.get", path="plans/prd-cart.md", project_id="app")["content"] == PRD, "your file is unchanged until you approve")
        # Cite a library file in a brief item and as room evidence.
        ok = sb.mcp("continuity.brief", action="add", project_id="app", agent="claude", session_id="s1", items=[{"kind": "next", "text": "Implement the cart per the PRD", "ref_id": "library:plans/prd-cart.md"}])
        rep.check("[see library:plans/prd-cart.md]" in ok["brief"], "a brief item can point at a library file", ok["brief"][:300])
        bad = sb.mcp("continuity.brief", action="add", project_id="app", agent="claude", session_id="s1", items=[{"kind": "next", "text": "Read the missing file", "ref_id": "library:nope.md"}])
        rep.check("not in the project library" in str(bad), "a missing library file cannot be cited")
        room = sb.mcp("room.join", session_id="agent-a", topic="cart expiry", project_id="app")["room"]["id"]
        r = sb.mcp("room.post", room_id=room, session_id="agent-a", kind="propose", body="Expire carts after thirty days, as the PRD says, and clean up nightly.", evidence=["library:plans/prd-cart.md"])
        rep.check("refused" not in json.dumps(r).lower(), "a library file counts as room evidence", json.dumps(r)[:200])

        # The project's own documents (found in the repository) show next to the library files, readable the same way.
        (sb.project / "PRD.md").write_text("# Shop PRD\n\nA small shop. Carts matter most.\n\n## Goals\n- Fast checkout\n")
        plist = sb.mcp("project.list")
        plist = plist.get("projects", plist) if isinstance(plist, dict) else plist
        pid = next(p["id"] for p in plist if p.get("slug") == "app")
        r = sb.run("doc", "track", "PRD.md", "--category", "prd", "--project", pid)
        both = sb.mcp("library.list", project_id="app")
        repo = [f for f in both["files"] if f.get("source") == "repo"]
        rep.check(r.returncode == 0 and repo and repo[0]["path"] == "PRD.md" and any(f.get("source") == "library" for f in both["files"]), "library.list shows the repository's documents too, marked source repo", (r.stdout + r.stderr)[-160:] + json.dumps(both)[:200])
        full = sb.mcp("library.get", path="PRD.md", tier=2, project_id="app")
        rep.check(full.get("source") == "repo" and "Carts matter most" in full["content"], "library.get opens one (the file itself at tier 2)", json.dumps(full)[:200])
        rep.check(sb.mcp("library.get", path="PRD.md", tier=0, project_id="app")["content"] != "", "and its abstract at tier 0")
        rep.check("library.list" in str(sb.mcp("doc.list", project_id="app")), "the removed name doc.list points to library.list")

        # L2: a folder view for agents without MCP, kept out of git status; linked files from the global config.
        r = sb.run("library", "sync")
        folder = sb.project / ".automemory" / "library"
        rep.check(r.returncode == 0 and (folder / "plans/prd-cart.md").read_text() == PRD and "plans/prd-cart.md" in (folder / "INDEX.md").read_text(), "sync writes the copy and INDEX.md into .automemory/library", r.stdout + r.stderr)
        rep.check(".automemory" not in subprocess.run(["git", "status", "--porcelain", "-uall"], cwd=sb.project, capture_output=True, text=True).stdout, "the folder does not show in git status (local exclude)")
        rep.check(not (sb.project / ".gitignore").exists(), ".gitignore was not created or changed")
        (folder / "plans/prd-cart.md").unlink()
        sb.hook("claude", "prompt", {"session_id": "s-restore", "hook_event_name": "UserPromptSubmit", "prompt": "hi", "cwd": str(sb.project)})
        rep.check((folder / "plans/prd-cart.md").exists(), "a new session finds the copy in place (the hook restores it)")
        (sb.project / ".gitignore").write_text("PRD-private.md\n")
        (sb.project / "PRD-private.md").write_text("# Private PRD\n\nKept out of git on purpose.\n")
        r = sb.run("library", "link", "PRD-private.md")
        rep.check(r.returncode != 0, "changing which files are kept in sync needs the password")
        r = pw("library", "link", "PRD-private.md")
        rep.check(r.returncode == 0, "with the password the pattern is added to the global config", r.stdout + r.stderr)
        rep.check(sb.run("library", "sync").returncode == 0, "sync reads it")
        listing = sb.mcp("library.list", project_id="app")
        linked = [f for f in listing["files"] if f["path"] == "PRD-private.md"]
        rep.check(bool(linked) and linked[0]["linked"] is True, "a git-ignored file named in the config is in the library, marked linked", json.dumps(listing)[:300])
        rep.check((sb.project / ".gitignore").read_text() == "PRD-private.md\n" and (sb.project / "PRD-private.md").exists() and not (folder / "PRD-private.md").exists(), "it stays where it is; git settings untouched; no copy made")
        (sb.project / "PRD-private.md").write_text("# Private PRD\n\nChanged on disk.\n")
        sb.run("library", "sync")
        rep.check("Changed on disk" in sb.mcp("library.get", path="PRD-private.md", project_id="app")["content"], "an edit in place is picked up")
        rep.check("PRD-private.md" in sb.run("library", "links").stdout, "links shows the pattern and the file")
        dr = sb.run("doctor").stdout
        rep.check("Checking project library... ✓" in dr, "doctor checks the library", dr[dr.find("Checking project library"):][:200])
        (folder / "INDEX.md").unlink()
        (folder / "plans/prd-cart.md").unlink()
        rep.check("not up to date" in sb.run("doctor").stdout, "doctor notices a missing copy")
        sb.run("library", "sync")
        pw("library", "unlink", "PRD-private.md")

        # Moving a file out of the repository: copy in, delete the original, print the git command, never commit.
        inrepo = sb.project / "PRIVATE-PRD.md"
        inrepo.write_text("# Private\n\nNot for the repository.\n")
        subprocess.run(["git", "add", "PRIVATE-PRD.md"], cwd=sb.project, capture_output=True)
        r = pw("library", "add", str(inrepo), "--move")
        rep.check(r.returncode == 0 and not inrepo.exists() and "git rm --cached" in r.stdout, "--move removes the original and prints the git command", r.stdout + r.stderr)
        untracked = sb.project / "SCRATCH-PRD.md"
        untracked.write_text("# Scratch\n\nNever added to git.\n")
        r = pw("library", "add", str(untracked), "--move")
        rep.check(r.returncode == 0 and "not tracking it" in r.stdout and "git rm" not in r.stdout, "--move on a file git never tracked says there is nothing to do in git", r.stdout + r.stderr)
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
