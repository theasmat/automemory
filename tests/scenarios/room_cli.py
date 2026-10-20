#!/usr/bin/env python3
"""`automemory room` from the terminal: list/show/approve/reject/again, each verdict behind the human password,
and the stored result (one decision, one task per step) matches what `show` previewed."""
import json
import subprocess
import sys
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parents[1] / "lib"))
from harness import Report, Sandbox  # noqa: E402


def j(x):
    if isinstance(x, dict):
        return x
    try:
        return json.loads(x)
    except (TypeError, ValueError):
        return {"text": x}


def main():
    rep = Report("room CLI")
    sb = Sandbox("roomcli")
    sb.env.update({"AUTOMEMORY_SECRETS_BACKEND": "file", "AUTOMEMORY_KEY": "cli-master"})
    try:
        sb.mcp("project.register", slug="app", display_name="app", path=str(sb.project))
        ev = j(sb.mcp("memory.save", summary="Orders in SQLite", content="orders", type="fact", scope="project", project_id="app"))
        ev = ev.get("id") or ev.get("memory", {}).get("id")

        def decided_room(topic, revision):
            rid = j(sb.mcp("room.join", session_id="a", agent="claude-code", topic=topic, project_id="app"))["room"]["id"]
            sb.mcp("room.join", session_id="b", agent="antigravity", room_id=rid)
            for who, kind, body in (("a", "propose", f"{topic}: the proposal, with enough detail"), ("b", "critique", f"{topic}: objection, with enough detail"), ("a", "revise", revision)):
                sb.mcp("room.post", room_id=rid, session_id=who, kind=kind, body=body, evidence=[ev])
            return rid

        out = sb.run("room", "list").stdout
        rep.check("No planning rooms yet" in out, "an empty list says how to start", out)
        r1 = decided_room("pagination", "Use keyset pagination\n- Add the cursor column\n- Update the list endpoint\n1. Write the tests")
        r2 = decided_room("logging", "Structured JSON logs for every service\n- Add the logger")
        r3 = decided_room("naming", "Rename the module to something clear\n- Rename it")
        lst = sb.run("room", "list").stdout
        rep.check(lst.count("WAITING FOR YOUR VERDICT") == 3 and r1 in lst, "list shows all three waiting for a verdict", lst)
        show = sb.run("room", "show", r1).stdout
        rep.check("pagination: objection, with enough detail" in show and "task: Add the cursor column" in show and "task: Write the tests" in show, "show prints the discussion and the tasks approval would create", show[-400:])
        rep.check(sb.run("room", "show", "room-missing").returncode != 0, "show of an unknown room fails")

        # Once a human password exists, every verdict is refused without it and nothing changes. (Before one is set the CLI
        # allows the action with a warning, by design.)
        subprocess.run([sb.bin, "rule", "passwd", "--from-env"], env={**sb.env, "AUTOMEMORY_PASSWORD": "correct horse"}, cwd=sb.project, capture_output=True)
        for verb in ("approve", "reject", "again"):
            r = sb.run("room", verb, r1)
            rep.check(r.returncode != 0, f"room {verb} without the password is refused", (r.stdout + r.stderr)[-120:])
        rep.check(sb.run("room", "list").stdout.count("WAITING FOR YOUR VERDICT") == 3, "and all three rooms are untouched")

        # With the password (the CLI reads it from the environment for scripts).
        env = {**sb.env, "AUTOMEMORY_PASSWORD": "correct horse"}
        run = lambda *a: subprocess.run([sb.bin, *a], capture_output=True, text=True, cwd=sb.project, env=env)
        r = run("room", "approve", r1)
        rep.check(r.returncode == 0 and "created 3 task(s)" in r.stdout, "approve stores the decision and the three tasks", r.stdout + r.stderr)
        rep.check(run("room", "approve", r1).returncode != 0, "a second approval is refused")
        decisions = sb.run("decision", "list").stdout
        rep.check("pagination" in decisions and "keyset" in decisions.lower(), "the decision is listed", decisions[:300])
        t1 = j(sb.mcp("task.next", session_id="worker", project_id="app"))
        rep.check(t1.get("task", {}).get("title") == "Add the cursor column", "tasks come out in step order", json.dumps(t1)[:140])

        r = run("room", "reject", r2)
        rep.check(r.returncode == 0 and "Nothing was stored" in r.stdout, "reject stores nothing", r.stdout + r.stderr)
        rep.check("logging" not in sb.run("decision", "list").stdout, "no decision for the rejected room")
        r = run("room", "again", r3)
        rep.check(r.returncode == 0 and "critiques next" in r.stdout, "again reopens the critique", r.stdout + r.stderr)
        lst = sb.run("room", "list").stdout
        rep.check("approved by you" in lst and "rejected by you" in lst and "critique, turn:" in lst, "list reflects every outcome", lst)
        rep.check(run("room", "reject", r1).returncode != 0, "a verdict on an approved room is refused")
    finally:
        sb.cleanup()
    return rep.finish()


if __name__ == "__main__":
    sys.exit(main())
