#!/usr/bin/env python3
"""A long task passed between two agents: what the second agent is told, what it can recall, and what survives a compaction."""
import json
import sqlite3
import sys
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parents[1] / "lib"))
from harness import Report, Sandbox  # noqa: E402


def main():
    rep = Report("long task across two agents")
    sb = Sandbox("longtask")
    sb.env.update({"AUTOMEMORY_SECRETS_BACKEND": "file", "AUTOMEMORY_KEY": "long"})
    try:
        root = sb.project.resolve()
        sb.mcp("project.register", slug="shop", display_name="shop", path=str(root))

        def prompt(agent, session, text):
            out = sb.hook(agent, "prompt", {"session_id": session, "hook_event_name": "UserPromptSubmit", "prompt": text, "cwd": str(root)})[0] or {}
            return out.get("hookSpecificOutput", {}).get("additionalContext", "")

        # Agent A works for a while.
        (root / "shop").mkdir()
        for f in ("refund.py", "webhook.py"):
            (root / "shop" / f).write_text("x")
            sb.hook("claude", "post-tool", {"session_id": "a1", "tool_name": "Write", "tool_input": {"file_path": str(root / "shop" / f)}, "tool_response": {}, "cwd": str(root)})
        sb.mcp("decision.record", project_id="shop", topic="refund idempotency", decision="Key refunds by Stripe event id", reason="Stripe retries a webhook, so a missing check refunded twice", alternatives=[], agent="claude")
        sb.mcp("memory.save", project_id="shop", type="lesson", summary="Verify the webhook signature before parsing", content="Parsing JSON first let a forged body through; verify the raw bytes first.", agent="claude")
        sb.mcp("continuity.brief", action="add", project_id="shop", agent="claude", session_id="a1", items=[
            {"kind": "goal", "text": "Ship partial refunds with an admin screen"},
            {"kind": "rejected", "text": "Refund through a job queue", "reason": "needs a broker"},
            {"kind": "next", "text": "Admin screen: list refunds"}])
        first = prompt("claude", "a1", "start the refund work")
        rep.check("Ship partial refunds" in first, "the first prompt carries the brief")
        rep.check("Ship partial refunds" not in prompt("claude", "a1", "go on"), "and later prompts of the same session do not repeat it")
        sb.hook("claude", "pre-compact", {"session_id": "a1", "cwd": str(root)})
        rep.check("Ship partial refunds" in prompt("claude", "a1", "after compaction"), "after a compaction the next prompt hands it over again")
        sb.hook("claude", "stop", {"session_id": "a1", "cwd": str(root)})
        sb.hook("claude", "session-end", {"session_id": "a1", "cwd": str(root)})

        # Agent B starts fresh and shares nothing but AutoMemory.
        b = prompt("codex", "b1", "continue the refund work")
        rep.check("Ship partial refunds" in b and "Admin screen: list refunds" in b, "the second agent is told the goal and the next step", b[:300])
        rep.check("Refund through a job queue" in b and "needs a broker" in b, "and what was ruled out, with the reason")
        rep.check("Key refunds by Stripe event id" in b, "and the decision in force")
        rep.check("Where it stands" in b and "Edited 2 file(s)" in b, "and where it stands, drafted from the checkpoint when the agent wrote no status", b[:900])
        rep.check("shop/refund.py" in b and str(root) not in b, "and which files were touched, as project-relative paths", b[:500])
        # A long session: the pack is handed over again after enough prompts, for agents that never say they compacted.
        seen = [i for i in range(1, 60) if "Ship partial refunds" in prompt("codex", "b1", f"step {i}")]
        rep.check(len(seen) == 2 and 20 <= seen[0] <= 26, "a long session is told the brief again after about 25 prompts (and again 25 later)", str(seen))

        # A later session brings back something the brief rules out: it is flagged for a person, not blocked.
        r = sb.mcp("decision.record", project_id="shop", topic="refund-processing", decision="Process refunds in a background job queue worker", reason="faster", agent="codex")
        rep.check(isinstance(r, dict) and r.get("status") == "recorded" and any("job queue" in w for w in r.get("warnings", [])), "a decision that brings back a ruled-out option comes back with a warning, and is still recorded", json.dumps(r)[:300])
        con = sqlite3.connect(sb.home / ".automemory" / "automemory.db")
        pending = con.execute("select description from review_proposals where proposal_type = 'rule_conflict' and status = 'pending'").fetchall()
        con.close()
        rep.check(len(pending) == 1 and "ruled out" in pending[0][0], "and a person is asked in the Inbox", str(pending))
        ok = sb.mcp("decision.record", project_id="shop", topic="refund-admin", decision="Admin screen lists refunds with a partial refund button", reason="the goal", agent="codex")
        rep.check(isinstance(ok, dict) and "warnings" not in ok, "a decision that does not touch a ruled-out option has no warning")

        # Another project's decisions and notes must not leak into this folder's session when no project is named.
        other = sb.root / "other"
        other.mkdir()
        sb.mcp("project.register", slug="other", display_name="other", path=str(other))
        sb.mcp("decision.record", project_id="other", topic="invoice-pricing", decision="No tax and no discounts in the other project", reason="its own scope", agent="codex")
        got = sb.mcp("decision.get", topic="invoice-pricing")
        rep.check(not (isinstance(got, dict) and got.get("active")), "decision.get without a project does not return another project's decision", json.dumps(got)[:200])
        leaked = sb.mcp("memory.recall", query="no tax and no discounts in the other project")
        rep.check(not any("other project" in m.get("summary", "") for m in (leaked if isinstance(leaked, list) else [])), "memory.recall without a project does not return another project's notes", json.dumps(leaked)[:200])
        ctx = sb.mcp("memory.list", view="context")
        rep.check("other project" not in json.dumps(ctx), "memory.list view=context does not either")
        # Two unrelated projects that both have a src/app.py must not collide on it, nor see each other's locks.
        sb.mcp("lock.acquire", project_id="other", scope="src/app.py", goal="editing the other project's app", agent="claude", session_id="x-other")
        mine = sb.mcp("lock.acquire", scope="src/app.py", goal="editing this project's app", agent="codex", session_id="x-mine")
        rep.check(json.dumps(mine).lower().find("conflict") == -1, "a lock on the same relative path in this folder's project is not blocked by another project's lock", json.dumps(mine)[:240])
        status = sb.mcp("lock.status")
        rep.check("other project's app" not in json.dumps(status), "lock.status without a project lists only this project's locks", json.dumps(status)[:240])

        # The "Last session" line must not repeat a next step the brief has since replaced.
        sb.hook("claude", "stop", {"session_id": "a1", "cwd": str(root)})
        pack = sb.run("resume").stdout
        last = next((l for l in pack.splitlines() if l.startswith("Last session")), "")
        rep.check("Next:" not in last, "the 'Last session' line does not repeat the stale next step once the brief has its own", last[:300])

        # The terminal commands an agent falls back on scope the same way, and accept the forms agents guess.
        r = sb.run("decision", "list")
        rep.check("No tax and no discounts" not in r.stdout, "`decision list` shows this folder's project, not another's", r.stdout[:200])
        rep.check("No tax and no discounts" in sb.run("decision", "list", "--all").stdout, "and `--all` shows every project's")
        did = sqlite3.connect(sb.home / ".automemory" / "automemory.db").execute("select id from decisions where topic = 'refund-admin'").fetchone()[0]
        rep.check("refund-admin" in sb.run("decision", "show", did).stdout, "`decision show <id>` works (agents guess it)")
        rep.check(sb.run("library", "list").returncode == 0, "`library list` works as an alias of `library ls`")

        recalled = sb.mcp("memory.recall", query="what must happen before parsing a webhook body", project_id="shop")
        lesson = [m for m in recalled if "signature" in m["summary"]]
        rep.check(bool(lesson) and "written by claude" in lesson[0]["label"], "the other agent's lesson can be recalled, labelled as unverified and by whom", json.dumps(recalled)[:300])
        rep.check(lesson[0]["trust_tier"] == 4 if lesson else False, "and it is still a tier 4 claim, not a fact")
    finally:
        sb.cleanup()
    return rep.finish()


if __name__ == "__main__":
    sys.exit(main())
