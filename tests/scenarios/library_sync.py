#!/usr/bin/env python3
"""Project library L3: library files travel between your computers through the encrypted vault (git remote): shared files and removals do,
personal files do not, a file kept in place arrives in place, and the remote holds ciphertext only."""
import subprocess
import sys
import time
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parents[1] / "lib"))
from harness import Report, Sandbox  # noqa: E402

PW = "correct horse"


def git(cwd, *args):
    return subprocess.run(["git", *args], cwd=cwd, capture_output=True, text=True, check=True).stdout


class Machine(Sandbox):
    def __init__(self, name):
        super().__init__(name)
        self.env.update({"AUTOMEMORY_SECRETS_BACKEND": "file", "AUTOMEMORY_KEY": "library-shared-key", "AUTOMEMORY_DEVICE_NAME": name})
        git(self.project, "remote", "add", "origin", "/code/shop.git")

    def pw(self, *a):
        return subprocess.run([self.bin, *a], env={**self.env, "AUTOMEMORY_PASSWORD": PW}, cwd=self.project, capture_output=True, text=True)

    def sync(self):
        for _ in range(40):
            r = self.run("sync", "auto")
            if "already running" not in r.stdout:
                return r.stdout.strip()
            time.sleep(0.25)
        return "still locked"


def main():
    rep = Report("project library sync")
    a = b = None
    try:
        a, b = Machine("laptop"), Machine("desktop")
        vault = a.root / "vault.git"
        vault.mkdir()
        git(vault, "init", "-q", "--bare", "-b", "main")
        for m in (a, b):
            m.run("sync", "remote", str(vault))
            m.mcp("project.register", slug="shop", display_name="shop", path=str(m.project.resolve()))
            subprocess.run([m.bin, "rule", "passwd", "--from-env"], env={**m.env, "AUTOMEMORY_PASSWORD": PW}, cwd=m.project, capture_output=True)

        prd = a.root / "prd.md"
        prd.write_text("# Cart PRD\n\nCarts expire after thirty days. Secretive phrase: zebra-umbrella.\n")
        mine = a.root / "mine.md"
        mine.write_text("# Mine\n\nOnly on the laptop: kangaroo-lantern.\n")
        rep.check(a.pw("library", "add", str(prd), "--as", "plans/prd.md").returncode == 0, "A adds a shared file")
        rep.check(a.pw("library", "add", str(mine), "--as", "mine.md", "--personal").returncode == 0, "and a personal one")
        (a.project / "PRD-private.md").write_text("# Private PRD\n\nKept out of git on purpose: walrus-teapot.\n")
        (a.project / ".gitignore").write_text("PRD-private.md\n")
        a.pw("library", "link", "PRD-private.md")
        a.run("library", "sync")
        out = a.sync()
        rep.check("pushed" in out or "already" in out, "A pushes to the vault", out)
        a.sync()

        files = git(vault, "ls-tree", "-r", "--name-only", "main")
        rep.check(".enc" in files and ".jsonl" not in files, "the remote holds encrypted files only", files)
        rep.check(subprocess.run(["git", "grep", "-i", "-e", "zebra-umbrella", "-e", "walrus-teapot", "main"], cwd=vault, capture_output=True, text=True).returncode != 0, "none of the document text is readable there")

        out = b.sync()
        rep.check("pulled" in out, "B pulls", out)
        ls = b.run("library", "ls").stdout
        rep.check("plans/prd.md" in ls and "PRD-private.md" in ls, "B's library lists the shared and the linked file", ls)
        rep.check("mine.md" not in ls, "the personal file stayed on A")
        b.run("library", "sync")
        folder = b.project / ".automemory" / "library"
        rep.check("zebra-umbrella" in (folder / "plans/prd.md").read_text() and "plans/prd.md" in (folder / "INDEX.md").read_text(), "B gets the copy and INDEX.md under .automemory/library")
        rep.check((b.project / "PRD-private.md").read_text().count("walrus-teapot") == 1, "the linked file is written in place where A keeps it")
        rep.check("zebra-umbrella" in b.run("library", "search", "zebra").stdout.replace("[", "").replace("]", "") or "plans/prd.md" in b.run("library", "search", "zebra").stdout, "and it is searchable on B")

        # A removal on A travels to B.
        a.pw("library", "rm", "plans/prd.md")
        a.sync()
        b.sync()
        b.run("library", "sync")
        rep.check("plans/prd.md" not in b.run("library", "ls").stdout and not (folder / "plans/prd.md").exists(), "a removal on A reaches B and removes the untouched copy")
        b.sync()
        a.sync()
        rep.check("plans/prd.md" not in a.run("library", "ls").stdout, "and later syncs do not bring it back")
    finally:
        for m in (a, b):
            if m:
                m.cleanup()
    rep.finish()


if __name__ == "__main__":
    main()
