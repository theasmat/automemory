#!/usr/bin/env python3
"""`doctor` names MCP servers that started before the installed binary was replaced (found in trial T11b: Antigravity's server
from the day before kept applying old room rules). Unix only. Runs on a COPY of the binary."""
import os
import shutil
import subprocess
import sys
import tempfile
import time
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parents[1] / "lib"))
from harness import Report, find_binary  # noqa: E402


def main():
    rep = Report("stale MCP servers")
    if os.name == "nt":
        print("SKIP: needs ps")
        return 0
    tmp = Path(tempfile.mkdtemp(prefix="am-stale-mcp-"))
    exe = tmp / "automemory"
    shutil.copy(find_binary(), exe)
    env = {**os.environ, "HOME": str(tmp / "home")}
    (tmp / "home").mkdir()
    doctor = lambda: subprocess.run([str(exe), "doctor"], capture_output=True, text=True, env=env, cwd=tmp).stdout
    old = new = None
    try:
        old = subprocess.Popen([str(exe), "mcp", "--db", str(tmp / "a.db")], stdin=subprocess.PIPE, stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL, env=env)
        time.sleep(8)
        out = doctor()
        rep.check("none older than this binary" in out, "a server that started after the install is not flagged", out[-300:])
        # "Upgrade": replace the file under the running server (a new inode, as cargo install and `update` do).
        shutil.copy(find_binary(), tmp / "automemory.new")
        os.replace(tmp / "automemory.new", exe)
        new = subprocess.Popen([str(exe), "mcp", "--db", str(tmp / "b.db")], stdin=subprocess.PIPE, stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL, env=env)
        time.sleep(1)
        out = doctor()
        rep.check(f"pid {old.pid} " in out and "restart the agent" in out, "after the binary is replaced, the old server is named", out[-400:])
        rep.check(f"pid {new.pid} " not in out, "and the server started after the replacement is not")
    finally:
        for p in (old, new):
            if p:
                p.kill()
        shutil.rmtree(tmp, ignore_errors=True)
    return rep.finish()


if __name__ == "__main__":
    sys.exit(main())
