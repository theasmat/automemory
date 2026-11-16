#!/usr/bin/env python3
"""A session in a folder that is no registered project must not be handed another project's memory (live run 2026-11-16: Codex in a scratch repo
described Ledgerly, because the server fell back to "where this agent last worked")."""
import json
import subprocess
import sys
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parents[1] / "lib"))
from harness import Report, Sandbox  # noqa: E402


def mcp_in(sb, cwd, tool, **args):
    msgs = [{"jsonrpc": "2.0", "id": 1, "method": "initialize", "params": {"protocolVersion": "2024-11-05", "capabilities": {}, "clientInfo": {"name": "codex", "version": "1"}}},
            {"jsonrpc": "2.0", "id": 2, "method": "tools/call", "params": {"name": tool, "arguments": args}}]
    r = subprocess.run([sb.bin, "mcp", "--agent", "codex"], input="\n".join(json.dumps(m) for m in msgs) + "\n", capture_output=True, text=True, cwd=cwd, env=sb.env, timeout=30)
    for line in r.stdout.splitlines():
        d = json.loads(line)
        if d.get("id") == 2:
            return d["result"]["content"][0]["text"]
    return r.stdout + r.stderr


def main():
    rep = Report("unregistered folder sees no project memory")
    sb = Sandbox("unreg")
    try:
        sb.run("connect", "codex")
        sb.mcp("decision.record", topic="pricing", decision="Zebra pricing uses integer cents", reason="exact money")
        sb.mcp("memory.save", summary="Zebra marker note", content="The zebra marker note belongs to the sandbox project", type="gotcha", scope="project")
        # the agent's hooks last saw it in the project...
        sb.hook("codex", "prompt", {"session_id": "c1", "hook_event_name": "UserPromptSubmit", "prompt": "hi", "cwd": str(sb.project)})
        # ...then a Codex session starts in a different folder that is a git repo but no project
        other = sb.root / "scratch-repo"
        other.mkdir()
        subprocess.run(["git", "init", "-q"], cwd=other)
        for tool, args in [("memory.recall", {"query": "zebra pricing"}), ("memory.search", {"query": "zebra"}), ("memory.list", {"task_query": "zebra project identity", "view": "context"}),
                           ("continuity.brief", {"action": "get"}), ("decision.get", {"topic": "pricing"}), ("memory.stats", {})]:
            out = mcp_in(sb, other, tool, **args)
            rep.check("zebra" not in out.lower() or tool == "decision.get" and "No active decision" in out, f"{tool} from an unregistered folder shows nothing of the project", out[:160])
        # the hooks too: no resume pack, brief or decisions of the project at session start or at a prompt
        sb.mcp("continuity.brief", action="add", items=[{"kind": "goal", "text": "Zebra goal of the sandbox project"}])
        sb.mcp("task.create", session_id="lead", title="Zebra task of the sandbox project", assigned_agent="codex")
        for ev, name in [("session-start", "SessionStart"), ("prompt", "UserPromptSubmit")]:
            r = sb.hook("codex", ev, {"session_id": "c9", "hook_event_name": name, "prompt": "what is this project?", "cwd": str(other)})
            rep.check("zebra" not in (r[1] or "").lower(), f"the {ev} hook in an unregistered folder carries nothing of the project", (r[1] or "")[:160])
        # the project's own folder still works
        out = mcp_in(sb, sb.project, "memory.recall", query="zebra pricing")
        rep.check("zebra" in out.lower(), "and the registered folder still finds its notes", out[:160])
        # a write from the unregistered folder is global, not filed under the other project
        mcp_in(sb, other, "memory.save", summary="Walrus note", content="walrus note written outside any project", type="gotcha", scope="project")
        rep.check("walrus" not in mcp_in(sb, sb.project, "memory.recall", query="walrus").lower(), "a note written there does not appear in the project")
        rep.check("walrus" in mcp_in(sb, other, "memory.recall", query="walrus").lower(), "but it is found again from that same folder")
        mcp_in(sb, other, "decision.record", topic="scratch-db", decision="Scratch repo uses flat files", reason="throwaway experiment")
        rep.check("flat files" in mcp_in(sb, other, "decision.get", topic="scratch-db"), "a decision made there is found there")
        rep.check("flat files" not in mcp_in(sb, sb.project, "decision.get", topic="scratch-db"), "and is not a decision of the project")
        mcp_in(sb, other, "continuity.brief", action="add", items=[{"kind": "goal", "text": "Experiment with flat files"}])
        rep.check("flat files" in mcp_in(sb, other, "continuity.brief", action="get"), "a brief kept there is found there")
        rep.check("flat files" not in mcp_in(sb, sb.project, "continuity.brief", action="get"), "and not in the project's brief")
    finally:
        sb.cleanup()
    return rep.finish()


if __name__ == "__main__":
    sys.exit(main())
