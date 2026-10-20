#!/usr/bin/env python3
"""Checks trial T11b from the AutoMemory database after you ran it: a room started in the dashboard, invited agents told by
their hooks (no room id pasted), proposal -> critique -> revision by the two agents, in order, with their names.
Read-only. Usage: t11b_check.py [path/to/automemory.db]   (default ~/.automemory/automemory.db)
"""
import json
import sqlite3
import sys
from pathlib import Path

db = Path(sys.argv[1]) if len(sys.argv) > 1 else Path.home() / ".automemory" / "automemory.db"
conn = sqlite3.connect(f"file:{db}?mode=ro", uri=True)
kv = lambda k: (conn.execute("select value from sync_kv where key = ?", (k,)).fetchone() or [None])[0]
failed = 0


def check(ok, label, detail=""):
    global failed
    failed += 0 if ok else 1
    print(f"  {'PASS' if ok else 'FAIL'}  {label}" + (f"  [{detail}]" if detail and not ok else ""))


rooms = conn.execute("select id, topic, state, members, created_at from rooms order by created_at desc").fetchall()
started = [(r, kv(f"room_meta:{r[0]}")) for r in rooms if kv(f"room_meta:{r[0]}")]
if not started:
    print("No room started from the dashboard yet (no room_meta). Click New room on /rooms first.")
    sys.exit(2)
(rid, topic, state, members, created), meta = started[0]
meta = json.loads(meta)
members = json.loads(members)
posts = conn.execute("select kind, author_session, body, evidence from room_posts where room_id = ? order by created_at", (rid,)).fetchall()
agents_of = {}
for sid in set(m for m in members) | {p[1] for p in posts}:
    row = conn.execute("select agent_adapter from tool_events where session_id = ? limit 1", (sid,)).fetchone()
    agents_of[sid] = (row or ["?"])[0]
print(f"Room {rid}: \"{topic}\" [{state}] invited: {', '.join(meta['invited'])}")
for k, a, body, ev in posts:
    print(f"   {k:8} by {agents_of.get(a, '?'):7} ({a[:8]}): {body.splitlines()[0][:70] if body else ''}")

kinds = [p[0] for p in posts if p[0] != "note"]
check(len(meta["invited"]) >= 2, "the room was started in the dashboard with two invited agents (the question is optional)", json.dumps(meta)[:120])
told = conn.execute("select key from sync_kv where key like ?", (f"room_told:%:{rid}:%",)).fetchall()
told_sessions = {k[0].split(":")[1] for k in told}
check(len(told_sessions) >= 2, "the hooks told at least two different agent sessions about the room", f"told: {sorted(s[:8] for s in told_sessions)}")
check(kinds[:3] == ["propose", "critique", "revise"], "posts came in order: proposal, critique, revision", str(kinds))
authors = [p[1] for p in posts if p[0] != "note"]
check(len(set(authors[:3])) == 2 and authors[0] == authors[2] and authors[0] != authors[1], "two different agents took part (proposer revises, other critiques)", str([a[:8] for a in authors]))
check({agents_of.get(a) for a in authors[:3]} >= {"claude", "agy"}, "one was Claude Code and the other Antigravity", str({agents_of.get(a) for a in authors[:3]}))
check(all(json.loads(p[3]) for p in posts if p[0] in ("propose", "critique", "revise")), "every post cites evidence")
rev = [p for p in posts if p[0] == "revise"]
check(bool(rev) and any(l.strip().startswith(("-", "1.", "*")) for l in rev[0][2].splitlines()), "the revision lists steps (- step lines)")
check(state == "decided", "the room reached decided (waiting for your verdict)", state)
print("\nPASS: T11b" if not failed else f"\n{failed} check(s) failed")
sys.exit(1 if failed else 0)
