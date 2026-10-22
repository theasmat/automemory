#!/usr/bin/env python3
"""A/B harness for "does memory help?" (roadmap, Phase 2 gate). Stdlib only.

The agent runs are done by you (or a runner later); this tool makes them comparable:

  run_eval.py prepare <task.toml> --arm off|on|verify --run 1
      Check out the task's base commit into an isolated worktree and print the prompt,
      the worktree path and the arm's setup. Run your agent there.
  run_eval.py score <task.toml> --arm ... --run 1 [--tokens N] [--seconds S] [--corrections N]
      Run the task's verify command in that worktree; success is its exit code, never the
      agent's claim. Appends a row to results/results.csv.
  run_eval.py report
      Per-arm success rate, tokens, time, and paired per-task comparison against `off`.
  run_eval.py auto <task.toml> --cmd 'claude -p {prompt}' [--cmd-on ...] [--arms off,on] [--runs 3]
      prepare + run the agent + score, for every arm and run.
  run_eval.py gate [--save]
      Fails when memory-on is worse than off, or under the stored baseline (baseline.json).
  run_eval.py selftest
      Checks the scoring and report maths on fake data (no agent, no network).

Arms: off = no automemory MCP/hooks; on = automemory connected; verify = on, and answers
carry verified/changed/stale labels (the default once Phase 3 is on).
"""
from __future__ import annotations

import argparse
import csv
import statistics
import subprocess
import sys
import tempfile
import time
from pathlib import Path

try:
    import tomllib  # Python 3.11+
except ModuleNotFoundError:  # system python 3.9/3.10
    tomllib = None

HERE = Path(__file__).resolve().parent
RESULTS = HERE / "results"
FIELDS = ["task", "arm", "run", "success", "tokens", "seconds", "corrections", "verify_seconds"]
ARMS = ("off", "on", "verify")


def load_task(path: Path) -> dict:
    """Task files are flat key = "value" TOML; parsed by hand when tomllib is unavailable."""
    text = path.read_text()
    if tomllib:
        return tomllib.loads(text)
    out = {}
    for line in text.splitlines():
        line = line.strip()
        if not line or line.startswith("#") or "=" not in line:
            continue
        k, v = line.split("=", 1)
        v = v.strip()
        out[k.strip()] = v[1:-1] if len(v) > 1 and v[0] == v[-1] == '"' else v
    return out


def run_dir(task: dict, arm: str, run: int) -> Path:
    return RESULTS / task["id"] / f"{arm}-{run}"


def prepare(args):
    task = load_task(Path(args.task))
    work = run_dir(task, args.arm, args.run) / "work"
    if work.exists():
        sys.exit(f"{work} exists; remove it or pick another --run")
    work.parent.mkdir(parents=True, exist_ok=True)
    repo = Path(task["repo"]).expanduser()
    subprocess.run(["git", "-C", str(repo), "worktree", "add", "--detach", str(work), task["base_commit"]],
                   check=True)
    print(f"\nworktree : {work}")
    print(f"arm      : {args.arm}")
    print({"off": "setup    : run the agent with NO automemory MCP server and NO automemory hooks",
           "on": "setup    : `automemory connect <agent>` in the worktree (MCP + hooks), fresh database",
           "verify": "setup    : as `on`; use recall answers with their verified/changed/stale labels"}[args.arm])
    if task.get("seed_memories"):
        print(f"seed     : load {task['seed_memories']} into the arm's database before starting (arms on/verify)")
    print(f"\nPROMPT\n------\n{task['prompt']}\n")
    print("When the agent is done, run:")
    print(f"  run_eval.py score {args.task} --arm {args.arm} --run {args.run} --tokens N --seconds S")


def score(args):
    task = load_task(Path(args.task))
    work = run_dir(task, args.arm, args.run) / "work"
    t = time.perf_counter()
    r = subprocess.run(task["verify"], shell=True, cwd=work, capture_output=True, text=True)
    verify_s = time.perf_counter() - t
    (work.parent / "verify.log").write_text(r.stdout + r.stderr)
    row = {"task": task["id"], "arm": args.arm, "run": args.run, "success": int(r.returncode == 0),
           "tokens": args.tokens, "seconds": args.seconds, "corrections": args.corrections,
           "verify_seconds": round(verify_s, 1)}
    append_row(RESULTS / "results.csv", row)
    print(f"{task['id']} {args.arm}#{args.run}: {'PASS' if row['success'] else 'FAIL'} (verify exit {r.returncode})")


def append_row(path: Path, row: dict):
    path.parent.mkdir(parents=True, exist_ok=True)
    new = not path.exists()
    with path.open("a", newline="") as f:
        w = csv.DictWriter(f, fieldnames=FIELDS)
        if new:
            w.writeheader()
        w.writerow(row)


