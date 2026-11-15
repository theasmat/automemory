#!/usr/bin/env python3
"""Tracewise nested inside AutoMemory's hook (docs/specs/tracewise-companion.md, phase 2). A fake `tracewise` pins the contract; the real one, when built,
shows two agents' locks coming from Tracewise alone."""
import json
import os
import stat
import subprocess
import sys
import time
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parents[1] / "lib"))
from harness import Report, Sandbox  # noqa: E402

REAL = Path.home() / "Developer/project/tracewise/target/release/tracewise"


def fake(dir_: Path, hook: str, sleep_s: str = "0", api: int = 1) -> Path:
    p = dir_ / "tracewise"
    p.write_text(f"#!/bin/sh\ncat >/dev/null\nif [ \"$1\" = companion ]; then echo '{{\"api\":{api},\"version\":\"9.9\"}}'; else sleep {sleep_s}; echo '{hook}'; fi\n")
    p.chmod(p.stat().st_mode | stat.S_IEXEC)
    return p


def pre_edit(sb, session, path, agent="claude"):
    return sb.hook(agent, "pre-tool", {"session_id": session, "hook_event_name": "PreToolUse", "tool_name": "Edit", "tool_input": {"file_path": path}, "cwd": str(sb.project)})


def denied(r):
    return "deny" in (r[1] or "")


def main():
    rep = Report("tracewise as a companion")
    # ---- contract, with a fake tracewise
    sb = Sandbox("companion-fake")
    try:
        tools = Path(sb.root) / "tools"
        tools.mkdir()
        sb.env["AUTOMEMORY_TRACEWISE_BIN"] = str(fake(tools, '{"api":1,"event":"pre-edit","decision":"deny","reason":"a.txt is held by codex (session s9)","context":[]}'))
        sb.run("connect", "claude")
        r = pre_edit(sb, "s1", "a.txt")
        rep.check(denied(r) and "held by codex" in r[1], "a deny from Tracewise reaches the agent with Tracewise's reason", r[1][:200])

        fake(tools, '{"api":1,"event":"pre-edit","decision":"allow","reason":null,"context":[]}')
        sb.run("doctor")  # (cache is per minute: clear it so the next answers are read afresh)
        import sqlite3
        con = sqlite3.connect(sb.home / ".automemory" / "automemory.db")
        con.execute("delete from sync_kv where key = 'companion_probe'")
        con.commit()
        con.close()
        rep.check(not denied(pre_edit(sb, "s1", "b.txt")), "an allow from Tracewise changes nothing")

        fake(tools, '{"api":1,"event":"pre-edit","decision":"deny","reason":"too slow to count"}', sleep_s="2")
        t = time.time()
        r = pre_edit(sb, "s1", "c.txt")
        rep.check(not denied(r) and time.time() - t < 1.5, "a Tracewise slower than the timeout is skipped: the agent is not blocked and not kept waiting", f"{time.time()-t:.2f}s {r[1][:120]}")

        con = sqlite3.connect(sb.home / ".automemory" / "automemory.db")
        con.execute("delete from sync_kv where key = 'companion_probe'")
        con.commit()
        con.close()
        fake(tools, '{"api":7,"decision":"deny","reason":"from the future"}', api=7)
        rep.check(not denied(pre_edit(sb, "s1", "d.txt")), "a companion with another API number is not used")

        con = sqlite3.connect(sb.home / ".automemory" / "automemory.db")
        con.execute("delete from sync_kv where key = 'companion_probe'")
        con.commit()
        con.close()
        fake(tools, '{"api":1,"event":"prompt","decision":"allow","reason":null,"context":["Tracewise: since your last prompt, codex changed a.txt"]}')
        sb.hook("claude", "prompt", {"session_id": "s1", "hook_event_name": "UserPromptSubmit", "prompt": "go on", "cwd": str(sb.project)})
        out = sb.hook("claude", "prompt", {"session_id": "s2", "hook_event_name": "UserPromptSubmit", "prompt": "hi", "cwd": str(sb.project)})
        rep.check("codex changed a.txt" in (out[1] or ""), "what Tracewise says at a prompt reaches the agent", (out[1] or "")[:200])

        (sb.home / ".automemory" / "config.toml").write_text('[companion]\ntracewise = "off"\n')
        con = sqlite3.connect(sb.home / ".automemory" / "automemory.db")
        con.execute("delete from sync_kv where key = 'companion_probe'")
        con.commit()
        con.close()
        fake(tools, '{"api":1,"event":"pre-edit","decision":"deny","reason":"off means off"}')
        rep.check(not denied(pre_edit(sb, "s1", "e.txt")), "[companion] tracewise = \"off\" ignores it")
        rep.check("Checking companion (Tracewise)... – off" in sb.run("doctor").stdout, "and doctor says so")
    finally:
        sb.cleanup()

    # ---- the real one
    if not REAL.is_file() or "--neutral" not in subprocess.run([str(REAL), "hook", "--help"], capture_output=True, text=True).stdout:
        print("  SKIP  real Tracewise with --neutral not built (~/Developer/project/tracewise, cargo build --release)")
        return rep.finish()
    sb = Sandbox("companion-real")
    try:
        sb.env["AUTOMEMORY_TRACEWISE_BIN"] = str(REAL)
        sb.env["TRACEWISE_HOME"] = str(sb.root / "twhome")
        sb.env.pop("CLAUDECODE", None)
        (sb.project / ".tracewiseconfig").write_text("[profile]\ncoordination = true\n")
        tw = subprocess.run([str(REAL), "init"], cwd=sb.project, env=sb.env, capture_output=True, text=True)
        rep.check(tw.returncode == 0, "tracewise init in the sandbox project", tw.stdout + tw.stderr)
        sb.run("connect", "claude")
        (sb.home / ".automemory" / "config.toml").write_text("[companion]\nown_locks = false\n")
        (sb.project / "a.txt").write_text("one\n")
        a = pre_edit(sb, "sA", "a.txt", "codex")
        rep.check(not denied(a), "the first session may edit", a[1][:160])
        b = pre_edit(sb, "sB", "a.txt", "claude")
        rep.check(denied(b), "with AutoMemory's own locks off, the second session is refused by Tracewise's lock", b[1][:240])
        locks = subprocess.run([str(REAL), "lock", "list", "--json"], cwd=sb.project, env=sb.env, capture_output=True, text=True).stdout
        rep.check('"agent":"codex"' in locks.replace(" ", ""), "the lock is in Tracewise's own store, held by the first agent", locks[:200])
        import sqlite3 as _s
        _c = _s.connect(sb.home / ".automemory" / "automemory.db")
        has = _c.execute("select count(*) from sqlite_master where name = 'leases'").fetchone()[0]
        _c.close()
        rep.check(has == 0, "and AutoMemory has no lease table of its own", str(has))
        sb.hook("codex", "stop", {"session_id": "sA", "hook_event_name": "Stop", "cwd": str(sb.project)})
        c = pre_edit(sb, "sB", "a.txt", "claude")
        rep.check(not denied(c), "once the first session's turn ends, Tracewise has released the file", c[1][:160])
    finally:
        sb.cleanup()
    return rep.finish()


if __name__ == "__main__":
    sys.exit(main())
