#!/usr/bin/env python3
"""Builds the anonymised Cursor fixture: synthetic state.vscdb files in the layout the reader follows (global cursorDiskKV with composerData:<id> and
bubbleId:<id>:<id> rows; a workspace folder with workspace.json and a composer.composerData index). Not a real Cursor file: written from public descriptions.
Run: python3 tests/continuity/make_cursor_fixture.py   (rewrites tests/continuity/fixtures/cursor/)"""
import json
import shutil
import sqlite3
from pathlib import Path

ROOT = Path(__file__).resolve().parent / "fixtures" / "cursor" / "home" / "Library" / "Application Support" / "Cursor" / "User"


def kv_db(path, tables=("ItemTable", "cursorDiskKV")):
    path.parent.mkdir(parents=True, exist_ok=True)
    if path.exists():
        path.unlink()
    c = sqlite3.connect(path)
    for t in tables:
        c.execute(f"CREATE TABLE {t} (key TEXT UNIQUE ON CONFLICT REPLACE, value BLOB)")
    return c


def main():
    shutil.rmtree(ROOT.parent.parent.parent.parent, ignore_errors=True)
    g = kv_db(ROOT / "globalStorage" / "state.vscdb")
    msgs = [
        (1, 1759309205000, "We want a shopping cart for the demo shop. Keep it plain Python, no web framework."),
        (2, 1759309220000, "I suggest storing carts in SQLite keyed by a cookie id. Server-side sessions would need sticky routing, so I rule them out."),
        (1, 1759309230000, "Use this key for the payment sandbox: sk-FIXTURESECRET1234567890abcdefghij and note /Users/someone/demo-shop/notes.txt"),
        (2, 1759309240000, "Understood. Next step: write the cart_items table with a 30 day cleanup."),
    ]

    def composer(cid, name, messages):
        headers = []
        for i, (t, ts, text) in enumerate(messages):
            bid = f"{cid}-b{i}"
            headers.append({"bubbleId": bid, "type": t})
            body = {"type": t, "text": text, "createdAt": ts, "bubbleId": bid}
            if t == 2 and i == 1:
                body["thinking"] = {"text": "PRIVATE REASONING that must never be stored"}
            g.execute("INSERT INTO cursorDiskKV VALUES (?, ?)", (f"bubbleId:{cid}:{bid}", json.dumps(body)))
            if t == 2 and i == 1:  # a tool bubble: no text, only what it ran
                tb = f"{cid}-tool"
                g.execute("INSERT INTO cursorDiskKV VALUES (?, ?)", (f"bubbleId:{cid}:{tb}", json.dumps({"type": 2, "text": "", "createdAt": ts + 1, "toolFormerData": {"name": "run_terminal_cmd", "rawArgs": json.dumps({"command": "cargo test"}), "result": "TOOL RESULT that must not be stored"}})))
                headers.append({"bubbleId": tb, "type": 2})
        g.execute("INSERT INTO cursorDiskKV VALUES (?, ?)", (f"composerData:{cid}", json.dumps({"composerId": cid, "name": name, "createdAt": messages[0][1], "fullConversationHeadersOnly": headers})))

    composer("cmp-001", "Plan the demo shop cart", msgs)
    composer("cmp-other", "Another project's chat", [(1, 1759400000000, "A chat from another project")])
    composer("cmp-orphan", "Not tied to any folder", [(1, 1759400100000, "A chat with no workspace")])
    g.commit()
    g.close()

    def workspace(wid, folder, composer_ids):
        d = ROOT / "workspaceStorage" / wid
        d.mkdir(parents=True, exist_ok=True)
        (d / "workspace.json").write_text(json.dumps({"folder": f"file://{folder}"}))
        w = kv_db(d / "state.vscdb", ("ItemTable",))
        w.execute("INSERT INTO ItemTable VALUES (?, ?)", ("composer.composerData", json.dumps({"allComposers": [{"composerId": c, "name": c, "createdAt": 1} for c in composer_ids], "selectedComposerIds": composer_ids[:1]})))
        w.commit()
        w.close()

    workspace("ws-demo-shop", "/tmp/demo-shop", ["cmp-001"])
    workspace("ws-other", "/tmp/other-project", ["cmp-other"])


if __name__ == "__main__":
    main()
