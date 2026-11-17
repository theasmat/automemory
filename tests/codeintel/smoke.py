#!/usr/bin/env python3
"""CodeIntel smoke test against the real codebase-memory-mcp engine and this repository.

Skips (exit 0 with a notice) when the engine is not installed. Indexes the repo, then asks
`memory.about` for a symbol whose callers are known: `parse_remote` (sync/backup_git.rs) is called by the git backup's setup, status and restore.
"""
import json
import os
import subprocess
import sys
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parents[1] / "lib"))
from harness import REPO, Report, Sandbox  # noqa: E402


def main():
    engine = os.environ.get("AUTOMEMORY_CBM_BIN") or next(
        (str(p) for d in os.environ.get("PATH", "").split(os.pathsep)
         if (p := Path(d) / "codebase-memory-mcp").is_file()),
        str(Path.home() / ".local/bin/codebase-memory-mcp"))
    if not Path(engine).is_file():
        print("SKIP: codebase-memory-mcp not installed")
        return 0
    rep = Report("CodeIntel via codebase-memory-mcp")
    sb = Sandbox("codeintel")
    try:
        idx = subprocess.run([engine, "cli", "--quiet", "index_repository",
                              json.dumps({"repo_path": str(REPO)})], capture_output=True, text=True)
        rep.check(idx.returncode == 0 and '"status":"indexed"' in idx.stdout.replace(" ", ""),
                  "engine indexes the repository", idx.stdout[:200] + idx.stderr[:200])
        msgs = [
            {"jsonrpc": "2.0", "id": 1, "method": "initialize",
             "params": {"protocolVersion": "2024-11-05", "capabilities": {},
                        "clientInfo": {"name": "t", "version": "1"}}},
            {"jsonrpc": "2.0", "id": 2, "method": "tools/call",
             "params": {"name": "memory.about", "arguments": {"target": "parse_remote"}}},
        ]
        # The engine keeps one index per account (under the real HOME), so this process keeps the
        # real HOME and only the AutoMemory database is isolated.
        r = subprocess.run([sb.bin, "mcp", "--db", str(sb.root / "am.db")],
                           input="\n".join(map(json.dumps, msgs)) + "\n",
                           capture_output=True, text=True, cwd=REPO,
                           env={**os.environ, "AUTOMEMORY_CBM_BIN": engine})
        callers = []
        for line in r.stdout.splitlines():
            d = json.loads(line)
            if d.get("id") == 2:
                callers = json.loads(d["result"]["content"][0]["text"]).get("callers", [])
        rep.check(any(k in c for c in callers for k in ("setup", "status", "restore", "provider")),
                  "memory.about returns real callers from the engine", f"callers={callers}")
        doctor = subprocess.run([sb.bin, "doctor"], capture_output=True, text=True, cwd=sb.project,
                                env={**sb.env, "AUTOMEMORY_CBM_BIN": engine})  # sandbox HOME: doctor writes a backup
        rep.check("CodeIntel engine" in doctor.stdout and "OK (codebase-memory-mcp" in doctor.stdout,
                  "doctor reports the engine version", doctor.stdout[-400:])
    finally:
        sb.cleanup()
    return rep.finish()


if __name__ == "__main__":
    sys.exit(main())
