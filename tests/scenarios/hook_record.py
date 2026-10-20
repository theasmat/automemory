#!/usr/bin/env python3
"""`hooks record`: off by default, saves redacted payloads while on, caps nothing silently, and the recorded payloads
turn into a contract fixture that the contract runner accepts."""
import json
import subprocess
import sys
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parents[1] / "lib"))
from harness import Report, Sandbox  # noqa: E402

REPO = Path(__file__).resolve().parents[2]
SECRET = "AKIAIOSFODNN7EXAMPLE"


def main():
    rep = Report("hook recording")
    s = Sandbox("record")
    saved = s.home / ".automemory" / "recorded-hooks" / "cursor.jsonl"
    try:
        s.hook("cursor", "pre-tool", {"session_id": "r1", "tool_name": "Bash", "tool_input": {"command": "ls"}})
        rep.check(not saved.exists(), "nothing is recorded by default")
        rep.check("OFF" not in s.run("hooks", "record", "status").stdout and "off" in s.run("hooks", "record", "status").stdout, "status says off")
        s.run("hooks", "record", "on")
        s.hook("cursor", "pre-tool", {"session_id": "r1", "tool_name": "Bash", "tool_input": {"command": "ls"}})
        s.hook("cursor", "pre-tool", {"session_id": "r2", "tool_name": "Bash", "tool_input": {"command": f"export AWS_KEY={SECRET} && rm -rf /"}})
        s.hook("cursor", "post-tool", {"session_id": "r2", "tool_name": "Bash", "tool_input": {"command": "ls"}, "tool_response": {"stdout": "a"}})
        text = saved.read_text() if saved.exists() else ""
        rep.check(len(text.splitlines()) == 3, "three payloads were saved", text[:200])
        rep.check(SECRET not in text, "secrets are removed before saving", text[:300])
        s.run("hooks", "record", "off")
        s.hook("cursor", "pre-tool", {"session_id": "r3", "tool_name": "Bash", "tool_input": {"command": "pwd"}})
        rep.check(len(saved.read_text().splitlines()) == 3, "recording stops when switched off")

        out = s.root / "recorded.json"
        r = subprocess.run([sys.executable, str(REPO / "tests/contract/record_to_fixture.py"), "cursor", str(saved), "--out", str(out)],
                           capture_output=True, text=True)
        cases = json.loads(out.read_text()) if out.exists() else []
        rep.check(r.returncode == 0 and len(cases) == 3, "the converter writes one case per distinct payload", r.stdout + r.stderr)
        kinds = {c["payload"].get("tool_input", {}).get("command", "")[:6]: c["expect"]["kind"] for c in cases if c["event"] == "pre-tool"}
        rep.check(kinds.get("ls") == "allow" and "deny" in kinds.values(), "a harmless call is allowed and the dangerous one denied", str(kinds))
    finally:
        s.cleanup()
    return rep.finish()


if __name__ == "__main__":
    sys.exit(main())
