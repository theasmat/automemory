#!/usr/bin/env python3
"""Contract tests: recorded agent payloads in, expected hook output out.

Each fixtures/<agent>/*.json holds cases:
  {"name", "event", "payload", "style": "native"|"flat" (default native), "expect": {...}}
  expect.kind: allow   -> stdout is {} (native) or {"decision":"allow"} / {} (flat)
               deny    -> native: hookSpecificOutput.permissionDecision == "deny"; flat: decision == "deny"
               context -> native: hookSpecificOutput.additionalContext present (an allow with a message)
  expect.contains: substring that must appear in the reason/context text.

When an agent changes its hook format, add the new recorded payload as a new case (or a new
fixtures/<agent>/<version>.json file) and fix the adapter until every file passes.
Usage: run.py [agent ...]
"""
import json
import sys
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parents[1] / "lib"))
from harness import Report, Sandbox  # noqa: E402

FIXTURES = Path(__file__).parent / "fixtures"
AGENT_ID = {"claude": "claude", "codex": "codex", "agy": "agy"}


def text_of(out, style):
    if not isinstance(out, dict):
        return ""
    if style == "native":
        h = out.get("hookSpecificOutput", {})
        return h.get("permissionDecisionReason") or h.get("additionalContext") or ""
    return out.get("reason", "")


def kind_of(out, style):
    if not isinstance(out, dict) or out == {}:
        return "allow"
    if style == "native":
        h = out.get("hookSpecificOutput", {})
        if h.get("permissionDecision") == "deny":
            return "deny"
        return "context" if h.get("additionalContext") else "other"
    d = out.get("decision")
    return {"allow": "allow", "deny": "deny", "warn": "context", "ask": "context"}.get(d, "other")


def main(agents):
    sb = Sandbox("contract")
    rep = Report("agent hook contract")
    try:
        for agent in agents or sorted(p.name for p in FIXTURES.iterdir() if p.is_dir()):
            for f in sorted((FIXTURES / agent).glob("*.json")):
                for case in json.loads(f.read_text()):
                    style = case.get("style", "native")
                    if "setup" in case:
                        sb.run(*case["setup"])
                    parsed, raw, err = sb.hook(AGENT_ID[agent], case["event"], case["payload"])
                    exp = case["expect"]
                    got = kind_of(parsed, style)
                    text = text_of(parsed, style)
                    contains_ok = exp.get("contains", "") in text
                    absent_ok = "absent" not in exp or exp["absent"] not in text
                    ok = got == exp["kind"] and contains_ok and absent_ok
                    rep.check(ok, f"[{agent}] {case['name']}",
                              f"want {exp}, got kind={got} stdout={raw[:160]!r}")
    finally:
        sb.cleanup()
    return rep.finish()


if __name__ == "__main__":
    sys.exit(main(sys.argv[1:]))
