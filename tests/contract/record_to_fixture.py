#!/usr/bin/env python3
"""Turns payloads saved by `automemory hooks record on` into a contract fixture file.

Usage: record_to_fixture.py <agent> [recorded.jsonl] [--style native|flat] [--out file]
Each distinct payload (session ids ignored) is replayed through the current binary in a clean sandbox and the observed result
becomes the case's `expect` (a golden test: when the agent changes its payload format or we change a decision, the
case fails). READ THE OUTPUT before committing: a fixture records what happens today, it does not judge it.
Writes tests/contract/fixtures/<agent>/recorded.json. Only agents known to run.py (AGENT_ID) can be added there.
"""
import json
import sys
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parents[1] / "lib"))
sys.path.insert(0, str(Path(__file__).parent))
from harness import Sandbox  # noqa: E402
from run import FIXTURES, kind_of, text_of  # noqa: E402


def main(argv):
    style = "native"
    if "--style" in argv:
        i = argv.index("--style")
        style = argv[i + 1]
        del argv[i:i + 2]
    out_arg = None
    if "--out" in argv:
        i = argv.index("--out")
        out_arg = Path(argv[i + 1])
        del argv[i:i + 2]
    agent = argv[0]
    src = Path(argv[1]) if len(argv) > 1 else Path.home() / ".automemory/recorded-hooks" / f"{agent}.jsonl"
    rows = [json.loads(l) for l in src.read_text().splitlines() if l.strip()]
    sb = Sandbox("record")
    cases, seen = [], set()
    try:
        for r in rows:
            p = r["payload"]
            if not isinstance(p, dict):
                continue
            # Same call from another session is the same case; different commands are different cases.
            key = (r["event"], p.get("tool_name") or p.get("tool"), json.dumps({k: v for k, v in p.items() if k != "session_id"}, sort_keys=True))
            if key in seen:
                continue
            seen.add(key)
            parsed, raw, _ = sb.hook(agent, r["event"], p)
            case_style = style
            if kind_of(parsed, "native") == "other":  # the agent speaks the flat {"decision": ...} dialect
                case_style = "flat"
            kind = kind_of(parsed, case_style)
            exp = {"kind": kind}
            if kind in ("deny", "context") and text_of(parsed, case_style):
                exp["contains"] = text_of(parsed, case_style)[:40]
            cases.append({"name": f"recorded {r['event']} {key[1] or ''} #{len(cases) + 1}".replace("  ", " "), "event": r["event"], "style": case_style,
                          "payload": p, "expect": exp})
    finally:
        sb.cleanup()
    out = out_arg or FIXTURES / agent / "recorded.json"
    out.parent.mkdir(parents=True, exist_ok=True)
    out.write_text(json.dumps(cases, indent=1) + "\n")
    print(f"wrote {len(cases)} case(s) to {out}; add '{agent}' to AGENT_ID in tests/contract/run.py if it is new")


if __name__ == "__main__":
    main(sys.argv[1:])
