#!/usr/bin/env python3
"""Continuity C1 end to end: an agent leaves a brief (goal, ruled-out options with reasons, open questions, next steps) through
the MCP tool; a different agent's hook hands it to a new session with provenance; no duplicates; secrets removed; a person's
words are protected from agents; the CLI is behind the password; the token budget trims and points at the rest."""
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
    rep = Report("continuity brief")
    sb = Sandbox("continuity")
    sb.env.update({"AUTOMEMORY_SECRETS_BACKEND": "file", "AUTOMEMORY_KEY": "cont-master"})
    try:
        sb.mcp("project.register", slug="app", display_name="app", path=str(sb.project))
        sb.mcp("decision.record", topic="storage", decision="SQLite", reason="simple", project_id="app")
        brief = lambda **a: j(sb.mcp("continuity.brief", project_id="app", **a))

        # Antigravity works for hours and leaves a brief before stopping.
        r = brief(action="add", agent="antigravity", session_id="ag-1", items=[
            {"kind": "goal", "text": "Ship the shop cart with order cancellation"},
            {"kind": "rejected", "text": "Server-side sessions for the cart", "reason": "the load balancer has no sticky routing"},
            {"kind": "open", "text": "Do partial refunds restore stock per item?"},
            {"kind": "next", "text": "Write the cart_items migration"},
        ])
        rep.check(len(r.get("changed", [])) == 4 and all(c["created"] for c in r["changed"]), "an agent records goal, ruled-out option, open question and next step", json.dumps(r)[:200])
        again = brief(action="add", agent="antigravity", session_id="ag-1", items=[{"kind": "next", "text": "write the cart_items migration."}])
        rep.check(again["changed"][0]["created"] is False and len([i for i in again["items"] if i["kind"] == "next"]) == 1, "saying it again updates the item instead of duplicating it", json.dumps(again["changed"]))
        leaked = brief(action="add", agent="antigravity", session_id="ag-1", items=[{"kind": "next", "text": "Use key sk-abcdefghijklmnopqrstuvwxyz1234 for the payment API"}])
        rep.check("abcdefghijklmnopqrstuvwxyz" not in json.dumps(leaked), "a secret in an item is removed before it is stored", json.dumps(leaked)[:200])
        bad = brief(action="add", agent="antigravity", items=[{"kind": "nonsense", "text": "x"}])
        rep.check("unknown kind" in json.dumps(bad), "an unknown kind is refused with the valid ones listed", json.dumps(bad)[:160])

        # A different agent starts a new session and is handed the brief by its hook.
        claude = json.dumps(sb.hook("claude", "prompt", {"session_id": "cl-9", "hook_event_name": "UserPromptSubmit", "prompt": "hi", "cwd": str(sb.project)})[0] or {})
        for needle in ("Brief (what the previous sessions worked out", "Ship the shop cart", "Server-side sessions for the cart because the load balancer has no sticky routing", "(antigravity,", "Do partial refunds restore stock"):
            rep.check(needle in claude, f"Claude's first prompt carries: {needle[:48]}", claude[:300])
        agy = sb.hook("agy", "boot", {"conversationId": "ag-2", "session_id": "ag-2", "invocationNum": 0, "cwd": str(sb.project)})[1]
        rep.check("Ruled out (do not redo)" in agy, "and a new Antigravity session gets it through its injection too", agy[:200])
        rep.check("storage: SQLite" in claude, "next to the decisions in force")

        # Ownership: a person's words are theirs.
        subprocess.run([sb.bin, "continuity", "add", "constraint", "No new paid services", "--reason", "budget"], cwd=sb.project, env=sb.env, capture_output=True)
        items = brief(action="get")["items"]
        human = next(i for i in items if i["kind"] == "constraint")
        rep.check(human["origin"] == "human" and human["by"] == "human", "an item you add from the terminal is marked as yours", json.dumps(human))
        r = brief(action="correct", agent="claude-code", id=human["id"], text="Paid services are fine")
        rep.check("person" in json.dumps(r) and "cannot" in json.dumps(r), "an agent cannot change an item a person wrote", json.dumps(r)[:160])
        r = brief(action="remove", agent="claude-code", id=human["id"])
        rep.check("cannot" in json.dumps(r), "nor remove it")
        open_q = next(i for i in items if i["kind"] == "open")
        r = brief(action="resolve", agent="claude-code", id=open_q["id"])
        rep.check(not any(i["id"] == open_q["id"] for i in r["items"]), "but any agent can resolve an open question", json.dumps(r["changed"]))
        next_item = next(i for i in items if i["kind"] == "next" and "migration" in i["text"])
        r = brief(action="correct", agent="claude-code", id=next_item["id"], text="Write the cart_items migration with a rollback")
        rep.check(any(i["text"].endswith("with a rollback") and i["by"] == "claude-code" for i in r["items"]), "an agent can correct another agent's item", json.dumps(r["changed"]))

        # The CLI: password-gated once one exists.
        env = {**sb.env, "AUTOMEMORY_PASSWORD": "correct horse"}
        subprocess.run([sb.bin, "rule", "passwd", "--from-env"], env=env, cwd=sb.project, capture_output=True)
        denied = sb.run("continuity", "add", "next", "Sneaky step")
        rep.check(denied.returncode != 0, "with a password set, adding from a terminal without it is refused", (denied.stdout + denied.stderr)[-120:])
        ok = subprocess.run([sb.bin, "continuity", "add", "next", "Review the migration"], cwd=sb.project, env=env, capture_output=True, text=True)
        rep.check(ok.returncode == 0, "and works with it", ok.stdout + ok.stderr)
        shown = sb.run("continuity", "show").stdout
        rep.check("Ruled out" in shown and "Items (id, kind, state, status)" in shown, "show prints the brief and the item ids", shown[:200])
        rep.check("Brief (what the previous sessions" in sb.run("resume").stdout, "automemory resume includes the brief")

        # The budget trims and points at the rest.
        for i in range(40):
            brief(action="add", agent="claude-code", items=[{"kind": "next", "text": f"A reasonably long follow-up step number {i} with detail attached to it"}])
        cfg = sb.home / ".automemory" / "config.toml"
        original = cfg.read_text()
        cfg.write_text(original.replace("inject_tokens = 3000", "inject_tokens = 250"))
        small = json.dumps(sb.hook("claude", "prompt", {"session_id": "cl-10", "hook_event_name": "UserPromptSubmit", "prompt": "hi", "cwd": str(sb.project)})[0] or {})
        shown_steps = small.count("A reasonably long follow-up step")
        rep.check("more item(s): call continuity.brief" in small and 0 < shown_steps < 20, "a small inject_tokens trims the brief and points at the rest", f"{shown_steps} steps shown, {len(small)} chars")

        # A config.toml that no longer parses must lock the gate, not open it (an appended duplicate table once did).
        cfg.write_text(original + "\n[continuity]\ninject_tokens = 250\n")
        broken = sb.run("continuity", "add", "next", "Slipped past a broken config")
        rep.check(broken.returncode != 0 and "cannot be read" in (broken.stdout + broken.stderr), "a broken config.toml refuses human-only actions instead of allowing them", (broken.stdout + broken.stderr)[-200:])
        rep.check("Checking config.toml... ✗" in sb.run("doctor").stdout, "and doctor says so")
        cfg.write_text(original)

        # Forget.
        rep.check(sb.run("continuity", "forget").returncode != 0, "forget needs the password")
        r = subprocess.run([sb.bin, "continuity", "forget"], cwd=sb.project, env=env, capture_output=True, text=True)
        rep.check(r.returncode == 0 and brief(action="get")["items"] == [], "forget with it empties the brief", r.stdout + r.stderr)

        # Rules text tells agents about it.
        sb.run("connect", "claude")
        sb.run("connect", "agy")
        md = (sb.project / "CLAUDE.md").read_text()
        rules = (sb.project / ".agents/rules/automemory.md").read_text() if (sb.project / ".agents/rules/automemory.md").exists() else ""
        rep.check("continuity.brief" in md and "ruled out" in md.lower(), "CLAUDE.md tells agents to leave and read the brief")
        rep.check("continuity.brief" in rules, "and so do Antigravity's rules")
    finally:
        sb.cleanup()
    return rep.finish()


if __name__ == "__main__":
    sys.exit(main())
