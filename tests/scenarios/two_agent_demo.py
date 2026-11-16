#!/usr/bin/env python3
"""A narrated story of two agents (Claude Code and Antigravity) sharing one project, checked as it runs.

Safe to show a friend: everything happens in a throwaway HOME and git repo, never in ~/.automemory.
  python3 tests/scenarios/two_agent_demo.py            (add DEMO_PAUSE=1 to wait for Enter between acts)
Acts: 1 shared decision, 2 a gotcha both see, 3 a file collision, 4 a hand-off, 5 what the dashboard would show.
"""
import json
import os
import sys
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parents[1] / "lib"))
from harness import Report, Sandbox  # noqa: E402

PAUSE = bool(os.environ.get("DEMO_PAUSE"))


def act(title):
    print(f"\n--- {title}")
    if PAUSE:
        input("    (Enter to run) ")


def say(text):
    print(f"    {text}")


def edit(sb, agent, session, path):
    return sb.hook(agent, "pre-tool", {"session_id": session, "tool_name": "Edit", "tool_input": {"file_path": path}})


def main():
    sb = Sandbox("friend-demo")
    rep = Report("two agents, one memory")
    sb.env.update({"AUTOMEMORY_SECRETS_BACKEND": "file", "AUTOMEMORY_KEY": "demo-master"})
    try:
        sb.run("connect", "claude")
        sb.mcp("project.register", slug="shop", display_name="shop", path=str(sb.project.resolve()))

        act("1. Claude Code records a decision")
        say("Claude Code: 'we keep carts in SQLite, not Postgres, because the app is local-first'")
        sb.mcp("decision.record", topic="cart storage", decision="Keep carts in SQLite", reason="the app is local-first",
               agent="claude-code", project_id="shop")
        found = sb.mcp("memory.recall", query="carts", project_id="shop", include_untrusted=True, agent="antigravity")
        say("Antigravity asks: 'where do we store carts?'")
        rep.check("SQLite" in json.dumps(found), "Antigravity recalls Claude Code's decision", json.dumps(found)[:160])

        act("2. Antigravity leaves a gotcha")
        say("Antigravity: 'the payments sandbox rejects amounts under 50 cents'")
        sb.mcp("memory.save", summary="Payments sandbox minimum", content="The payments sandbox rejects amounts under 50 cents",
               type="gotcha", scope="project", project_id="shop", agent="antigravity")
        found = sb.mcp("memory.recall", query="payments sandbox", project_id="shop", include_untrusted=True, agent="claude-code")
        rep.check("50 cents" in json.dumps(found), "Claude Code recalls Antigravity's gotcha", json.dumps(found)[:160])

        act("3. Both agents reach for the same file")
        edit(sb, "claude", "cc-1", "src/cart.rs")
        say("Claude Code starts editing src/cart.rs (locks are Tracewise's; see companion.py)")
        _, raw, _ = edit(sb, "agy", "ag-1", "src/cart.rs")
        say(f"Antigravity tries the same file and is told: {raw[:110]}")
        rep.check("leased" not in raw and "locked" not in raw, "without Tracewise AutoMemory does not lock: the edit goes through", raw[:200])

        act("4. Claude Code hands the work over")
        sb.mcp("continuity.brief", action="add", project_id="shop", agent="claude-code", session_id="cc-1",
               items=[{"kind": "goal", "text": "Ship the cart page with coupons"},
                      {"kind": "next", "text": "Add the coupon field to the cart page"},
                      {"kind": "rejected", "text": "Postgres: too heavy for a local-first app"}])
        sb.hook("claude", "session-end", {"session_id": "cc-1"})
        brief = json.dumps(sb.mcp("memory.resume", project_id="shop", agent="antigravity"))
        say("Antigravity starts a new session and is given:")
        for line in [l for l in brief.split("\\n") if "coupon" in l or "Postgres" in l][:3]:
            say("  " + line.strip().rstrip('"}'))
        rep.check("coupon" in brief and "Postgres" in brief, "the next step and the ruled-out idea reach Antigravity", brief[:900])

        act("5. What the dashboard shows")
        stats = sb.run("memory", "list", "--project", "shop").stdout
        say(stats.strip()[:300])
        rep.check("SQLite" in stats and "Payments" in stats, "both agents' notes are in one list", stats[:200])
        say("Run `automemory serve` on your own machine to see the same thing on the Notes and Agents pages.")
    finally:
        sb.cleanup()
    return rep.finish()


if __name__ == "__main__":
    sys.exit(main())
