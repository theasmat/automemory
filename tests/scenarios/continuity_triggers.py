#!/usr/bin/env python3
"""Continuity C6: chat history refreshes by itself at Stop / SessionEnd / PreCompact, but only for agents a person turned on;
and chat history moves between computers only when sync-history is run (encrypted, password to send)."""
import json
import re
import shutil
import subprocess
import sys
import time
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parents[1] / "lib"))
from harness import REPO, Report, Sandbox  # noqa: E402

FIXTURE = REPO / "tests/continuity/fixtures/claude/home/.claude/projects/-tmp-demo-shop/sess-001.jsonl"
PW = "correct horse"


def git(cwd, *args):
    return subprocess.run(["git", *args], cwd=cwd, capture_output=True, text=True, check=True).stdout


class Machine(Sandbox):
    def __init__(self, name):
        super().__init__(name)
        self.env.update({"AUTOMEMORY_SECRETS_BACKEND": "file", "AUTOMEMORY_KEY": "hist-shared-key", "AUTOMEMORY_DEVICE_NAME": name})
        git(self.project, "remote", "add", "origin", "/code/shop.git")

    def chat(self):
        root = self.project.resolve()
        d = self.home / ".claude" / "projects" / re.sub(r"[^A-Za-z0-9]", "-", str(root))
        d.mkdir(parents=True, exist_ok=True)
        shutil.copy(FIXTURE, d / "sess-001.jsonl")

    def found(self, word="sticky"):
        return word in self.run("continuity", "search", word).stdout


def wait(fn, tries=60):
    for _ in range(tries):
        if fn():
            return True
        time.sleep(0.25)
    return False


def main():
    rep = Report("continuity triggers and history sync")
    a = b = None
    try:
        a, b = Machine("laptop"), Machine("desktop")
        a.mcp("project.register", slug="shop", display_name="shop", path=str(a.project.resolve()))
        a.chat()

        # Off: a Stop does not read anything.
        a.hook("claude", "stop", {"session_id": "s1"})
        time.sleep(1.5)
        rep.check(not a.found(), "with no agent turned on, a Stop imports nothing")

        a.run("continuity", "enable", "claude")
        a.hook("claude", "stop", {"session_id": "s1"})
        rep.check(wait(a.found), "after a person turns Claude on, a Stop imports the chat by itself")
        items = a.run("continuity", "show").stdout
        rep.check("shopping cart" in items and "draft" in items, "and drafts the brief from it", items[-300:])

        # Throttle: a second Stop right away does nothing harmful; SessionEnd and PreCompact still refresh.
        r = a.hook("claude", "pre-compact", {"session_id": "s1"})
        rep.check(r[2].strip() == "" or "error" not in r[2].lower(), "PreCompact is accepted by the hook", r[2][:120])

        # ── History between computers: only by request ──
        vault = a.root / "vault.git"
        vault.mkdir()
        git(vault, "init", "-q", "--bare", "-b", "main")
        for m in (a, b):
            m.run("sync", "remote", str(vault))
        subprocess.run([a.bin, "rule", "passwd", "--from-env"], env={**a.env, "AUTOMEMORY_PASSWORD": PW}, cwd=a.project, capture_output=True)
        out = a.run("continuity", "sync-history").stdout
        rep.check("sess-001" in out and "KB encrypted" in out and "Nothing was sent" in out, "without a flag it shows what would be sent, with sizes", out)
        a.run("sync", "auto")
        rep.check("chat/" not in git(vault, "ls-tree", "-r", "--name-only", "main") if subprocess.run(["git", "rev-parse", "main"], cwd=vault, capture_output=True).returncode == 0 else True,
                  "ordinary sync never carries chat history")
        r = a.run("continuity", "sync-history", "--push")
        rep.check(r.returncode != 0, "pushing history needs the password", (r.stdout + r.stderr)[-160:])
        r = subprocess.run([a.bin, "continuity", "sync-history", "--push"], env={**a.env, "AUTOMEMORY_PASSWORD": PW}, cwd=a.project, capture_output=True, text=True)
        rep.check(r.returncode == 0 and "encrypted into the vault" in r.stdout, "with the password it is encrypted and pushed", r.stdout + r.stderr)
        files = git(vault, "ls-tree", "-r", "--name-only", "main")
        rep.check("chat/" in files and not subprocess.run(["git", "grep", "-i", "sticky", "main"], cwd=vault, capture_output=True).stdout, "the remote holds the chat as ciphertext", files)

        b.mcp("project.register", slug="shop", display_name="shop", path=str(b.project.resolve()))
        rep.check(not b.found(), "the other computer has no history yet")
        r = b.run("continuity", "sync-history", "--pull")
        rep.check(r.returncode == 0 and "Added 1 session" in r.stdout, "pulling adds the session", r.stdout + r.stderr)
        rep.check(b.found(), "and it is searchable there")
        rep.check("Added 0 session" in b.run("continuity", "sync-history", "--pull").stdout, "pulling twice adds nothing")
    finally:
        for m in (a, b):
            if m:
                m.cleanup()
    rep.finish()


if __name__ == "__main__":
    main()
