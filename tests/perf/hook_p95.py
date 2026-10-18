#!/usr/bin/env python3
"""Hook latency as an agent experiences it: whole-process wall time per `automemory hook` call.

Usage: hook_p95.py [runs=200] [budget_ms=50]
The PRD budget is 5 ms p95 for the *pipeline*; process start-up is included here, so this
number is an upper bound. Use a release binary. The in-process pipeline mean comes from
`cargo bench -p automemory-core --bench perf`.
"""
import json
import statistics
import subprocess
import sys
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parents[1] / "lib"))
from harness import Report, Sandbox, now_ms, percentile  # noqa: E402


def main():
    runs = int(sys.argv[1]) if len(sys.argv) > 1 else 200
    budget = float(sys.argv[2]) if len(sys.argv) > 2 else 50.0
    sb = Sandbox("perf")
    rep = Report(f"hook latency over {runs} runs")
    try:
        payload = json.dumps({"session_id": "p1", "tool_name": "Bash", "tool_input": {"command": "ls -la"}})
        cmd = [sb.bin, "hook", "--agent", "claude", "--event", "pre-tool", "--project", "auto"]
        for _ in range(5):  # warm the page cache and create the database
            subprocess.run(cmd, input=payload, text=True, capture_output=True, cwd=sb.project, env=sb.env)
        times = []
        for _ in range(runs):
            t = now_ms()
            subprocess.run(cmd, input=payload, text=True, capture_output=True, cwd=sb.project, env=sb.env)
            times.append(now_ms() - t)
        p50, p95, p99 = (percentile(times, p) for p in (50, 95, 99))
        print(f"  mean={statistics.mean(times):.1f}ms p50={p50:.1f}ms p95={p95:.1f}ms p99={p99:.1f}ms "
              f"max={max(times):.1f}ms  binary={sb.bin}")
        rep.check(p95 < budget, f"p95 {p95:.1f}ms under {budget:.0f}ms (process start-up included)")
    finally:
        sb.cleanup()
    return rep.finish()


if __name__ == "__main__":
    sys.exit(main())
