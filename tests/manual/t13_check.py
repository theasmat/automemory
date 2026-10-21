#!/usr/bin/env python3
"""Trial T13 checker: the project library with real agents (Claude Code, Antigravity, Codex). Read-only on the AutoMemory database.

Usage: t13_check.py [--db ~/.automemory/automemory.db] [--project /path/to/trial/project] [--answers DIR]
DIR may hold claude.txt, agy.txt, codex.txt: each agent's reply to the read question (see TRIAL_LOG.md, T13). Only the agents with a file are judged.
"""
import json
import sqlite3
import subprocess
import sys
from pathlib import Path

args = sys.argv[1:]
opt = lambda name, default=None: args[args.index(name) + 1] if name in args else default
db = Path(opt("--db", str(Path.home() / ".automemory" / "automemory.db")))
project = Path(opt("--project", "."))
answers = Path(opt("--answers")) if opt("--answers") else None
conn = sqlite3.connect(f"file:{db}?mode=ro", uri=True)
failed = warned = 0


def check(ok, label, detail=""):
    global failed
    failed += 0 if ok else 1
    print(f"  {'PASS' if ok else 'FAIL'}  {label}" + (f"  [{detail}]" if detail and not ok else ""))


def warn(ok, label, detail=""):
    global warned
    warned += 0 if ok else 1
    print(f"  {'PASS' if ok else 'WARN'}  {label}" + (f"  [{detail}]" if detail and not ok else ""))


files = {r[0]: r for r in conn.execute("select path, origin, author, link, scope, status from library_files where status = 'active'")}
print("Library:", ", ".join(f"{p} ({'in place' if r[3] else r[1] + ':' + r[2]})" for p, r in sorted(files.items())) or "empty")
check("plans/prd-cart.md" in files and files["plans/prd-cart.md"][1] == "human", "the PRD is in the library as your file")
check("PRD-private.md" in files and files["PRD-private.md"][3] == 1, "the git-ignored file is kept in place")

# Git stays clean.
st = subprocess.run(["git", "status", "--porcelain", "-uall"], cwd=project, capture_output=True, text=True).stdout
check(".automemory" not in st, "git status shows nothing from .automemory", st[:200])
ign = subprocess.run(["git", "check-ignore", "-q", ".automemory/library/INDEX.md"], cwd=project).returncode == 0
check(ign, "the library folder is excluded locally (.git/info/exclude)")
check(not any(l.strip().startswith(".automemory") for l in (project / ".gitignore").read_text().splitlines()) if (project / ".gitignore").exists() else True, ".gitignore was not touched by AutoMemory")

# Which agents used library tools.
calls = conn.execute("select agent_adapter, tool_name, count(*) from tool_events where tool_name like '%library%' group by 1, 2").fetchall()
by_agent = {}
for a, t, n in calls:
    by_agent.setdefault(a, []).append(f"{t.split('__')[-1]}x{n}")
for a, ts in sorted(by_agent.items()):
    print(f"   {a:10} library calls: {', '.join(ts)}")

# Agent-written files and suggestions.
agent_files = [(p, r[2]) for p, r in files.items() if r[1] == "agent"]
print("Agent-written files:", ", ".join(f"{p} by {a}" for p, a in agent_files) or "none")
warn(bool(agent_files), "at least one agent saved a note with library.put")
props = conn.execute("select id, status, description from review_proposals where proposal_type = 'library_edit'").fetchall()
for i, s, d in props:
    print(f"   suggestion {i[-6:]} [{s}] {d.splitlines()[0][:110]}")
warn(bool(props), "at least one agent suggested a change with library.propose")
prd = conn.execute("select id from library_files where path = 'plans/prd-cart.md' and status = 'active'").fetchone()
revs = conn.execute("select rev, author, origin from library_revisions where file_id = ? order by rev", (prd[0],)).fetchall() if prd else []
warn(any(a.startswith("approved from") for _, a, _ in revs), "you approved a suggestion (a revision says 'approved from <agent>')")
bypass = [a for _, a, _ in revs if a == "edited in folder"]
warn(not bypass, "no agent edited the PRD copy directly in .automemory/library", f"{len(bypass)} revision(s) 'edited in folder': an agent bypassed library.propose; record which one")

# Replies.
if answers:
    need = {"claude": ["claude", "claude-code"], "agy": ["agy", "antigravity"], "codex": ["codex"]}
    for name, aliases in need.items():
        f = answers / f"{name}.txt"
        if not f.exists():
            continue
        text = f.read_text().lower()
        check("37 days" in text and "redis" in text, f"{name}: the reply has the PRD facts (37 days, Redis rejected)")
        check("pelican-2290" in text, f"{name}: the reply has the private file's marker")
        used = any(any(al in a.lower() for al in aliases) for a in by_agent)
        warn(used, f"{name}: a library tool call was recorded (else it read INDEX.md or the file directly)")
else:
    print("(pass --answers DIR with claude.txt, agy.txt, codex.txt to judge the replies)")

print(f"\n{'PASS' if not failed else 'FAIL'}: T13 ({failed} failed, {warned} warning(s))")
sys.exit(1 if failed else 0)
