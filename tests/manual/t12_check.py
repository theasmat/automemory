#!/usr/bin/env python3
"""Trial T12: a long Antigravity chat continued in Claude Code. Read-only checks on the AutoMemory database, plus an answer check.

Steps (you do 1-4, this script does the rest):
  1. In the project folder: automemory continuity enable agy   (password), then: automemory continuity import
  2. Look at the Continuity page (or `automemory continuity show`), confirm or fix the drafts, press Hand off now.
  3. Open Claude Code in the same project and ask: "Where did we get to? What did we rule out and why? What is the next step?"
  4. Save Claude's reply to a file.
Usage: t12_check.py [--db path] [--answer reply.txt]      (default db ~/.automemory/automemory.db)
"""
import re
import sqlite3
import sys
from pathlib import Path

args = sys.argv[1:]
opt = lambda name: args[args.index(name) + 1] if name in args else None
db = Path(opt("--db") or Path.home() / ".automemory" / "automemory.db")
conn = sqlite3.connect(f"file:{db}?mode=ro", uri=True)
failed = 0


def check(ok, label, detail=""):
    global failed
    failed += 0 if ok else 1
    print(f"  {'PASS' if ok else 'FAIL'}  {label}" + (f"  [{detail}]" if detail and not ok else ""))


sessions = conn.execute("select agent, count(*), sum(messages) from chat_sessions group by agent").fetchall()
print("Imported chats:", ", ".join(f"{a}: {n} session(s), {m} message(s)" for a, n, m in sessions) or "none")
check(any(a == "agy" and (m or 0) > 20 for a, n, m in sessions), "a long Antigravity chat was imported (more than 20 messages)")
items = conn.execute("select kind, text, state, origin, author from brief_items where status = 'active'").fetchall()
for k, t, st, o, a in items:
    print(f"   {k:10} {st:9} {o:10} {t[:90]}")
kinds = {k for k, *_ in items}
check({"goal", "next"} <= kinds, "the brief has a goal and a next step", ", ".join(sorted(kinds)))
check(any(k == "rejected" for k, *_ in items), "the brief says what was ruled out")
check(any(st != "draft" for _, _, st, *_ in items), "at least one item was confirmed or edited by you or an agent")
check(conn.execute("select count(*) from sync_kv where key like 'resume_shown:%'").fetchone()[0] > 0, "a new session was given the resume pack (hook ran)")
leak = conn.execute("select count(*) from chat_messages where text like '%sk-%' or text like '%BEGIN PRIVATE KEY%'").fetchone()[0]
check(leak == 0, "no key-like text in the stored chat", f"{leak} message(s)")

ans = opt("--answer")
if ans:
    text = Path(ans).read_text().lower()
    words = lambda s: {w for w in re.findall(r"[a-z]{5,}", s.lower())}
    for kind in ("goal", "rejected", "next"):
        for k, t, *_ in [i for i in items if i[0] == kind][:3]:
            w = words(t)
            hit = len([x for x in w if x in text])
            check(not w or hit / len(w) >= 0.5, f"Claude's reply covers the {kind}: {t[:60]}", f"{hit}/{len(w)} words")
else:
    print("(pass --answer reply.txt to check Claude's reply against the brief)")
sys.exit(1 if failed else 0)
