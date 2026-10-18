#!/usr/bin/env python3
"""A/B harness for "does memory help?" (PLAN Phase 2 gate). Stdlib only.

The agent runs are done by you (or a runner later); this tool makes them comparable:

  run_eval.py prepare <task.toml> --arm off|on|verify --run 1
      Check out the task's base commit into an isolated worktree and print the prompt,
      the worktree path and the arm's setup. Run your agent there.
  run_eval.py score <task.toml> --arm ... --run 1 [--tokens N] [--seconds S] [--corrections N]
      Run the task's verify command in that worktree; success is its exit code, never the
      agent's claim. Appends a row to results/results.csv.
  run_eval.py report
      Per-arm success rate, tokens, time, and paired per-task comparison against `off`.
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
    lines.append("\nGate (PLAN phase 3): memory-on not worse than off overall, and better on non-inferable tasks.")
    return "\n".join(lines)


def report(_args):
    path = RESULTS / "results.csv"
    if not path.exists():
        sys.exit("no results yet: run `score` first")
    print(summarize(read_rows(path)))


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
    sub.add_parser("report").set_defaults(fn=report)
    sub.add_parser("selftest").set_defaults(fn=selftest)
    a = ap.parse_args()
    a.fn(a)


if __name__ == "__main__":
    main()
