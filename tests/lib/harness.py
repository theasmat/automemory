"""Shared helpers for the script tests. Python 3 standard library only.

Every test runs against an isolated HOME (so ~/.automemory, ~/.claude, ~/.codex of the
real user are never touched) and a throwaway git repository as the project.
"""
from __future__ import annotations

import json
import os
import shutil
import subprocess
import sys
import tempfile
import time
from pathlib import Path

REPO = Path(__file__).resolve().parents[2]


def find_binary() -> str:
    """AUTOMEMORY_BIN, then the repo's release build, then debug, then PATH."""
    exe = ".exe" if os.name == "nt" else ""
    candidates = [os.environ.get("AUTOMEMORY_BIN"),
                  REPO / f"target/release/automemory{exe}",
                  REPO / f"target/debug/automemory{exe}",
                  shutil.which("automemory")]
    for c in candidates:
        if c and Path(c).is_file():
            return str(c)
    sys.exit("automemory binary not found. Run `cargo build --release` or set AUTOMEMORY_BIN.")


class Sandbox:
    """A temp HOME plus a git repo to act as the project."""

    def __init__(self, name: str = "t"):
        self.bin = find_binary()
        self.root = Path(tempfile.mkdtemp(prefix=f"am-{name}-"))
        self.home = self.root / "home"
        self.project = self.root / "project"
        self.home.mkdir()
        self.project.mkdir()
        # Windows reads USERPROFILE (and the `dirs` crate asks the OS, which ignores both), so scenarios there run in a CI VM.
        self.env = {**os.environ, "HOME": str(self.home), "USERPROFILE": str(self.home), "AUTOMEMORY_PASSWORD": ""}
        # A Tracewise installed on the machine must not leak into scenarios about "no Tracewise" (the sandbox HOME hides ~/.tracewise, PATH does not).
        keep = [d for d in self.env.get("PATH", "").split(os.pathsep) if d and not os.path.exists(os.path.join(d, "tracewise"))]
        self.env["PATH"] = os.pathsep.join(keep)
        self._git("init", "-q", "-b", "main")
        (self.project / "README.md").write_text("# sandbox\n")
        self._git("add", "-A")
        self._git("-c", "user.name=t", "-c", "user.email=t@t", "commit", "-qm", "init")

    def _git(self, *args):
        subprocess.run(["git", *args], cwd=self.project, check=True, capture_output=True)

    def cleanup(self):
        shutil.rmtree(self.root, ignore_errors=True)

    def run(self, *args, stdin: str | None = None, timeout: int = 30):
        return subprocess.run([self.bin, *args], input=stdin, capture_output=True, text=True,
                              cwd=self.project, env=self.env, timeout=timeout)

    def hook(self, agent: str, event: str, payload: dict):
        """Runs `automemory hook` and returns (parsed stdout JSON or None, raw stdout, stderr)."""
        r = self.run("hook", "--agent", agent, "--event", event, "--project", "auto",
                     stdin=json.dumps(payload))
        out = r.stdout.strip()
        try:
            parsed = json.loads(out) if out else None
        except json.JSONDecodeError:
            parsed = None
        return parsed, out, r.stderr

    def mcp(self, tool: str, **arguments):
        """One MCP tools/call over stdio; returns the tool's parsed JSON result (or raw text)."""
        msgs = [
            {"jsonrpc": "2.0", "id": 1, "method": "initialize",
             "params": {"protocolVersion": "2024-11-05", "capabilities": {},
                        "clientInfo": {"name": "test", "version": "1"}}},
            {"jsonrpc": "2.0", "id": 2, "method": "tools/call",
             "params": {"name": tool, "arguments": arguments}},
        ]
        r = self.run("mcp", stdin="\n".join(json.dumps(m) for m in msgs) + "\n")
        for line in r.stdout.splitlines():
            resp = json.loads(line)
            if resp.get("id") == 2:
                text = resp["result"]["content"][0]["text"]
                try:
                    return json.loads(text)
                except json.JSONDecodeError:
                    return text
        raise RuntimeError(f"no MCP response: {r.stdout!r} {r.stderr!r}")


class Report:
    """Collects pass/fail lines and sets the exit code."""

    def __init__(self, title: str):
        self.title, self.failed = title, 0
        print(f"== {title}")

    def check(self, ok: bool, label: str, detail: str = ""):
        print(f"  {'PASS' if ok else 'FAIL'}  {label}" + (f"  [{detail}]" if detail and not ok else ""))
        self.failed += 0 if ok else 1

    def finish(self):
        print(f"-- {self.title}: {'OK' if not self.failed else str(self.failed) + ' FAILED'}")
        return 1 if self.failed else 0


def percentile(values, p):
    s = sorted(values)
    return s[min(len(s) - 1, int(round(p / 100 * (len(s) - 1))))]


def now_ms():
    return time.perf_counter() * 1000
