#!/usr/bin/env python3
"""Builds the anonymised Antigravity fixture (a synthetic conversation database in the format the reader decodes).
Run: python3 tests/continuity/make_antigravity_fixture.py   (rewrites tests/continuity/fixtures/antigravity/)"""
import json
import shutil
import sqlite3
from pathlib import Path

ROOT = Path(__file__).resolve().parent / "fixtures" / "antigravity" / "home" / ".gemini" / "antigravity"
SID = "agy-sess-001"


def varint(v):
    out = bytearray()
    while True:
        b = v & 0x7F
        v >>= 7
        out.append(b | (0x80 if v else 0))
        if not v:
            return bytes(out)


def vf(num, v):
    return varint(num << 3) + varint(v)


def lf(num, payload):
    if isinstance(payload, str):
        payload = payload.encode()
    return varint(num << 3 | 2) + varint(len(payload)) + payload


def ts(secs):
    return lf(5, lf(1, vf(1, secs)))


def user(secs, text):
    return vf(1, 14) + vf(4, 3) + ts(secs) + lf(19, lf(2, text) + lf(3, lf(1, text)))


def planner(secs, text=None, thinking=None, calls=()):
    body = b""
    if text:
        body += lf(1, text)
    if thinking:
        body += lf(3, thinking)
    for cid, name, args in calls:
        body += lf(7, lf(1, cid) + lf(2, name) + lf(3, json.dumps(args)))
    return vf(1, 15) + vf(4, 3) + ts(secs) + lf(20, body)


def other(step_type, secs, text):
    return vf(1, step_type) + vf(4, 3) + ts(secs) + lf(30, lf(1, text))


def build(root, workspace):
    """Writes the synthetic conversation under `root` (a folder that stands in for ~/.gemini/antigravity)."""
    root = Path(root)
    for d in ("conversations", "annotations", f"brain/{SID}"):
        (root / d).mkdir(parents=True)
    db = sqlite3.connect(root / "conversations" / f"{SID}.db")
    db.executescript("""
        CREATE TABLE trajectory_metadata_blob (id text DEFAULT "main", data blob, PRIMARY KEY (id));
        CREATE TABLE steps (idx integer, step_type integer NOT NULL DEFAULT 0, status integer NOT NULL DEFAULT 0,
            has_subtrajectory numeric NOT NULL DEFAULT false, metadata blob, error_details blob, permissions blob,
            task_details blob, render_info blob, step_payload blob, step_format integer NOT NULL DEFAULT 0, PRIMARY KEY (idx));
    """)
    db.execute("INSERT INTO trajectory_metadata_blob (id, data) VALUES ('main', ?)", (lf(1, lf(1, "file://" + workspace) + lf(2, "file://" + workspace)),))
    t0 = 1790000000
    steps = [
        user(t0, "We want a shopping cart for the demo shop. Keep it plain Python, no web framework."),
        other(90, t0 + 1, "[AutoMemory] injected hook message that is not the conversation"),
        planner(t0 + 5, thinking="PRIVATE REASONING that must never be stored", calls=[("call_1", "read_file", {"Path": "/Users/someone/demo-shop/shop/cart.py"})]),
        other(132, t0 + 6, "TOOL OUTPUT that must not be stored"),
        planner(t0 + 20, text="I suggest SQLite keyed by a cookie id; server-side sessions would need sticky routing, so I rule them out."),
        user(t0 + 60, "Use this key for the payment sandbox: sk-FIXTURESECRET1234567890abcdefghij and note /Users/someone/demo-shop/notes.txt"),
        other(17, t0 + 61, "agent executor error: network issue"),
        planner(t0 + 90, text="Understood. Next step: write the cart_items table with a 30 day cleanup.", calls=[("call_2", "run_command", {"CommandLine": "python3 -m unittest"})]),
    ]
    for i, payload in enumerate(steps):
        step_type = {0: 14, 1: 90, 2: 15, 3: 132, 4: 15, 5: 14, 6: 17, 7: 15}[i]
        db.execute("INSERT INTO steps (idx, step_type, step_payload) VALUES (?, ?, ?)", (i, step_type, payload))
    db.commit()
    db.close()
    (root / "annotations" / f"{SID}.pbtxt").write_text('title:"Plan the demo shop cart" last_user_view_time:{seconds:1790000100 nanos:0}\n')
    (root / "brain" / SID / "plan.md").write_text("# Cart plan\n- SQLite cart_items keyed by cookie id\n- Ruled out: server-side sessions (no sticky routing)\n")
    (root / "brain" / SID / "plan.md.metadata.json").write_text('{"kind":"plan"}')
    return root


if __name__ == "__main__":
    out = ROOT
    shutil.rmtree(out.parent.parent, ignore_errors=True)
    build(out, "/tmp/demo-shop")
    print("wrote", out)
