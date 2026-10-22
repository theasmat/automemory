#!/usr/bin/env python3
"""Y8/Y9: the session checkpoint says what happened (files, next step) instead of activity statistics, and notes nobody used are offered for archiving at session start."""
import json
import sqlite3
import sys
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parents[1] / "lib"))
from harness import Report, Sandbox  # noqa: E402


def main():
    rep = Report("session checkpoint and unused notes")
    sb = Sandbox("ckpt")
    sb.env.update({"AUTOMEMORY_SECRETS_BACKEND": "file", "AUTOMEMORY_KEY": "ckpt-master"})
    try:
        root = sb.project.resolve()
        sb.mcp("project.register", slug="app", display_name="app", path=str(root))
        sb.mcp("continuity.brief", action="add", project_id="app", agent="claude", session_id="s1", items=[{"kind": "next", "text": "Wire the cart page to the new route"}])
        for f in ("shop/cart.py", "shop/db.py", "tests/test_cart.py"):
            sb.hook("claude", "post-tool", {"session_id": "sess-ckpt-1", "tool_name": "Edit", "tool_input": {"file_path": str(root / f)}, "tool_response": {}, "cwd": str(root)})
        sb.hook("claude", "stop", {"session_id": "sess-ckpt-1", "cwd": str(root)})
        db = sqlite3.connect(sb.home / ".automemory" / "automemory.db")
        row = db.execute("select content from memories where summary like 'Session checkpoint%'").fetchone()
        text = row[0] if row else ""
        rep.check(bool(row) and "Edited 3 file(s):" in text and "shop/cart.py" in text, "the checkpoint names the files edited (relative paths)", text[:300])
        rep.check("Next: Wire the cart page to the new route" in text, "and the next step from the brief", text[:300])
        rep.check("Activity Window Digest" not in text and "0 modified files" not in text, "and no longer the activity statistics")

        # Unused agent notes: offered for archiving once, at the start of a session.
        old = "2026-06-01T00:00:00+00:00"
        for i in range(2):
            db.execute("insert into memories (id, scope, project_id, memory_type, content, summary, confidence, importance, freshness, status, source_kind, created_at, updated_at, last_recalled_at, tags, entities, metadata) values (?, 'global', NULL, 'fact', ?, ?, 0.8, 0.5, 1.0, 'active', 'agent', ?, ?, NULL, '[]', '[]', '{}')",
                       (f"old-{i}", f"old note {i} body", f"Old unused note {i}", old, old))
        db.commit()
        db.close()
        out = json.dumps(sb.hook("claude", "prompt", {"session_id": "sess-new", "hook_event_name": "UserPromptSubmit", "prompt": "hi", "cwd": str(root)})[0] or {})
        db = sqlite3.connect(sb.home / ".automemory" / "automemory.db")
        offers = db.execute("select description from review_proposals where proposal_type = 'stale_note' and status = 'pending'").fetchall()
        rep.check(len(offers) == 2 and "Nobody has used this note" in offers[0][0], "unused notes are in the Inbox as 'archive this?'", str(offers))
        sb.hook("claude", "prompt", {"session_id": "sess-newer", "hook_event_name": "UserPromptSubmit", "prompt": "hi", "cwd": str(root)})
        n = db.execute("select count(*) from review_proposals where proposal_type = 'stale_note'").fetchone()[0]
        rep.check(n == 2, "and each is offered once, not at every session")
    finally:
        sb.cleanup()
    rep.finish()


if __name__ == "__main__":
    main()
