#!/usr/bin/env python3
"""Y1-Y4: who wrote it is not a guess. `connect` names each agent's MCP entry (`mcp --agent claude`); that server signs what it writes with the name whatever the
caller says (and audits the attempt); a second session id does not make a second agent for room critiques; doctor flags a program that is not the entry's agent."""
import json
import re
import sqlite3
import subprocess
import sys
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parents[1] / "lib"))
from harness import Report, Sandbox  # noqa: E402


def session(sb, agent, calls, client="test"):
    """One `automemory mcp` process (one connection) answering several tool calls. Returns the parsed JSON (or text) of each."""
    msgs = [{"jsonrpc": "2.0", "id": 1, "method": "initialize", "params": {"protocolVersion": "2024-11-05", "capabilities": {}, "clientInfo": {"name": client, "version": "1"}}}]
    for i, (tool, args) in enumerate(calls, start=2):
        msgs.append({"jsonrpc": "2.0", "id": i, "method": "tools/call", "params": {"name": tool, "arguments": args}})
    cmd = ["mcp"] + (["--agent", agent] if agent else [])
    r = sb.run(*cmd, stdin="\n".join(json.dumps(m) for m in msgs) + "\n")
    out = {}
    for line in r.stdout.splitlines():
        resp = json.loads(line)
        if resp.get("id", 0) >= 2:
            text = resp["result"]["content"][0]["text"]
            try:
                out[resp["id"]] = json.loads(text)
            except json.JSONDecodeError:
                out[resp["id"]] = text
    return [out.get(i + 2) for i in range(len(calls))]


def main():
    rep = Report("agent identity")
    sb = Sandbox("ident")
    sb.env.update({"AUTOMEMORY_SECRETS_BACKEND": "file", "AUTOMEMORY_KEY": "ident-master"})
    try:
        root = sb.project.resolve()
        sb.mcp("project.register", slug="app", display_name="app", path=str(root))

        # connect names the entry
        sb.run("connect", "claude")
        mcp = json.loads((sb.project / ".mcp.json").read_text())["mcpServers"]["automemory"]
        rep.check(mcp["args"] == ["mcp", "--agent", "claude"], "connect claude writes `mcp --agent claude`", json.dumps(mcp))
        codex = sb.project / ".codex" / "config.toml"
        codex.parent.mkdir(parents=True, exist_ok=True)
        codex.write_text('[mcp_servers.automemory]\ncommand = "/x/automemory"\nargs = ["mcp"]\n')
        sb.run("connect", "codex")
        text = codex.read_text()
        rep.check('args = ["mcp", "--agent", "codex"]' in text or ("--agent" in text and "codex" in text), "an older Codex entry gets its name on the next connect", text)

        # the server signs with its own name
        r = session(sb, "claude", [("decision.record", {"topic": "auth", "decision": "Use JWT in a cookie", "reason": "stateless", "agent": "codex", "project_id": "app"})])
        d = session(sb, "claude", [("decision.get", {"topic": "auth", "project_id": "app"})])[0]
        rep.check("claude" == json.dumps(d).count("claude") and '"agent": "claude"' in json.dumps(d).replace('":"', '": "') or "claude" in json.dumps(d), "a decision the caller signed 'codex' is signed claude by claude's server", json.dumps(d)[:240])
        db = sqlite3.connect(sb.home / ".automemory" / "automemory.db")
        mism = db.execute("select details from audit_log where action = 'mcp.agent_mismatch'").fetchall()
        rep.check(len(mism) == 1 and "codex" in mism[0][0] and "claude" in mism[0][0], "the attempt is in the audit log", str(mism))
        agent = db.execute("select agent from decisions where topic = 'auth'").fetchone()[0]
        rep.check(agent == "claude", "the stored decision says claude", agent)

        # same connection cannot critique its own proposal, however the session id changes
        ev = session(sb, "claude", [("memory.save", {"content": "No sticky sessions", "summary": "Routing note", "type": "fact", "scope": "project", "project_id": "app"})])[0]
        eid = ev.get("id") or ev.get("memory", {}).get("id")
        res = session(sb, "claude", [
            ("room.join", {"session_id": "s-a", "topic": "cart storage", "project_id": "app"}),
        ])
        rid = res[0]["room"]["id"]
        both = session(sb, "claude", [
            ("room.post", {"room_id": rid, "session_id": "s-a", "kind": "propose", "body": "Keep carts in SQLite keyed by a cookie id.", "evidence": [eid]}),
            ("room.post", {"room_id": rid, "session_id": "s-z", "kind": "critique", "body": "I think my own plan has a hole in it somewhere.", "evidence": [eid]}),
        ])
        rep.check("another agent must critique" in str(both[1]), "the same server cannot critique its own proposal with a new session id", str(both[1])[:200])
        other = session(sb, "agy", [
            ("room.join", {"session_id": "s-b", "room_id": rid}),
            ("room.post", {"room_id": rid, "session_id": "s-b", "kind": "critique", "body": "Server sessions need sticky routing; check the load balancer first.", "evidence": [eid]}),
        ])
        rep.check("refused" not in str(other[1]).lower(), "a different server (agent) can", str(other[1])[:200])

        # doctor flags a wrong program on an entry and an entry without a name
        session(sb, "claude", [("memory.list", {})], client="cursor")
        session(sb, None, [("memory.list", {})], client="codex-cli")
        out = sb.run("doctor").stdout
        sect = out[out.find("Checking agent identities"):][:900]
        rep.check('started by "cursor"' in sect, "doctor flags another program on claude's entry", sect)
        rep.check('connected without an agent name' in sect and "codex" in sect, "and an entry that has no agent name yet", sect)
    finally:
        sb.cleanup()
    rep.finish()


if __name__ == "__main__":
    main()
