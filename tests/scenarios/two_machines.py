#!/usr/bin/env python3
"""The same experience on another machine, through a private git remote.

Two machines = two separate HOMEs, the repository checked out at different paths, one shared
(bare) git repository as the vault. Machine A makes a decision and works in a session; machine B,
which has never seen the project, starts a new session and must get A's task and decision.

Both machines use the AUTOMEMORY_KEY override, so the real keychain is never touched.
"""
import json
import subprocess
import sys
import time
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parents[1] / "lib"))
from harness import Report, Sandbox  # noqa: E402


def git(cwd, *args):
    return subprocess.run(["git", *args], cwd=cwd, capture_output=True, text=True, check=True).stdout


class Machine(Sandbox):
    def __init__(self, name, device, code_remote, key):
        super().__init__(name)
        self.env.update({"AUTOMEMORY_KEY": key, "AUTOMEMORY_DEVICE_NAME": device})
        git(self.project, "remote", "add", "origin", str(code_remote))

    def sync(self):
        """A synchronous pass; waits out a background sync a hook may have started."""
        for _ in range(40):
            r = self.run("sync", "auto")
            if "already running" not in r.stdout:
                return r.stdout.strip()
            time.sleep(0.25)
        return "still locked"

    def edit(self, session, path):
        self.hook("claude", "post-tool", {"session_id": session, "tool_name": "Edit",
                                          "tool_input": {"file_path": path}, "tool_response": {}})


def main():
    rep = Report("same experience on another machine")
    a = b = None
    try:
        a = Machine("machine-a", "laptop", "/code/shop.git", "shared-test-key")
        b = Machine("machine-b", "desktop", "/code/shop.git", "shared-test-key")
        vault = a.root / "vault.git"
        vault.mkdir()
        git(vault, "init", "-q", "--bare", "-b", "main")
        rep.check(str(a.project) != str(b.project), "the repository lives at different paths", f"{a.project} {b.project}")

        # ── Machine A ──
        a.run("sync", "remote", str(vault))
        a.mcp("project.register", slug="shop", display_name="shop", path=str(a.project))
        d = a.mcp("decision.record", topic="payments", decision="Use Stripe webhooks, not polling",
                  reason="polling missed events during outages", project_id="shop", agent="claude")
        rep.check("error" not in str(d).lower(), "A records a decision", str(d)[:160])
        for f in ("src/checkout.rs", "src/webhook.rs", "src/lib.rs"):
            a.edit("sA", f)
        a.hook("claude", "stop", {"session_id": "sA", "stop_hook_active": False})
        a.hook("claude", "session-end", {"session_id": "sA"})  # writes the hand-off, starts a sync
        out = a.sync()
        rep.check("pushed" in out or "already" in out, "A pushes to the vault", out)
        a.sync()

        remote_files = git(vault, "ls-tree", "-r", "--name-only", "main")
        rep.check(".enc" in remote_files and ".jsonl" not in remote_files,
                  "the remote holds encrypted files only", remote_files)
        tree = subprocess.run(["git", "grep", "-i", "stripe", "main"], cwd=vault, capture_output=True, text=True)
        rep.check(tree.returncode != 0, "and none of the decision text is readable there", tree.stdout[:200])

        # ── Machine B: never saw this project ──
        b.run("sync", "remote", str(vault))
        out = b.sync()
        rep.check("pulled" in out, "B pulls the vault", out)
        projects = b.mcp("project.list")
        names = [p.get("slug") for p in (projects.get("projects", projects) if isinstance(projects, dict) else projects)]
        rep.check(names.count("shop") == 1, "B has exactly one 'shop' project (created from the repository key)", str(names))

        # A new session on B: the first prompt carries the resume pack.
        parsed, raw, _ = b.hook("claude", "prompt", {"session_id": "sB", "prompt": "continue the checkout work"})
        rep.check("Resuming this project" in raw and "Last hand-off" in raw, "B's first prompt gets the resume pack", raw[:400])
        rep.check("payments" in raw and "Stripe webhooks" in raw, "it names A's decision", raw[:600])
        rep.check("claude" in raw, "and who handed off", raw[:400])
        parsed, raw, _ = b.hook("claude", "prompt", {"session_id": "sB", "prompt": "next question"})
        rep.check("Resuming this project" not in raw, "the pack is shown once per session", raw[:200])

        got = b.mcp("decision.get", topic="payments", project_id="shop")
        rep.check("Stripe webhooks" in str(got), "decision.get on B returns A's decision", str(got)[:200])
        resumed = b.mcp("memory.resume", project_id="shop", session_id="sB2")
        rep.check(resumed.get("resume") and "hand-off" in resumed["resume"], "memory.resume works over MCP", str(resumed)[:200])

        # ── Round trip: B decides, A receives (no duplicates) ──
        b.mcp("decision.record", topic="auth", decision="JWT in an httpOnly cookie", reason="XSS", project_id="shop", agent="claude")
        b.sync()
        a.sync()
        got = a.mcp("decision.get", topic="auth", project_id="shop")
        rep.check("httpOnly cookie" in str(got), "A receives B's decision", str(got)[:200])
        a.sync()
        b.sync()
        pa = a.mcp("project.list")
        names_a = [p.get("slug") for p in (pa.get("projects", pa) if isinstance(pa, dict) else pa)]
        rep.check(names_a.count("shop") == 1, "still one project on A after the round trips", str(names_a))
        files_before = git(vault, "ls-tree", "-r", "--name-only", "main").count(".enc")
        a.sync()
        b.sync()
        rep.check(git(vault, "ls-tree", "-r", "--name-only", "main").count(".enc") == files_before,
                  "quiet passes add nothing to the vault")
    finally:
        for m in (a, b):
            if m:
                m.cleanup()
    return rep.finish()


if __name__ == "__main__":
    sys.exit(main())
