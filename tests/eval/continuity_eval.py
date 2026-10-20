#!/usr/bin/env python3
"""Continuity eval (C8): can a new agent answer the questions that matter after only reading the brief the drafter wrote?

Golden chats (written here, Claude Code format) have known answers: the goal, what was ruled out, what was decided, the next
step, what is still open. For each, the chat is imported (agent turned on) and drafted with no model; the score is how many
expected answers appear in the right section of the brief. Also checked: planted secrets and 'thinking' never reach the brief.
The score is recorded in tests/eval/results/continuity.json and must not fall below the floor in FLOOR.

Phrasings are deliberately varied, some the heuristics are not built to catch, so the number is honest rather than perfect.
Usage: python3 tests/eval/continuity_eval.py"""
import json
import re
import sys
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parents[1] / "lib"))
from harness import REPO, Report, Sandbox  # noqa: E402

FLOOR = 0.8

# Each chat: [(role, text)], expected {kind: [keyword that must appear in an item of that kind]}
GOLDEN = {
    "cart": (
        [("user", "We want a shopping cart for the demo shop. Keep it plain Python, no web framework."),
         ("assistant", "I suggest storing carts in SQLite keyed by a cookie id. Server-side sessions would need sticky routing, so I rule them out."),
         ("user", "Good. Let's go with SQLite for the cart table because we already ship it."),
         ("assistant", "Done with the schema. Next step: write the cart_items table with a 30 day cleanup."),
         ("user", "Should the cart survive a login?")],
        {"goal": ["shopping cart"], "constraint": ["no web framework"], "rejected": ["sticky routing"], "decision": ["SQLite"], "next": ["cart_items"], "open": ["survive a login"]},
    ),
    "auth": (
        [("user", "Add login to the admin panel. Never store passwords in plain text."),
         ("assistant", "I will use bcrypt instead of SHA-256 because SHA-256 is too fast to brute force."),
         ("user", "We decided to use a session cookie rather than JWT for the admin panel."),
         ("assistant", "Next step: add the login route and a rate limit of five tries a minute."),
         ("user", "ok")],
        {"goal": ["login"], "constraint": ["plain text"], "rejected": ["SHA-256", "JWT"], "decision": ["session cookie"], "next": ["login route"]},
    ),
    "migrate": (
        [("user", "Move the orders table from Postgres to SQLite for the offline build."),
         ("assistant", "Plan: export with a script, then import. We should not use the ORM migration tool, it drops indexes."),
         ("user", "Agreed. Let's use plain SQL files and keep them in db/migrations."),
         ("assistant", "Remaining: write the export script\nTODO: verify row counts after import"),
         ("user", "What about the archived orders from 2019?")],
        {"goal": ["orders table"], "rejected": ["ORM migration"], "decision": ["plain SQL"], "next": ["export script"], "open": ["archived orders"]},
    ),
    "plain": (  # little structure: a good drafter says little rather than guessing
        [("user", "Can you look at why the build is slow?"),
         ("assistant", "The build spends most of its time linking. I measured 40 seconds in the final link step."),
         ("user", "Thanks, that helps.")],
        {"goal": ["build is slow"]},
    ),
}
SECRET = "sk-EVALSECRET1234567890abcdefghijkl"


def write_chat(sb, name, turns):
    root = sb.project.resolve()
    d = sb.home / ".claude" / "projects" / re.sub(r"[^A-Za-z0-9]", "-", str(root))
    d.mkdir(parents=True, exist_ok=True)
    lines = []
    for i, (role, text) in enumerate(turns):
        content = text if role == "user" else [{"type": "thinking", "thinking": "PRIVATE EVAL REASONING"}, {"type": "text", "text": text}]
        lines.append({"type": role, "isSidechain": False, "sessionId": name, "timestamp": f"2026-10-01T09:{i:02d}:00.000Z", "cwd": str(root), "message": {"role": role, "content": content}})
    lines.insert(1, {"type": "user", "isSidechain": False, "sessionId": name, "timestamp": "2026-10-01T09:00:30.000Z", "message": {"role": "user", "content": f"my key is {SECRET}"}})
    (d / f"{name}.jsonl").write_text("\n".join(json.dumps(l) for l in lines) + "\n")


def main():
    rep = Report("continuity eval")
    rows, found, total, noise = [], 0, 0, 0
    for name, (turns, expect) in GOLDEN.items():
        sb = Sandbox(f"cev-{name}")
        sb.env.update({"AUTOMEMORY_SECRETS_BACKEND": "file", "AUTOMEMORY_KEY": "cev"})
        try:
            sb.mcp("project.register", slug="app", display_name="app", path=str(sb.project.resolve()))
            write_chat(sb, name, turns)
            sb.run("continuity", "enable", "claude")
            sb.run("continuity", "import")
            got = sb.mcp("continuity.brief", action="get")
            items = got.get("items", []) if isinstance(got, dict) else []
            blob = json.dumps(items)
            rep.check(SECRET not in blob and "PRIVATE EVAL" not in blob, f"{name}: no secret or reasoning reached the brief")
            matched = set()
            for kind, words in expect.items():
                for w in words:
                    total += 1
                    hit = next((i for i in items if i["kind"] == kind and w.lower() in i["text"].lower()), None)
                    if hit:
                        found += 1
                        matched.add(hit["id"])
                    rows.append((name, kind, w, bool(hit)))
            noise += len([i for i in items if i["id"] not in matched])
        finally:
            sb.cleanup()
    score = found / total
    for n, k, w, ok in rows:
        print(f"  {'hit ' if ok else 'MISS'}  {n:8} {k:10} {w}")
    print(f"  recall {found}/{total} = {score:.2f}; items not matching an expected answer: {noise}")
    out = REPO / "tests/eval/results"
    out.mkdir(parents=True, exist_ok=True)
    (out / "continuity.json").write_text(json.dumps({"recall": round(score, 3), "found": found, "total": total, "unmatched_items": noise, "misses": [list(r[:3]) for r in rows if not r[3]]}, indent=1))
    rep.check(score >= FLOOR, f"recall {score:.2f} is at least {FLOOR}")
    rep.finish()


if __name__ == "__main__":
    main()
