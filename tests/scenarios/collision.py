#!/usr/bin/env python3
"""Phase 1 gate: two or more agents, one repo, zero silent overwrites.

A. Agent A leases a file; agent B's edit of the same file through the real hook must reach the
   agent as hook output (a warning in the default Vibe mode, a deny in Strict), while edits of
   other files stay silent.
B. Eight processes race for one lease at once: exactly one is granted.
C. After A releases, B's edit is clean again.
"""
import sys
from concurrent.futures import ThreadPoolExecutor
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parents[1] / "lib"))
from harness import Report, Sandbox  # noqa: E402


def edit(sb, agent, session, path):
    payload = {"session_id": session, "tool_name": "Edit", "tool_input": {"file_path": path}}
    return sb.hook(agent, "pre-tool", payload)


def reported(parsed, raw, stderr):
    """True when the agent was told something: stdout carries a deny or context.
    stderr is not shown to the agent, so it does not count."""
    return bool(parsed and parsed != {})


def main():
    sb = Sandbox("collision")
    rep = Report("two-agent collision")
    try:
        # Register the sandbox project so leases resolve to it.
        sb.run("connect", "claude")  # workspace-scope; also registers hooks in the sandbox only

        granted = sb.mcp("lock.acquire", scope="src/auth.rs", goal="refactor auth",
                         agent="claude-code", session_id="sA")
        rep.check(granted.get("status") == "granted", "A acquires the lease", str(granted)[:160])

        parsed, raw, err = edit(sb, "claude", "sB", "src/auth.rs")
        rep.check(reported(parsed, raw, err), "B editing A's file is reported (not silent)",
                  f"{raw!r} {err!r}")

        parsed, raw, err = edit(sb, "claude", "sB", "src/other.rs")
        rep.check(not reported(parsed, raw, err), "B editing a different file stays silent",
                  f"{raw!r} {err!r}")

        parsed, raw, err = edit(sb, "claude", "sA", "src/auth.rs")
        rep.check(not reported(parsed, raw, err), "A editing its own leased file stays silent",
                  f"{raw!r} {err!r}")

        sb.mcp("lock.release", session_id="sA")
        parsed, raw, err = edit(sb, "claude", "sB", "src/auth.rs")
        rep.check(not reported(parsed, raw, err), "after release, B's edit is clean",
                  f"{raw!r} {err!r}")

        # Strict mode turns the warning into a block.
        sb.run("mode", "set", "strict")
        sb.mcp("lock.acquire", scope="src/strict.rs", goal="strict test", agent="claude-code",
               session_id="sA")
        parsed, raw, err = edit(sb, "claude", "sB", "src/strict.rs")
        rep.check('"deny"' in raw and "locked" in raw, "Strict mode: B's edit is denied", raw[:200])
        sb.run("mode", "set", "vibe")

        def grab(i):
            r = sb.mcp("lock.acquire", scope="src/race.rs", goal="race", agent=f"a{i}",
                       session_id=f"race-{i}")
            return r.get("status") == "granted"

        with ThreadPoolExecutor(max_workers=8) as ex:
            wins = sum(ex.map(grab, range(8)))
        rep.check(wins == 1, "8 parallel processes: exactly one lease granted", f"granted={wins}")
    finally:
        sb.cleanup()
    return rep.finish()


if __name__ == "__main__":
    sys.exit(main())
