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




def antigravity():
    """C3: a long Antigravity chat becomes searchable for a Claude Code session, and its own plan is imported."""
    sys.path.insert(0, str(REPO / "tests" / "continuity"))
    from make_antigravity_fixture import build  # noqa: E402

    rep = Report("continuity: Antigravity chat history")
    sb = Sandbox("agyhist")
    sb.env.update({"AUTOMEMORY_SECRETS_BACKEND": "file", "AUTOMEMORY_KEY": "agy-master"})
    try:
        root = sb.project.resolve()
        sb.mcp("project.register", slug="app", display_name="app", path=str(root))
        base = sb.home / ".gemini" / "antigravity"
        build(base, str(root))
        # A conversation of another workspace must not be read for this project.
        other = sb.home / ".gemini" / "antigravity-ide"
        build(other, "/somewhere/else/entirely")
        for f in (other / "conversations").glob("*.db"):
            f.rename(other / "conversations" / "other-sess.db")

        out = sb.run("continuity", "sources").stdout
        rep.check("Antigravity (agy)  off" in out and "found: 1" in out.split("Antigravity (agy)")[1], "sources finds this project's Antigravity chat and not the other workspace's", out[-500:])
        rep.check(sb.run("continuity", "enable", "agy").returncode == 0, "a person turns Antigravity on")
        r = sb.run("continuity", "import", "agy")
        m = re.search(r"(\d+) new message", r.stdout)
        rep.check(r.returncode == 0 and m and int(m.group(1)) >= 6, "the conversation and its plan are imported", r.stdout + r.stderr)
        rep.check("0 new message" in sb.run("continuity", "import", "agy").stdout, "importing again adds nothing")

        hits = sb.run("continuity", "search", "sticky routing", "--agent", "agy").stdout
        rep.check("agy:agy-sess-001" in hits and "Plan the demo shop cart" in hits, "search finds the exchange, with the conversation's title", hits)
        plan = sb.run("continuity", "search", "ruled out server-side sessions").stdout
        rep.check("plan.md" in plan or "Ruled" in plan or "ruled" in plan, "the agent's own plan file is searchable too", plan)
        for secret_word in ("FIXTURESECRET", "PRIVATE REASONING", "TOOL OUTPUT"):
            rep.check(secret_word not in sb.run("continuity", "search", secret_word).stdout, f"{secret_word.lower()} was never stored")
        rep.check("→ read_file" not in sb.run("continuity", "search", "read_file").stdout, "one-line tool notes are not searched")

        # The switch: a new Claude Code session asks what was said.
        g = j(sb.mcp("chat.search", query="sticky routing", agent="agy", project_id="app"))
        rep.check(g["hits"] and g["hits"][0]["agent"] == "agy", "a Claude session finds the Antigravity discussion with chat.search", json.dumps(g)[:240])
        got = j(sb.mcp("chat.get", session=g["hits"][0]["session"], from_seq=0, limit=12))
        roles = [m["role"] for m in got["messages"]]
        rep.check("user" in roles and "assistant" in roles and "tool" in roles, "chat.get opens the exchange with user, assistant and tool notes in order", json.dumps(roles))
        brief_add = j(sb.mcp("continuity.brief", project_id="app", action="add", agent="claude-code", session_id="cl-1", items=[
            {"kind": "rejected", "text": "Server-side sessions for the cart", "reason": "no sticky routing (found in the Antigravity chat)"}]))
        rep.check(brief_add["changed"][0]["created"], "and records what it learned in the brief")
    finally:
        sb.cleanup()
    return rep.finish()


def codex():
    """Codex rollout files: found by the folder the session ran in (a sub-folder counts), read when turned on, searchable, no reasoning or tool output."""
    rep = Report("continuity chat history (Codex)")
    sb = Sandbox("codexhist")
    sb.env.update({"AUTOMEMORY_SECRETS_BACKEND": "file", "AUTOMEMORY_KEY": "hist-master"})
    try:
        root = sb.project.resolve()
        sb.mcp("project.register", slug="app", display_name="app", path=str(root))
        src = REPO / "tests/continuity/fixtures/codex/home/.codex"
        dst = sb.home / ".codex"
        shutil.copytree(src, dst)
        for f in dst.rglob("*.jsonl"):
            f.write_text(f.read_text().replace("/tmp/demo-shop", str(root)))
        out = sb.run("continuity", "sources").stdout
        rep.check("Codex (codex)  off" in out and "found: 1" in out.split("Codex (codex)")[1], "sources finds the Codex chat of this project (not the other project's)", out[-400:])
        r = sb.run("continuity", "import", "codex")
        rep.check(r.returncode != 0 and "is off" in (r.stdout + r.stderr), "nothing is read until a person turns Codex on")
        rep.check(sb.run("continuity", "enable", "codex").returncode == 0, "a person turns Codex on")
        r = sb.run("continuity", "import", "codex")
        m = re.search(r"(\d+) new message", r.stdout)
        rep.check(r.returncode == 0 and m and int(m.group(1)) >= 5, "the rollout is imported", r.stdout + r.stderr)
        rep.check("0 new message" in sb.run("continuity", "import", "codex").stdout, "importing again adds nothing")
        hits = sb.run("continuity", "search", "sticky routing", "--agent", "codex").stdout
        rep.check("codex:0199aaaa" in hits, "search finds the exchange", hits)
        for word in ("FIXTURESECRET", "PRIVATE REASONING", "TOOL OUTPUT", "environment_context"):
            rep.check(word not in sb.run("continuity", "search", word).stdout, f"{word.lower()} was never stored")
        show = sb.run("continuity", "show").stdout
        rep.check("shopping cart" in show and "sticky routing" in show, "the drafter turns it into goal and ruled-out drafts", show[-400:])
    finally:
        sb.cleanup()
    return rep.finish()


if __name__ == "__main__":
    sys.exit(max(main(), antigravity(), codex()))
