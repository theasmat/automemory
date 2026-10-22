#!/usr/bin/env python3
"""Planning-room edge cases through the real binary: how each agent family is told about a room (Claude prompt hook,
Antigravity PreInvocation injection), project scoping, the rules MCP enforces on agents, a posting race, restart
persistence, input limits on the human side, and what the announcement costs the hook when many rooms exist."""
import json
import subprocess
import sys
import threading
import time
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parents[1] / "lib"))
sys.path.insert(0, str(Path(__file__).parent))
from dashboard import free_port, http  # noqa: E402
from harness import Report, Sandbox  # noqa: E402

PW = {"X-AutoMemory-Password": "correct horse"}


def j(x):
    if isinstance(x, dict):
        return x
    try:
        return json.loads(x)
    except (TypeError, ValueError):
        return {"text": x}


def main():
    rep = Report("planning room edge cases")
    sb = Sandbox("roomedges")
    sb.env.update({"AUTOMEMORY_SECRETS_BACKEND": "file", "AUTOMEMORY_KEY": "edges-master"})
    server = None
    try:
        sb.mcp("project.register", slug="app", display_name="app", path=str(sb.project))
        other = Path(sb.root / "other")
        other.mkdir()
        subprocess.run(["git", "init", "-q", "-b", "main"], cwd=other, check=True)
        sb.mcp("project.register", slug="other", display_name="other", path=str(other))
        ev = j(sb.mcp("memory.save", summary="Shop runs one process", content="Single process", type="fact", scope="project", project_id="app"))
        ev = ev.get("id") or ev.get("memory", {}).get("id")
        subprocess.run([sb.bin, "rule", "passwd", "--from-env"], env={**sb.env, "AUTOMEMORY_PASSWORD": "correct horse"}, cwd=sb.project, capture_output=True)
        port = free_port()
        server = subprocess.Popen([sb.bin, "serve", "--port", str(port)], cwd=sb.project, env=sb.env, stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL)
        for _ in range(50):
            try:
                http(port, "/api/stats")
                break
            except OSError:
                time.sleep(0.1)

        # ── Input limits on the human side ──
        for body, why in (({"topic": "", "question": "q", "agents": ["claude"]}, "an empty topic"), ({"topic": "t", "question": "q", "agents": []}, "no invited agent"),
                          ({"topic": "t", "agents": ["claude"]}, "no question"), ({"topic": "t", "question": "   ", "agents": ["claude"]}, "a blank question"),
                          ({"topic": "t", "question": "q", "agents": ["claude"], "evidence": ["nope"]}, "evidence that does not exist")):
            code, _ = http(port, "/api/rooms", "POST", PW, body)
            rep.check(code == 400, f"starting a room with {why} is refused", str(code))
        code, body = http(port, "/api/rooms", "POST", PW, {"topic": "x" * 500, "question": "q" * 2000, "agents": ["agy"], "scope": ["a"] * 50, "project_id": "app"})
        d = json.loads(body)["room"]
        brief = json.loads(http(port, f"/api/rooms/{d['id']}")[1])["brief"]
        rep.check(code == 200 and len(d["topic"]) <= 121 and len(brief["question"]) <= 401 and len(brief["scope"]) <= 20, "topic, question and scope are capped", f"{len(d['topic'])} {len(brief['question'])} {len(brief['scope'])}")
        code, _ = http(port, f"/api/rooms/{d['id']}/decide", "POST", PW, {"decision": "   ", "steps": []})
        rep.check(code == 400, "an empty decision is refused")
        code, _ = http(port, f"/api/rooms/{d['id']}/note", "POST", PW, {"text": "   "})
        rep.check(code == 400, "an empty note is refused")
        code, _ = http(port, "/api/rooms/room-nope/decide", "POST", PW, {"decision": "x"})
        rep.check(code == 400, "deciding a room that does not exist is refused")

        # ── Announcements: Claude prompt hook vs Antigravity PreInvocation, and project scoping ──
        code, body = http(port, "/api/rooms", "POST", PW, {"topic": "cache layer", "question": "Do we cache catalog reads?", "agents": ["claude", "agy"], "evidence": [ev], "project_id": "app"})
        rid = json.loads(body)["room"]["id"]
        code, body = http(port, "/api/rooms", "POST", PW, {"topic": "other project secret topic", "question": "Anything?", "agents": ["claude", "agy"], "project_id": "other"})
        rep.check(code == 200, "a room in another project is created")
        claude = json.dumps(sb.hook("claude", "prompt", {"session_id": "c-1", "hook_event_name": "UserPromptSubmit", "prompt": "hi", "cwd": str(sb.project)})[0] or {})
        rep.check("cache layer" in claude and rid in claude and "c-1" in claude, "Claude is told, with the room id and its own session id", claude[:260])
        rep.check("other project secret topic" not in claude, "rooms of another project are not announced here")
        there = json.dumps(sb.hook("claude", "prompt", {"session_id": "c-9", "hook_event_name": "UserPromptSubmit", "prompt": "hi", "cwd": str(other)})[0] or {})
        rep.check("other project secret topic" in there and "cache layer" not in there, "and an agent working in that project is told about its own room", there[:200])
        agy_raw = sb.hook("agy", "boot", {"conversationId": "a-1", "session_id": "a-1", "invocationNum": 0, "cwd": str(sb.project)})[1]
        rep.check("injectSteps" in agy_raw and "cache layer" in agy_raw and "other project secret topic" not in agy_raw, "Antigravity gets it through its PreInvocation injection", agy_raw[:260])
        agy_again = sb.hook("agy", "boot", {"conversationId": "a-1", "session_id": "a-1", "invocationNum": 1, "cwd": str(sb.project)})[1]
        rep.check("cache layer" not in agy_again, "and only once per turn")

        # ── A room a person started before the question was required: the line says so ──
        import sqlite3
        db = sqlite3.connect(sb.home / ".automemory" / "automemory.db")
        pid = db.execute("select id from projects where slug = 'app'").fetchone()[0]
        db.execute("insert into rooms (id, project_id, topic, state, members, turn_holder, round, created_at, updated_at) values ('room-legacy', ?, 'legacy cart', 'open', '[]', NULL, 0, '2026-01-01T00:00:00Z', '2026-01-01T00:00:00Z')", (pid,))
        db.execute("insert into sync_kv (key, value) values ('room_meta:room-legacy', ?)", (json.dumps({"invited": ["codex"], "created_by": "human"}),))
        db.commit()
        db.close()
        line = json.dumps(sb.hook("codex", "prompt", {"session_id": "cx-7", "hook_event_name": "UserPromptSubmit", "prompt": "hi", "cwd": str(sb.project)})[0] or {})
        rep.check("legacy cart" in line and "No question was given" in line, "a human room without a question tells the agent so", line[:260])

        # ── MCP rules for agents ──
        r = sb.mcp("room.post", room_id=rid, session_id="c-1", kind="note", body="I am the human", evidence=[ev])
        rep.check("human" in json.dumps(r).lower() and "refused" in json.dumps(r).lower(), "an agent cannot post a human note", json.dumps(r)[:140])
        sb.mcp("room.join", session_id="c-1", agent="claude-code", room_id=rid)
        r = sb.mcp("room.post", room_id=rid, session_id="c-1", kind="propose", body="Yes, an in-process LRU (with enough detail to count)", evidence=[])
        rep.check("refused" in json.dumps(r).lower(), "a proposal still needs evidence", json.dumps(r)[:120])
        r = sb.mcp("room.post", room_id=rid, session_id="c-1", kind="propose", body="Yes, an in-process LRU (with enough detail to count)", evidence=[f"note-{rid}"])
        rep.check("refused" in json.dumps(r).lower(), "a note id cannot be used as evidence", json.dumps(r)[:120])
        for junk in ("test", "lgtm", "looks fine to me"):
            r = sb.mcp("room.post", room_id=rid, session_id="c-1", kind="propose", body=junk, evidence=[ev])
            rep.check("at least" in json.dumps(r), f"a placeholder post ({junk!r}) is refused", json.dumps(r)[:140])
        sb.mcp("room.post", room_id=rid, session_id="c-1", kind="propose", body="Yes, an in-process LRU (with enough detail to count)", evidence=[ev])

        # ── A posting race: the turn holder answers twice at once (two processes); exactly one is stored ──
        sb.mcp("room.join", session_id="a-1", agent="antigravity", room_id=rid)
        results = []

        def critique(sid):
            results.append(json.dumps(sb.mcp("room.post", room_id=rid, session_id=sid, kind="critique", body=f"critique by {sid}, with enough detail to count", evidence=[ev])))

        sb.mcp("room.join", session_id="a-2", agent="antigravity", room_id=rid)
        threads = [threading.Thread(target=critique, args=(sid,)) for sid in ("a-1", "a-1")]
        [t.start() for t in threads]
        [t.join() for t in threads]
        posts = json.loads(http(port, f"/api/rooms/{rid}")[1])["posts"]
        critiques = [p for p in posts if p["kind"] == "critique"]
        rep.check(len(critiques) == 1, "two simultaneous critiques: exactly one is stored", f"{len(critiques)} stored; {[r[:60] for r in results]}")

        # ── Restart persistence ──
        server.terminate()
        server.wait(timeout=5)
        server = subprocess.Popen([sb.bin, "serve", "--port", str(port)], cwd=sb.project, env=sb.env, stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL)
        for _ in range(50):
            try:
                http(port, "/api/stats")
                break
            except OSError:
                time.sleep(0.1)
        again = json.loads(http(port, f"/api/rooms/{rid}")[1])
        rep.check(len(again["posts"]) == 2 and again["brief"]["question"] == "Do we cache catalog reads?" and again["labels"].get("c-1", "").startswith("Claude Code"),
                  "after a daemon restart the discussion, the brief and the names are intact", json.dumps(again["labels"]))

        # ── Cost: the announcement must not slow the hook when many rooms exist ──
        for i in range(60):
            http(port, "/api/rooms", "POST", PW, {"topic": f"bulk {i}", "question": "Q?", "agents": ["codex"], "project_id": "app"})
        times = []
        for i in range(15):
            t = time.time()
            sb.hook("claude", "prompt", {"session_id": f"perf-{i}", "hook_event_name": "UserPromptSubmit", "prompt": "hi", "cwd": str(sb.project)})
            times.append((time.time() - t) * 1000)
        times.sort()
        p90 = times[int(len(times) * 0.9)]
        print(f"  prompt hook with 60+ rooms: median {times[len(times)//2]:.0f} ms, p90 {p90:.0f} ms")
        rep.check(p90 < 400, "the prompt hook stays fast with 60+ rooms (p90 under 400 ms, process start included)", f"{p90:.0f} ms")
    finally:
        if server:
            server.terminate()
            server.wait(timeout=5)
        sb.cleanup()
    return rep.finish()


if __name__ == "__main__":
    sys.exit(main())