def read_rows(path: Path) -> list[dict]:
    with path.open() as f:
        return [{**r, "success": int(r["success"]), "tokens": int(r["tokens"] or 0),
                 "seconds": float(r["seconds"] or 0), "corrections": int(r["corrections"] or 0)}
                for r in csv.DictReader(f)]


def summarize(rows: list[dict]) -> str:
    lines = [f"{'arm':8} {'runs':>5} {'success':>8} {'tokens/task':>12} {'sec/task':>9} {'corrections':>12}"]
    by_arm = {a: [r for r in rows if r["arm"] == a] for a in ARMS}
    for arm, rs in by_arm.items():
        if rs:
            lines.append(f"{arm:8} {len(rs):>5} {statistics.mean(r['success'] for r in rs):>8.0%} "
                         f"{statistics.mean(r['tokens'] for r in rs):>12.0f} "
                         f"{statistics.mean(r['seconds'] for r in rs):>9.0f} "
                         f"{statistics.mean(r['corrections'] for r in rs):>12.1f}")
    # Paired per-task success rate vs `off`: same tasks, same repos.
    tasks = sorted({r["task"] for r in rows})
    for arm in ("on", "verify"):
        deltas = []
        for t in tasks:
            off = [r["success"] for r in rows if r["task"] == t and r["arm"] == "off"]
            this = [r["success"] for r in rows if r["task"] == t and r["arm"] == arm]
            if off and this:
                deltas.append(statistics.mean(this) - statistics.mean(off))
        if deltas:
            better, worse = sum(d > 0 for d in deltas), sum(d < 0 for d in deltas)
            lines.append(f"\n{arm} vs off over {len(deltas)} paired tasks: mean success delta "
                         f"{statistics.mean(deltas):+.0%}; better on {better}, worse on {worse}, same on {len(deltas) - better - worse}")
    lines.append("\nGate (roadmap phase 3): memory-on not worse than off overall, and better on non-inferable tasks.")
    return "\n".join(lines)


def report(args):
    path = RESULTS / "results.csv"
    if not path.exists():
        sys.exit("no results yet: run `score` first")
    rows = read_rows(path)
    print(summarize(rows))
    if args.md:
        out = ["| arm | runs | success | tokens median (min-max) | seconds median (min-max) | corrections |", "|---|---|---|---|---|---|"]
        for arm in ARMS:
            rs = [r for r in rows if r["arm"] == arm]
            if rs:
                out.append(f"| {arm} | {len(rs)} | {statistics.mean(r['success'] for r in rs):.0%} | "
                           f"{spread([r['tokens'] for r in rs])} | {spread([r['seconds'] for r in rs])} | "
                           f"{statistics.mean(r['corrections'] for r in rs):.1f} |")
        print("\n" + "\n".join(out))


def auto(args):
    """Runs the agent for you: prepare, run `--cmd-<arm>` in the worktree, score. One row per run.
    Command templates take {prompt}; tokens are read from a `tokens=N` line the command prints (optional)."""
    import re
    import shlex
    task = load_task(Path(args.task))
    for arm in args.arms.split(","):
        template = getattr(args, f"cmd_{arm}") or args.cmd
        if not template:
            sys.exit(f"no command for arm {arm}: pass --cmd or --cmd-{arm}")
        for run in range(1, args.runs + 1):
            ns = argparse.Namespace(task=args.task, arm=arm, run=run)
            with open("/dev/null", "w") as null:
                real, sys.stdout = sys.stdout, null
                try:
                    prepare(ns)
                finally:
                    sys.stdout = real
            work = run_dir(task, arm, run) / "work"
            t = time.perf_counter()
            r = subprocess.run(template.replace("{prompt}", shlex.quote(task["prompt"])), shell=True, cwd=work,
                               capture_output=True, text=True, timeout=args.timeout)
            secs = time.perf_counter() - t
            (work.parent / "agent.log").write_text(r.stdout + r.stderr)
            m = re.search(r"tokens=(\d+)", r.stdout + r.stderr)
            score(argparse.Namespace(task=args.task, arm=arm, run=run, tokens=int(m.group(1)) if m else 0,
                                     seconds=round(secs, 1), corrections=0))


def spread(values):
    return f"{statistics.median(values):.0f} ({min(values):.0f}-{max(values):.0f})"


