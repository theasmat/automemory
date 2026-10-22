#!/usr/bin/env python3
"""Planning room end to end with two simulated agents (separate MCP sessions) and a human:
turn rules, evidence rules, the agents cannot approve their own plan, the human verdict (dashboard API and CLI)
records one decision and one task per step, and a task can then be claimed by another agent."""
import json
import subprocess
import sys
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
        return {"text": x}  # tool errors come back as plain text


def main():
    rep = Report("planning room")
    sb = Sandbox("room")
    sb.env.update({"AUTOMEMORY_SECRETS_BACKEND": "file", "AUTOMEMORY_KEY": "room-master"})
    server = None
    try:
        sb.mcp("project.register", slug="app", display_name="app", path=str(sb.project))
        mem = j(sb.mcp("memory.save", summary="Sessions need sticky routing", content="Load balancer has no sticky sessions", type="fact", scope="project", project_id="app"))
        ev = mem.get("id") or mem.get("memory", {}).get("id")
        rep.check(bool(ev), "an evidence note exists", str(mem)[:120])

        # Agent A opens the room, agent B joins.
        room = j(sb.mcp("room.join", session_id="agent-a", topic="auth strategy", project_id="app"))["room"]
        rid = room["id"]
        j(sb.mcp("room.join", session_id="agent-b", room_id=rid))
        post = lambda who, kind, body, evidence=None: sb.mcp("room.post", room_id=rid, session_id=who, kind=kind, body=body, evidence=[ev] if evidence is None else evidence)

        r = post("agent-b", "propose", "I go first (with enough detail to count)")
        rep.check("refused" in json.dumps(r).lower(), "the wrong agent cannot post out of turn", json.dumps(r)[:140])
        r = post("agent-a", "propose", "no evidence (with enough detail to count)", evidence=[])
        rep.check("refused" in json.dumps(r).lower(), "a proposal without evidence is refused", json.dumps(r)[:140])
        r = post("agent-a", "propose", "fake (with enough detail to count)", evidence=["does-not-exist"])
        rep.check("refused" in json.dumps(r).lower(), "evidence that does not resolve is refused", json.dumps(r)[:140])
        post("agent-a", "propose", "Use server sessions (with enough detail to count)")
        post("agent-b", "critique", "Sessions need sticky routing, we have none")
        done = j(post("agent-a", "revise", "Use JWT in an HttpOnly cookie\n1. Add the token table\n2. Write the login route\n3. Add rotation tests"))
        rep.check(done["room"]["state"] == "decided", "after one round the room is decided", json.dumps(done)[:160])
        rep.check("human" in done["next_action"].lower() and "decision.record" in done["next_action"], "agents are told a human approves, not to record it themselves", done["next_action"])

        # Nothing is stored until the human approves.
        nt = j(sb.mcp("task.next", session_id="agent-c", project_id="app"))
        rep.check("No open tasks" in json.dumps(nt), "no tasks exist before approval", json.dumps(nt)[:100])
        subprocess.run([sb.bin, "rule", "passwd", "--from-env"], env={**sb.env, "AUTOMEMORY_PASSWORD": "correct horse"}, cwd=sb.project, capture_output=True)

        # CLI: the verdict needs the password; agents (and scripts) without it are refused.
        r = sb.run("room", "approve", rid)
        rep.check(r.returncode != 0, "the CLI refuses to approve without the password", (r.stdout + r.stderr)[-160:])
        out = sb.run("room", "show", rid).stdout
        rep.check("If you approve" in out and "task: Add the token table" in out, "room show previews the decision and tasks", out[-300:])
        rep.check("WAITING FOR YOUR VERDICT" in sb.run("room", "list").stdout, "room list says it waits for you")

        # Dashboard API.
        port = free_port()
        server = subprocess.Popen([sb.bin, "serve", "--port", str(port)], cwd=sb.project, env=sb.env, stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL)
        for _ in range(50):
            try:
                http(port, "/api/stats")
                break
            except OSError:
                time.sleep(0.1)
        code, body = http(port, "/api/rooms")
        rooms = json.loads(body)
        rep.check(code == 200 and rooms[0]["needs_you"] is True and rooms[0]["posts"] == 3, "GET /api/rooms shows it needs you", body[:160])
        code, body = http(port, f"/api/rooms/{rid}")
        d = json.loads(body)
        rep.check(len(d["posts"]) == 3 and len(d["plan"]["tasks"]) == 3 and d["plan"]["decision"].startswith("Use JWT"), "GET /api/rooms/:id has the discussion and the plan", body[:200])
        for action in ("approve", "reject", "another-round"):
            code, _ = http(port, f"/api/rooms/{rid}/{action}", "POST", {}, {})
            rep.check(code == 403, f"Protected: {action} is refused without the password", str(code))
        code, body = http(port, f"/api/rooms/{rid}/approve", "POST", PW, {})
        out = json.loads(body).get("outcome", {})
        rep.check(code == 200 and len(out.get("task_ids", [])) == 3 and out.get("decision_id"), "approving with the password stores a decision and 3 tasks", f"{code} {body[:160]}")
        code, body = http(port, "/api/decisions")
        rep.check("auth strategy" in body and "Sessions need sticky routing" in body, "the decision carries the critique as dissent", body[:240])
        code, _ = http(port, f"/api/rooms/{rid}/approve", "POST", PW, {})
        rep.check(code == 400, "a second approval is refused")

        # Another agent claims the first task.
        t = j(sb.mcp("task.next", session_id="agent-c", project_id="app"))
        rep.check(t.get("task", {}).get("title") == "Add the token table", "another agent claims the first step", json.dumps(t)[:160])
        # Holding a claimed task: the first Stop is reminded, later ones are let through (no reminder loop, seen in T11).
        first = sb.hook("claude", "stop", {"session_id": "agent-c", "hook_event_name": "Stop"})[1]
        second = sb.hook("claude", "stop", {"session_id": "agent-c", "hook_event_name": "Stop"})[1]
        rep.check('"block"' in first and "claimed task" in first and '"block"' not in second, "a claimed task is reminded once at Stop, not every time", f"{first[:100]} | {second[:100]}")

        # A lone proposer does not keep the turn; the human can note and decide at any stage; agents show by name.
        r3 = j(sb.mcp("room.join", session_id="agent-x", agent="claude-code", topic="cart storage", project_id="app"))["room"]["id"]
        sb.mcp("room.post", room_id=r3, session_id="agent-x", kind="propose", body="Keep carts in SQLite (with enough detail to count)", evidence=[ev])
        r = sb.mcp("room.post", room_id=r3, session_id="agent-x", kind="critique", body="self (with enough detail to count)", evidence=[ev])
        rep.check("another agent" in json.dumps(r), "an agent cannot critique its own proposal", json.dumps(r)[:140])
        joined = j(sb.mcp("room.join", session_id="agent-y", agent="antigravity", room_id=r3))["room"]
        rep.check(joined["turn_holder"] == "agent-y", "the next agent to join gets the turn", json.dumps(joined)[:140])
        code, _ = http(port, f"/api/rooms/{r3}/note", "POST", {}, {"text": "Carts must survive a login"})
        rep.check(code == 403, "Protected: a note needs the password")
        code, _ = http(port, f"/api/rooms/{r3}/note", "POST", PW, {"text": "Carts must survive a login"})
        st = j(sb.mcp("room.status", room_id=r3))
        rep.check(code == 200 and any(p["kind"] == "note" and "survive a login" in p["body"] for p in st["posts"]) and st["labels"]["agent-x"].startswith("Claude Code"),
                  "agents see your note and each other's names in room.status", json.dumps(st)[:200])
        d3 = json.loads(http(port, f"/api/rooms/{r3}")[1])
        rep.check(d3["labels"]["agent-y"].startswith("Antigravity") and d3["labels"]["human"] == "You", "the dashboard gets names, not session ids", json.dumps(d3["labels"]))
        code, body = http(port, f"/api/rooms/{r3}/decide", "POST", PW, {"decision": "Store carts in SQLite", "steps": ["Add cart table", "Migrate cookie carts"]})
        rep.check(code == 200 and len(json.loads(body)["outcome"]["task_ids"]) == 2, "you can decide mid-discussion; steps become tasks", f"{code} {body[:120]}")
        r = sb.mcp("room.post", room_id=r3, session_id="agent-y", kind="critique", body="late (with enough detail to count)", evidence=[ev])
        rep.check("decided" in json.dumps(r), "after your decision the agents cannot post", json.dumps(r)[:120])

        # R1: you start a room in the dashboard; invited agents learn about it from their prompt hook, once per turn.
        code, _ = http(port, "/api/rooms", "POST", {}, {"topic": "checkout retries", "question": "Q?", "agents": ["claude"]})
        rep.check(code == 403, "Protected: starting a room needs the password")
        code, body = http(port, "/api/rooms", "POST", PW, {"topic": "checkout retries", "question": "Retry failed payments or not?",
                                                         "agents": ["claude", "agy"], "evidence": [ev], "constraints": ["No new services"]})
        r4 = json.loads(body).get("room", {}).get("id")
        rep.check(code == 200 and r4, "a room started in the dashboard", f"{code} {body[:120]}")
        hook_ctx = lambda agent, sid: json.dumps(sb.hook(agent, "prompt", {"session_id": sid, "hook_event_name": "UserPromptSubmit", "prompt": "hi"})[0] or {})
        first = hook_ctx("claude", "cl-1")
        rep.check("checkout retries" in first and "proposal" in first and "Retry failed payments" in first, "the invited agent is told on its next prompt", first[:240])
        rep.check("checkout retries" not in hook_ctx("claude", "cl-1"), "and only once per turn")
        rep.check("checkout retries" not in hook_ctx("codex", "cx-1"), "an agent that was not invited is not told")
        joined = j(sb.mcp("room.join", session_id="cl-1", agent="claude-code", room_id=r4))
        rep.check(joined["room"]["turn_holder"] == "cl-1" and joined["brief"]["constraints"] == ["No new services"], "joining gives the turn and the brief", json.dumps(joined)[:200])
        sb.mcp("room.post", room_id=r4, session_id="cl-1", kind="propose", body="Retry once after 5 s (with enough detail to count)", evidence=[ev])
        agy = hook_ctx("agy", "ag-1")
        rep.check("checkout retries" in agy and "critique" in agy, "the other invited agent is told it is its turn to critique", agy[:240])

        # Human tiebreak on a second room: send it back for another round.
        r2 = j(sb.mcp("room.join", session_id="agent-a", topic="cache", project_id="app"))["room"]["id"]
        j(sb.mcp("room.join", session_id="agent-b", room_id=r2))
        for who, kind in (("agent-a", "propose"), ("agent-b", "critique"), ("agent-a", "revise")):
            sb.mcp("room.post", room_id=r2, session_id=who, kind=kind, body=f"{kind} - one step, written with enough detail", evidence=[ev])
        code, body = http(port, f"/api/rooms/{r2}/another-round", "POST", PW, {})
        room2 = json.loads(body).get("room", {})
        rep.check(code == 200 and room2.get("state") == "critique" and room2.get("turn_holder") == "agent-b", "another round hands the critique to the other agent", f"{code} {body[:140]}")

        # R2 + R3: Draft saving, structured steps with owner/files/check, lease acquisition on claim, and Revise plan superseding.
        draft_steps = [
            {"title": "Build Redis cache wrapper", "assigned_agent": "claude", "files": ["src/cache.rs"], "check_cmd": "cargo test"},
            {"title": "Add cache invalidation", "assigned_agent": None, "files": [], "check_cmd": None},
        ]
        code, _ = http(port, f"/api/rooms/{r2}/draft", "PUT", {}, {"decision": "Use Redis cache", "steps": draft_steps})
        rep.check(code == 403, "Protected: saving a draft needs the password")
        code, body = http(port, f"/api/rooms/{r2}/draft", "PUT", PW, {"decision": "Use Redis cache", "steps": draft_steps})
        rep.check(code == 200 and json.loads(body).get("draft", {}).get("decision") == "Use Redis cache", "R2: PUT /api/rooms/:id/draft persists structured steps", body[:160])
        code, body = http(port, f"/api/rooms/{r2}/approve", "POST", PW, {})
        out2 = json.loads(body).get("outcome", {})
        rep.check(code == 200 and out2.get("edited_by_human") is True and len(out2.get("task_ids", [])) == 2, "R2: approving with a saved draft marks edited_by_human and creates structured tasks", body[:180])

        # R3: Check tasks on GET /api/rooms/:id, inline PATCH, assigned_agent filtering, and file lease locking on claim.
        d_r2 = json.loads(http(port, f"/api/rooms/{r2}")[1])
        tasks_r2 = d_r2.get("tasks", [])
        rep.check(len(tasks_r2) == 2 and tasks_r2[0]["files"] == ["src/cache.rs"] and tasks_r2[0]["assigned_agent"] == "claude", "R3: GET /api/rooms/:id returns structured tasks", json.dumps(tasks_r2)[:180])
        t1_id, t2_id = tasks_r2[0]["id"], tasks_r2[1]["id"]
        code, body = http(port, f"/api/tasks/{t2_id}", "PATCH", PW, {"title": "Add TTL cache invalidation"})
        rep.check(code == 200 and json.loads(body).get("task", {}).get("title") == "Add TTL cache invalidation", "R3: PATCH /api/tasks/:id updates a task inline", body[:160])

        # Claude claims task #1 (assigned to claude) and locks src/cache.rs.
        claimed_cl = j(sb.mcp("task.next", session_id="claude-worker", project_id="app"))
        rep.check(claimed_cl.get("task", {}).get("id") == t1_id, "R3: assigned agent claims its scoped task", json.dumps(claimed_cl)[:160])
        coord = json.loads(http(port, "/api/coordination")[1])
        leases = [l["scope"] for l in coord.get("leases", [])]
        rep.check("src/cache.rs" in leases, "R3: claiming a task acquires file leases for its scoped files", json.dumps(leases))
        j(sb.mcp("task.submit", session_id="claude-worker", id=t1_id, result="cargo test passed (0 failed)"))
        d_r2_after = json.loads(http(port, f"/api/rooms/{r2}")[1])
        t1_after = [t for t in d_r2_after["tasks"] if t["id"] == t1_id][0]
        rep.check(t1_after["status"] == "submitted" and t1_after["check_status"] == "passed", "R3: submitting a task records check_status and releases leases", json.dumps(t1_after)[:160])

        # R3: Revise this plan opens a follow-up room; approving it supersedes the prior decision and cancels unclaimed tasks.
        code, body = http(port, f"/api/rooms/{r2}/revise", "POST", PW, {})
        followup_id = json.loads(body).get("room", {}).get("id")
        rep.check(code == 200 and followup_id and json.loads(body)["room"]["parent_room"] == r2, "R3: POST /api/rooms/:id/revise opens a linked follow-up room", body[:180])
        code, body = http(port, f"/api/rooms/{followup_id}/decide", "POST", PW, {"decision": "Use in-memory LRU cache instead", "steps": [{"title": "Add LRU crate", "files": ["Cargo.toml"]}]})
        rep.check(code == 200, "R3: deciding the follow-up room succeeds", f"{code} {body[:140]}")
        d_r2_sup = json.loads(http(port, f"/api/rooms/{r2}")[1])
        t2_after = [t for t in d_r2_sup["tasks"] if t["id"] == t2_id][0]
        rep.check(d_r2_sup.get("superseded_by") == followup_id and t2_after["cancelled"] is True and t1_after["status"] == "submitted",
                  "R3: approving follow-up room links superseded_by and cancels only unclaimed tasks", json.dumps(d_r2_sup)[:200])

        # R4: Multi-round & multi-critic room + skip turn + end room.
        code, body = http(port, "/api/rooms", "POST", PW, {"topic": "db pool", "question": "Pool size?", "agents": ["claude", "agy", "codex"], "rounds": 2})
        r_multi = json.loads(body)["room"]["id"]
        j(sb.mcp("room.join", session_id="s-cl", agent="claude-code", room_id=r_multi))
        j(sb.mcp("room.join", session_id="s-ag", agent="antigravity", room_id=r_multi))
        j(sb.mcp("room.join", session_id="s-cx", agent="codex", room_id=r_multi))
        sb.mcp("room.post", room_id=r_multi, session_id="s-cl", kind="propose", body="Pool size 10 (with enough detail to count)", evidence=[ev])
        after_c1 = j(sb.mcp("room.post", room_id=r_multi, session_id="s-ag", kind="critique", body="Too small under burst (with enough detail to count)", evidence=[ev]))["room"]
        rep.check(after_c1["state"] == "critique" and after_c1["turn_holder"] == "s-cx", "R4: multi-critic room rotates to the second critic before revise", json.dumps(after_c1)[:160])
        code, body = http(port, f"/api/rooms/{r_multi}/skip", "POST", PW, {})
        after_skip = json.loads(body).get("room", {})
        rep.check(code == 200 and after_skip.get("state") == "revise" and after_skip.get("turn_holder") == "s-cl", "R4: skipping the last critic advances to revise", body[:160])
        code, body = http(port, f"/api/rooms/{r_multi}/end", "POST", PW, {})
        rep.check(code == 200 and json.loads(body).get("outcome", {}).get("outcome") == "ended", "R4: POST /api/rooms/:id/end closes an undecided room", body[:160])
    finally:
        if server:
            server.terminate()
            server.wait(timeout=5)
        sb.cleanup()
    return rep.finish()


if __name__ == "__main__":
    sys.exit(main())
