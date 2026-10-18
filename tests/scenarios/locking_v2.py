#!/usr/bin/env python3
"""Automatic locking beyond edit tools (PLAN §6): lifecycle, shell prediction,
reconciliation, visibility, ignore policy and the coverage report.

Uses a real git repository (the sandbox project) so reconciliation sees real changes.
"""
import subprocess
import sys
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parents[1] / "lib"))
from harness import Report, Sandbox  # noqa: E402


def pre(sb, agent, session, tool, **tool_input):
    return sb.hook(agent, "pre-tool", {"session_id": session, "tool_name": tool, "tool_input": tool_input})


def post(sb, agent, session, tool, **tool_input):
    return sb.hook(agent, "post-tool", {"session_id": session, "tool_name": tool, "tool_input": tool_input,
                                        "tool_response": {}})


def held(sb):
    return [l.get("scope", "") for l in sb.mcp("lock.status").get("active_leases", [])]


def told(parsed):
    return bool(parsed and parsed != {})


def main():
    sb = Sandbox("locking-v2")
    rep = Report("automatic locking v2")
    try:
        # 1. Lifecycle: a turn ending does not drop the lease; the session ending does.
        pre(sb, "claude", "A", "Edit", file_path="src/life.rs")
        sb.hook("claude", "stop", {"session_id": "A", "stop_hook_active": False})
        rep.check(any(s.endswith("life.rs") for s in held(sb)), "lease survives Stop (end of a turn)")
        p, raw, _ = pre(sb, "codex", "B", "apply_patch", input="*** Begin Patch\n*** Update File: src/life.rs\n*** End Patch")
        rep.check(told(p), "other agent still warned between A's turns", raw[:160])
        sb.hook("claude", "session-end", {"session_id": "A"})
        rep.check(not any(s.endswith("life.rs") for s in held(sb)), "SessionEnd releases the lease")

        # 2. Shell prediction: sed -i / redirects lease their targets before running.
        p, raw, _ = pre(sb, "claude", "C", "Bash", command="sed -i 's/a/b/' src/pred.rs && echo done > notes/out.txt")
        rep.check(not told(p), "predicted shell write is silent for its own session", raw[:160])
        scopes = held(sb)
        rep.check(any(s.endswith("pred.rs") for s in scopes) and any(s.endswith("out.txt") for s in scopes),
                  "sed -i target and redirect target are leased before the command runs", str(scopes))
        p, raw, _ = pre(sb, "codex", "D", "Bash", command="echo x >> src/pred.rs")
        rep.check(told(p), "another agent's shell write to the same file is reported", raw[:160])
        sb.hook("claude", "session-end", {"session_id": "C"})
        sb.hook("codex", "session-end", {"session_id": "D"})

        # 3. Reconciliation: a script writes files nobody could predict.
        pre(sb, "claude", "E", "Bash", command="python3 gen.py")
        (sb.project / "gen").mkdir(exist_ok=True)
        (sb.project / "gen" / "made.rs").write_text("generated")
        p, raw, _ = post(sb, "claude", "E", "Bash", command="python3 gen.py")
        rep.check(any(s.endswith("gen/made.rs") for s in held(sb)),
                  "file written by an unpredictable script is leased afterwards", str(held(sb)))

        # F holds a file; E's next script changes it.
        pre(sb, "codex", "F", "apply_patch", input="*** Begin Patch\n*** Update File: shared.rs\n*** End Patch")
        pre(sb, "claude", "E", "Bash", command="python3 gen2.py")
        (sb.project / "shared.rs").write_text("changed by E's script")
        p, raw, _ = post(sb, "claude", "E", "Bash", command="python3 gen2.py")
        rep.check("shared.rs" in raw and "codex" in raw and "leased" in raw,
                  "script touching another agent's file is reported to the agent", raw[:240])

        # 4. Visibility: a new prompt lists what others hold.
        p, raw, _ = sb.hook("claude", "prompt", {"session_id": "G", "prompt": "hello"})
        rep.check("shared.rs" in raw and "editing right now" in raw,
                  "prompt hook lists files other agents hold", raw[:240])
        p, raw, _ = sb.hook("codex", "prompt", {"session_id": "F", "prompt": "hello"})
        rep.check("shared.rs" not in raw, "an agent is not told about its own files", raw[:240])

        # 4b. Antigravity: its hooks accept a fixed schema (antigravity.google/docs/hooks).
        def agy(event, **payload):
            return sb.hook("agy", event, {"conversationId": "agyB", **payload})

        pre(sb, "claude", "A2", "Edit", file_path="src/agy.rs")  # Claude holds src/agy.rs
        p, raw, _ = agy("boot", invocationNum=2)
        rep.check("injectSteps" in (p or {}) and "src/agy.rs" in raw,
                  "agy PreInvocation injects the files other agents hold", raw[:240])
        rep.check(set(p or {}) == {"injectSteps"}, "agy PreInvocation output has only injectSteps", raw[:240])
        tool = {"toolCall": {"name": "run_command", "args": {"CommandLine": "echo x >> src/agy.rs"}}}
        agy("pre-tool", **tool)
        (sb.project / "src").mkdir(exist_ok=True)
        (sb.project / "src" / "agy.rs").write_text("changed by agy's command")
        p, raw, _ = agy("post-tool", **tool)
        rep.check(raw.strip() == "{}", "agy PostToolUse prints exactly {} (any other field is an error)", raw[:200])
        p, raw, _ = agy("boot", invocationNum=3)
        rep.check("Lock warning" in raw and "claude" in raw,
                  "the collision found afterwards reaches agy on its next PreInvocation", raw[:300])
        p, raw, _ = agy("boot", invocationNum=4)
        rep.check("Lock warning" not in raw, "a note is delivered once", raw[:300])
        p, raw, _ = agy("stop", executionNum=1, terminationReason="model_stop", fullyIdle=True)
        rep.check(p == {"decision": "allow"}, "agy Stop answers with a decision", raw[:200])

        # 5. Ignore policy: build output and lockfiles never lock.
        for path in ("target/debug/out.o", "node_modules/x/index.js", "Cargo.lock"):
            pre(sb, "claude", "H", "Write", file_path=path)
        rep.check(not any(s.startswith(("target", "node_modules")) or s.endswith("Cargo.lock") for s in held(sb)),
                  "ignored paths take no lease", str(held(sb)))

        # 6. Coverage report reflects what happened.
        out = sb.run("locks", "--coverage").stdout
        rep.check("covered or reported" in out and "leased before the write" in out, "locks --coverage prints a report", out[-300:])
        rep.check("found after the command" in out and "(0%)" not in out.split("found after the command")[1].splitlines()[0],
                  "reconciled writes are counted", out[-300:])
    finally:
        sb.cleanup()
    return rep.finish()


if __name__ == "__main__":
    sys.exit(main())