def gate(args):
    """Fails when memory-on is worse than off, or below the stored baseline (tests/eval/baseline.json)."""
    import json
    path = RESULTS / "results.csv"
    if not path.exists():
        sys.exit("no results yet")
    rows = read_rows(path)
    rate = lambda arm: statistics.mean(r["success"] for r in rows if r["arm"] == arm) if any(r["arm"] == arm for r in rows) else None
    off, on = rate("off"), rate("on")
    base_path = HERE / "baseline.json"
    problems = []
    if off is not None and on is not None and on < off:
        problems.append(f"memory-on success {on:.0%} is below off {off:.0%}")
    if base_path.exists() and on is not None:
        floor = json.loads(base_path.read_text())["on_success"] - args.tolerance
        if on < floor:
            problems.append(f"memory-on success {on:.0%} is below baseline floor {floor:.0%}")
    if args.save and on is not None:
        base_path.write_text(json.dumps({"on_success": on, "off_success": off, "runs": len(rows)}, indent=2))
        print(f"baseline saved: on {on:.0%}")
    print("GATE FAILED: " + "; ".join(problems) if problems else "gate ok")
    sys.exit(1 if problems else 0)


def selftest(_args):
    fake = [
        {"task": "t1", "arm": "off", "run": 1, "success": 0, "tokens": 9000, "seconds": 300, "corrections": 3},
        {"task": "t1", "arm": "on", "run": 1, "success": 1, "tokens": 7000, "seconds": 200, "corrections": 1},
        {"task": "t2", "arm": "off", "run": 1, "success": 1, "tokens": 5000, "seconds": 100, "corrections": 0},
        {"task": "t2", "arm": "on", "run": 1, "success": 1, "tokens": 5200, "seconds": 110, "corrections": 0},
    ]
    with tempfile.TemporaryDirectory() as d:
        csv_path = Path(d) / "r.csv"
        for r in fake:
            append_row(csv_path, {**r, "verify_seconds": 1})
        text = summarize(read_rows(csv_path))
    assert "on vs off over 2 paired tasks: mean success delta +50%" in text, text
    assert "better on 1, worse on 0, same on 1" in text, text
    with tempfile.TemporaryDirectory() as d:
        task = Path(d) / "t.toml"
        task.write_text('id = "auto-self"\nrepo = "%s"\nbase_commit = "HEAD"\nprompt = "make DONE.txt"\nverify = "test -f DONE.txt"\n' % HERE.parents[1])
        global RESULTS
        keep, RESULTS = RESULTS, Path(d) / "res"
        try:
            auto(argparse.Namespace(task=str(task), arms="off,on", runs=1, cmd="echo {prompt} > /dev/null; touch DONE.txt; echo tokens=42",
                                    cmd_off=None, cmd_on=None, cmd_verify=None, timeout=60))
            rows = read_rows(RESULTS / "results.csv")
            assert len(rows) == 2 and all(r["success"] == 1 and r["tokens"] == 42 for r in rows), rows
            subprocess.run(["git", "-C", str(HERE.parents[1]), "worktree", "prune"], check=False)
        finally:
            for w in (RESULTS / "auto-self").glob("*/work"):
                subprocess.run(["git", "-C", str(HERE.parents[1]), "worktree", "remove", "--force", str(w)], capture_output=True)
            RESULTS = keep
    print("selftest OK")


def main():
    ap = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    sub = ap.add_subparsers(dest="cmd", required=True)
    for name, fn in (("prepare", prepare), ("score", score)):
        p = sub.add_parser(name)
        p.add_argument("task")
        p.add_argument("--arm", choices=ARMS, required=True)
        p.add_argument("--run", type=int, default=1)
        if name == "score":
            p.add_argument("--tokens", type=int, default=0)
            p.add_argument("--seconds", type=float, default=0)
            p.add_argument("--corrections", type=int, default=0, help="times you had to repeat or correct context")
        p.set_defaults(fn=fn)
    p = sub.add_parser("auto")
    p.add_argument("task")
    p.add_argument("--arms", default="off,on")
    p.add_argument("--runs", type=int, default=3)
    p.add_argument("--cmd", help="agent command, {prompt} is replaced; e.g. 'claude -p {prompt} --permission-mode acceptEdits'")
    for a in ARMS:
        p.add_argument(f"--cmd-{a}", help=f"override the command for arm {a} (e.g. a different HOME or MCP config)")
    p.add_argument("--timeout", type=int, default=1800)
    p.set_defaults(fn=auto)
    p = sub.add_parser("report")
    p.add_argument("--md", action="store_true", help="also print a markdown table with median and spread")
    p.set_defaults(fn=report)
    p = sub.add_parser("gate")
    p.add_argument("--save", action="store_true", help="store the current memory-on success as the baseline")
    p.add_argument("--tolerance", type=float, default=0.1)
    p.set_defaults(fn=gate)
    sub.add_parser("selftest").set_defaults(fn=selftest)
    a = ap.parse_args()
    a.fn(a)


if __name__ == "__main__":
    main()
