#!/usr/bin/env python3
"""Performance gates besides hook latency (tests/perf/hook_p95.py): binary size and recall latency.

Usage: gates.py [notes=300]
Thresholds are generous on purpose (shared CI machines are noisy); they catch a 2x regression, not noise.
"""
import os
import statistics
import sys
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parents[1] / "lib"))
from harness import Report, Sandbox, now_ms, percentile  # noqa: E402

MAX_BINARY_MB = 16
MAX_RECALL_P95_MS = 150  # whole process: start-up + open DB + FTS search


def main():
    notes = int(sys.argv[1]) if len(sys.argv) > 1 else 300
    sb = Sandbox("gates")
    rep = Report(f"performance gates ({notes} notes)")
    try:
        size_mb = os.path.getsize(sb.bin) / 1e6
        rep.check(size_mb <= MAX_BINARY_MB, f"binary {size_mb:.1f} MB within {MAX_BINARY_MB} MB")
        for i in range(notes):
            sb.run("note", f"Note {i}: the service number {i} uses queue-{i % 17} and retries {i % 5} times")
        times = []
        for q in ("queue-3", "retries", "service number 42", "queue-16 retries 4") * 10:
            t = now_ms()
            sb.run("memory", "search", q)
            times.append(now_ms() - t)
        p95 = percentile(times, 95)
        print(f"  recall p50={statistics.median(times):.1f}ms p95={p95:.1f}ms")
        rep.check(p95 <= MAX_RECALL_P95_MS, f"recall p95 {p95:.1f}ms within {MAX_RECALL_P95_MS}ms")
    finally:
        sb.cleanup()
    return rep.finish()


if __name__ == "__main__":
    sys.exit(main())
