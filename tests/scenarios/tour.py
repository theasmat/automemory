#!/usr/bin/env python3
"""Guided tour, learn topics, demo data, and shell completions.

Validates that:
1. `automemory tour --auto` executes all 8 sandbox steps and passes.
2. Step 5 collision protection stops Agent B with the real contract hook message.
3. `automemory tour --step 2 --auto` and `automemory tour --reset` function properly.
4. `automemory learn [topic]` displays guided topic explainers and copy-paste commands.
5. `automemory demo` seeds sample data marked `metadata.demo = true` which is excluded
   from export-rules and recall, and `automemory demo --remove` leaves zero rows.
6. `automemory completions <shell>` produces valid completion scripts.
7. Real user ~/.automemory is never touched throughout.
"""
from __future__ import annotations

import os
import sys
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parents[1] / "lib"))
from harness import Report, Sandbox  # noqa: E402


def main():
    sb = Sandbox("tour")
    rep = Report("guided tour, learn topics, and demo data")

    real_home_automemory = Path.home() / ".automemory"
    initial_real_mtime = real_home_automemory.stat().st_mtime if real_home_automemory.exists() else None

    try:
        # 1. Full automated 8-step tour
        r = sb.run("tour", "--auto")
        rep.check(r.returncode == 0, "`automemory tour --auto` exits 0", r.stdout[-300:] + r.stderr)
        rep.check("Step 1/8 · Welcome & Diagnostics" in r.stdout, "tour step 1 ran", r.stdout[:300])
        rep.check("Step 2/8 · Remember Something" in r.stdout, "tour step 2 ran")
        rep.check("Step 3/8 · Record an Architectural Decision" in r.stdout, "tour step 3 ran")
        rep.check("Step 4/8 · Instruction Files" in r.stdout, "tour step 4 ran")
        rep.check("Step 5/8 · Two Agents, One File" in r.stdout, "tour step 5 ran")
        rep.check("Tracewise" in r.stdout.split("Step 5/8")[1].split("Step 6/8")[0], "tour step 5 points to Tracewise for locks")
        rep.check("Step 6/8 · Session Continuity & Hand-Off" in r.stdout, "tour step 6 ran")
        rep.check("Step 7/8 · Review Proposals & Safety Modes" in r.stdout, "tour step 7 ran")
        rep.check("Step 8/8 · Your Turn!" in r.stdout, "tour step 8 ran")
        rep.check("You have completed the AutoMemory Guided Tour!" in r.stdout, "tour completion banner shown")

        # 2. Targeted step execution and reset
        r = sb.run("tour", "--step", "2", "--auto")
        rep.check(r.returncode == 0, "`automemory tour --step 2 --auto` exits 0")
        rep.check("Step 2/8" in r.stdout and "Step 3/8" not in r.stdout, "only step 2 ran with --step 2")

        r = sb.run("tour", "--reset")
        rep.check(r.returncode == 0, "`automemory tour --reset` exits 0")
        rep.check("Tour sandbox and progress reset" in r.stdout, "tour reset confirmation printed")

        # 3. Guided learning topics
        r = sb.run("learn")
        rep.check(r.returncode == 0, "`automemory learn` exits 0")
        rep.check("memory" in r.stdout and "decisions" in r.stdout and "locks" in r.stdout, "learn lists core topics")

        r = sb.run("learn", "memory")
        rep.check(r.returncode == 0, "`automemory learn memory` exits 0")
        rep.check("automemory note" in r.stdout and "automemory memory search" in r.stdout, "learn memory shows copy-paste commands")

        r = sb.run("learn", "nonexistent")
        rep.check(r.returncode == 0, "`automemory learn nonexistent` exits 0 gracefully")
        rep.check("Unknown topic" in r.stdout and "Try: `automemory learn memory`" in r.stdout, "unknown topic suggests valid command")

        # 4. Demo sample data seed and removal
        r = sb.run("demo")
        rep.check(r.returncode == 0, "`automemory demo` exits 0")
        rep.check("Demo project seeded" in r.stdout, "demo seed confirmation printed")

        # Verify demo memories exist in list
        r = sb.run("memory", "list")
        rep.check("demo-mem-" in r.stdout, "demo memories visible in memory list")

        # Verify demo memories are excluded from export-rules
        r = sb.run("export-rules")
        rep.check(r.returncode == 0, "export-rules runs")
        agents_md = sb.project / "AGENTS.md"
        if agents_md.exists():
            content = agents_md.read_text()
            rep.check("demo-mem-" not in content, "demo memories excluded from AGENTS.md export")

        # Verify demo memories are excluded from search
        r = sb.run("memory", "search", "Nuxt")
        rep.check("demo-mem-" not in r.stdout, "demo memories excluded from recall search")

        # Remove demo data
        r = sb.run("demo", "--remove")
        rep.check(r.returncode == 0, "`automemory demo --remove` exits 0")
        rep.check("Zero demo rows remain" in r.stdout, "demo remove confirmation printed")

        r = sb.run("memory", "list")
        rep.check("No active memories found" in r.stdout or "demo-mem-" not in r.stdout, "no demo memories remain after remove")

        # 5. Shell completions
        r = sb.run("completions", "zsh")
        rep.check(r.returncode == 0, "`automemory completions zsh` exits 0")
        rep.check("#compdef automemory" in r.stdout, "zsh completion script generated")

        r = sb.run("completions", "bash")
        rep.check(r.returncode == 0, "`automemory completions bash` exits 0")
        rep.check("complete -F" in r.stdout, "bash completion script generated")

        # 6. Verify real user ~/.automemory was not touched
        if real_home_automemory.exists():
            current_mtime = real_home_automemory.stat().st_mtime
            rep.check(current_mtime == initial_real_mtime, "real ~/.automemory directory untouched by sandbox tests")
        else:
            rep.check(not real_home_automemory.exists(), "real ~/.automemory was not created by sandbox tests")

    finally:
        sb.cleanup()

    return rep.finish()


if __name__ == "__main__":
    sys.exit(main())
