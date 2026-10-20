#!/usr/bin/env python3
"""Continuity C2 end to end: reading an agent's chat files is off until a person turns that agent on; then the project's Claude
Code chats are imported incrementally (read-only, secrets and the home folder removed, thinking and tool output never kept),
searchable from the CLI and by agents (chat.search / chat.get), and forgettable."""
import hashlib
import json
import re
import shutil
import subprocess
import sys
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parents[1] / "lib"))
from harness import REPO, Report, Sandbox  # noqa: E402

FIXTURE = REPO / "tests/continuity/fixtures/claude/home/.claude/projects/-tmp-demo-shop/sess-001.jsonl"


def j(x):
    if isinstance(x, dict):
        return x
    try:
        return json.loads(x)
    except (TypeError, ValueError):
        return {"text": x}


def main():
    rep = Report("continuity chat history")
    sb = Sandbox("history")
    sb.env.update({"AUTOMEMORY_SECRETS_BACKEND": "file", "AUTOMEMORY_KEY": "hist-master"})
    try:
        root = sb.project.resolve()
        sb.mcp("project.register", slug="app", display_name="app", path=str(root))
        folder = re.sub(r"[^A-Za-z0-9]", "-", str(root))
        d = sb.home / ".claude" / "projects" / folder
        d.mkdir(parents=True)
        src = d / "sess-001.jsonl"
        shutil.copy(FIXTURE, src)
        # A message that names the (sandbox) home folder, to prove it is shown as ~.
        with src.open("a") as f:
            f.write(json.dumps({"type": "user", "isSidechain": False, "sessionId": "sess-001", "timestamp": "2026-10-01T09:02:00.000Z",
                                "message": {"role": "user", "content": f"The notes live in {sb.home}/demo/notes.txt"}}) + "\n")
        digest = lambda: hashlib.sha256(src.read_bytes()).hexdigest()
        before = digest()

        out = sb.run("continuity", "sources").stdout
        rep.check("Claude Code (claude)  off" in out and "found: 1" in out, "sources lists Claude Code as off and finds the project's chat", out)
        r = sb.run("continuity", "import", "claude")
        rep.check(r.returncode != 0 and "is off" in (r.stdout + r.stderr), "importing an agent that is off is refused", r.stdout + r.stderr)
        rep.check("No agent is turned on" in sb.run("continuity", "import").stdout, "importing with nothing on says so")
        empty = j(sb.mcp("chat.search", query="sticky", project_id="app"))
        rep.check(empty["hits"] == [] and "continuity enable" in empty["next_action"], "before any import, an agent is told how a person turns it on", json.dumps(empty)[:200])

        r = sb.run("continuity", "enable", "claude")
        rep.check(r.returncode == 0 and "may now read Claude Code" in r.stdout, "a person turns Claude Code on", r.stdout + r.stderr)
        r = sb.run("continuity", "import")
        m = re.search(r"(\d+) new message", r.stdout)
        first = int(m.group(1)) if m else 0
        rep.check(r.returncode == 0 and first >= 5, "the first import stores the conversation", r.stdout + r.stderr)
        rep.check("0 new message" in sb.run("continuity", "import").stdout, "importing again adds nothing")
        rep.check(digest() == before, "the agent's chat file is unchanged (read-only)")

        hits = sb.run("continuity", "search", "sticky routing").stdout
        rep.check("claude:sess-001" in hits and "Plan the demo shop cart" in hits and "sticky" in hits, "search finds the exchange with its title", hits)
        rep.check("FIXTURESECRET" not in sb.run("continuity", "search", "FIXTURESECRET").stdout and "FIXTURESECRET" not in sb.run("continuity", "search", "payment sandbox").stdout, "the planted key is not stored", "")
        notes = sb.run("continuity", "search", "notes txt").stdout
        rep.check("~/demo/" in notes and str(sb.home) not in notes, "the home folder is shown as ~", notes)
        rep.check("PRIVATE REASONING" not in sb.run("continuity", "search", "PRIVATE REASONING").stdout and "sub-agent chatter" not in sb.run("continuity", "search", "chatter").stdout, "thinking and side-chain chatter were never stored")
        rep.check("TOOL OUTPUT" not in sb.run("continuity", "search", "TOOL OUTPUT").stdout, "nor tool output")

        # Agents reach it through MCP.
        s = j(sb.mcp("chat.search", query="sticky routing", project_id="app"))
        rep.check(len(s["hits"]) >= 1 and s["hits"][0]["agent"] == "claude", "chat.search returns hits for an agent", json.dumps(s)[:240])
        g = j(sb.mcp("chat.get", session=s["hits"][0]["session"], from_seq=max(0, s["hits"][0]["seq"] - 1), limit=4))
        rep.check(len(g["messages"]) >= 2 and g["title"] == "Plan the demo shop cart", "chat.get opens the exchange in order", json.dumps(g)[:240])
        rep.check("no such session" in json.dumps(j(sb.mcp("chat.get", session="claude:nope"))).lower(), "an unknown session is an error")

        # Incremental: the agent keeps talking.
        with src.open("a") as f:
            f.write("not json at all\n")
            f.write(json.dumps({"type": "assistant", "isSidechain": False, "sessionId": "sess-001", "timestamp": "2026-10-01T10:00:00.000Z",
                                "message": {"role": "assistant", "content": [{"type": "text", "text": "Later decision: keep the cleanup job in cron, not in the request path."}]}}) + "\n")
        r = sb.run("continuity", "import")
        rep.check("1 new message" in r.stdout, "a later import adds only the new message (a corrupt line is skipped)", r.stdout)
        rep.check("cron" in sb.run("continuity", "search", "cleanup cron").stdout, "and it is searchable")

        # Control: password gate, disable, forget.
        env = {**sb.env, "AUTOMEMORY_PASSWORD": "correct horse"}
        subprocess.run([sb.bin, "rule", "passwd", "--from-env"], env=env, cwd=sb.project, capture_output=True)
        rep.check(sb.run("continuity", "disable", "claude").returncode != 0, "with a password set, turning an agent off needs it")
        rep.check(sb.run("continuity", "history-forget").returncode != 0, "so does deleting history")
        r = subprocess.run([sb.bin, "continuity", "disable", "claude"], cwd=sb.project, env=env, capture_output=True, text=True)
        rep.check(r.returncode == 0 and "no longer read" in r.stdout, "with it, disable works", r.stdout + r.stderr)
        with src.open("a") as f:
            f.write(json.dumps({"type": "user", "isSidechain": False, "sessionId": "sess-001", "message": {"role": "user", "content": "Written after it was turned off"}}) + "\n")
        rep.check("is off" in (sb.run("continuity", "import", "claude").stdout + sb.run("continuity", "import", "claude").stderr), "a disabled agent is not read any more")
        r = subprocess.run([sb.bin, "continuity", "history-forget", "--agent", "claude"], cwd=sb.project, env=env, capture_output=True, text=True)
        rep.check(r.returncode == 0 and "Deleted" in r.stdout, "forgetting deletes the imported history", r.stdout + r.stderr)
        rep.check("Nothing found" in sb.run("continuity", "search", "sticky").stdout and j(sb.mcp("chat.search", query="sticky", project_id="app"))["hits"] == [], "and it is gone from search")
    finally:
        sb.cleanup()
    return rep.finish()


if __name__ == "__main__":
    sys.exit(main())
