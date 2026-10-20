#!/usr/bin/env python3
"""Garbage into the real hook process: it must always exit 0 and print one JSON object, never crash or hang,
and a pathologically nested command must be blocked rather than slip through. Seeded; HOOK_FUZZ_N=2000 runs harder."""
import json
import os
import random
import subprocess
import sys
import time
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parents[1] / "lib"))
from harness import Report, Sandbox  # noqa: E402

AGENTS = ["claude", "agy", "codex", "cursor", "windsurf", "gemini-cli", "copilot"]
EVENTS = ["boot", "pre-tool", "post-tool", "post-tool-fail", "prompt", "stop", "pre-commit", "task-done", "nonsense"]
BITS = ['{', '}', '[', ']', '"', "'", ':', ',', '\\', '\0', '\n', 'null', 'true', '1e999', '-0', '"tool_name"', '"tool_input"', '"command"',
        '"session_id"', '"file_path"', '$(', '`', '‮', '😀', '{"a":' * 50, '[' * 300, '9' * 400, '"' + 'x' * 5000 + '"']
SEEDS = [
    {"session_id": "s", "tool_name": "Bash", "tool_input": {"command": "rm -rf /"}},
    {"session_id": "s", "hook_event_name": "PreToolUse", "tool_name": "Write", "tool_input": {"file_path": "a.rs", "content": "x"}},
    {"session_id": "s", "tool_name": "Edit", "tool_input": {"file_path": "../../etc/passwd"}},
    {"hook_event_name": "UserPromptSubmit", "prompt": "hi"},
    {"session_id": "s", "tool_name": "Bash", "tool_input": {"command": "ls"}, "tool_response": {"stdout": "a", "stderr": 5}},
]


def mutate(rng, text):
    s = list(text)
    for _ in range(rng.randint(1, 6)):
        at = rng.randint(0, len(s))
        op = rng.randint(0, 3)
        if op == 0 and s:
            del s[min(at, len(s) - 1)]
        elif op == 1:
            s[at:at] = list(rng.choice(BITS))
        elif op == 2 and len(s) > 4:
            a = rng.randint(0, len(s) - 2)
            s = s[:a]
        else:
            s[at:at] = list(rng.choice(["null", "[]", "{}", '"', "0"]))
    return "".join(s)


def main():
    rep = Report("hook fuzz")
    sb = Sandbox("hookfuzz")
    rng = random.Random(int(os.environ.get("HOOK_FUZZ_SEED", "7")))
    n = int(os.environ.get("HOOK_FUZZ_N", "250"))
    bad, slowest = [], 0.0
    try:
        for i in range(n):
            agent, event = rng.choice(AGENTS), rng.choice(EVENTS)
            payload = mutate(rng, json.dumps(rng.choice(SEEDS))) if rng.random() < 0.9 else os.urandom(rng.randint(0, 300)).decode("latin-1")
            t = time.time()
            try:
                r = subprocess.run([sb.bin, "hook", "--agent", agent, "--event", event, "--project", "auto"], input=payload.encode("utf-8", "surrogatepass"),
                                   capture_output=True, cwd=sb.project, env=sb.env, timeout=20)
            except subprocess.TimeoutExpired:
                bad.append(f"HANG {agent}/{event} {payload[:80]!r}")
                continue
            slowest = max(slowest, time.time() - t)
            out = r.stdout.decode("utf-8", "replace").strip()
            try:
                ok = r.returncode == 0 and isinstance(json.loads(out or "{}"), dict)
            except json.JSONDecodeError:
                ok = False
            if not ok:
                bad.append(f"exit={r.returncode} {agent}/{event} stdout={out[:60]!r} stderr={r.stderr.decode('utf-8', 'replace')[-80:]!r} in={payload[:80]!r}")
        print(f"  {n} inputs, slowest {slowest * 1000:.0f} ms")
        rep.check(not bad, "every garbage input gets exit 0 and one JSON object", "\n".join(bad[:5]))

        deep = {"session_id": "s", "tool_name": "Bash", "tool_input": {"command": "$(" * 60000 + "rm -rf /tmp/x"}}
        parsed, raw, err = sb.hook("claude", "pre-tool", deep)
        rep.check(isinstance(parsed, dict) and parsed.get("hookSpecificOutput", {}).get("permissionDecision") == "deny",
                  "a command nested 60000 deep is denied, not waved through", f"{raw[:120]!r} {err[-120:]!r}")
    finally:
        sb.cleanup()
    return rep.finish()


if __name__ == "__main__":
    sys.exit(main())
