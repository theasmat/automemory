#!/usr/bin/env python3
"""A Claude Code session end to end through the hooks: edits, a commit, then stop.

Checks that Stop leaves a progress checkpoint for sessions that
edited files (and none for read-only ones).
"""
import sys
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parents[1] / "lib"))
from harness import Report, Sandbox  # noqa: E402


def tool(sb, session, name, **tool_input):
    sb.hook("claude", "post-tool", {"session_id": session, "tool_name": name, "tool_input": tool_input,
                                    "tool_response": {}})


def main():
    sb = Sandbox("lifecycle")
    rep = Report("session lifecycle")
    try:
        for i in range(3):
            tool(sb, "edit-session", "Edit", file_path=f"src/mod{i}.rs")
        tool(sb, "edit-session", "Bash", command="cargo test")
        sb.hook("claude", "stop", {"session_id": "edit-session", "stop_hook_active": False})
        # Stop fires after every agent turn: more edits, another Stop.
        for i in range(3, 6):
            tool(sb, "edit-session", "Edit", file_path=f"src/mod{i}.rs")
        sb.hook("claude", "stop", {"session_id": "edit-session", "stop_hook_active": False})

        tool(sb, "read-session", "Read", file_path="README.md")
        sb.hook("claude", "stop", {"session_id": "read-session", "stop_hook_active": False})

        found = sb.mcp("memory.search", query="Session checkpoint")
        text = str(found)
        rep.check("Session checkpoint" in text, "editing session leaves a checkpoint memory", text[:200])
        rep.check("Edited 6 file(s)" in text and "src/mod0.rs" in text, "checkpoint states which files were edited", text[:200])
        rep.check(text.count("Session checkpoint edit-ses") == 1,
                  "two Stops of one session leave exactly one checkpoint", text[:300])
        rep.check("Session checkpoint read-ses" not in text, "read-only session leaves no checkpoint", text[:200])
    finally:
        sb.cleanup()
    return rep.finish()


if __name__ == "__main__":
    sys.exit(main())
