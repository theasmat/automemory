#!/usr/bin/env python3
"""Soak test of the git backup (manual: it takes ten minutes; `SOAK_N=10 SOAK_SECONDS=30 python3 tests/perf/backup_soak.py` is the 30-second version).

Soak test of the git backup: a fake git project, 100 memories saved over 10 minutes the way agents work (save a note, the turn ends, a hook fires and starts
the automatic background backup), checkpoints every 20 notes, and at the end a verify BEFORE any manual backup plus a restore on a second computer."""
import json
import os
import re
import shutil
import statistics
import subprocess
import sys
import time
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parents[1] / "lib"))
from harness import Sandbox  # noqa: E402

N = int(os.environ.get("SOAK_N", "100"))
DURATION = float(os.environ.get("SOAK_SECONDS", "600"))
LOG = Path(os.environ.get("SOAK_LOG", "/tmp/soak.log"))
INTERVAL = DURATION / N


def say(msg):
    line = f"[{time.strftime('%H:%M:%S')}] {msg}"
    print(line, flush=True)
    with LOG.open("a") as f:
        f.write(line + "\n")


def git(cwd, *args):
    return subprocess.run(["git", *args], cwd=cwd, capture_output=True, text=True, check=True).stdout


TOPICS = ["cart totals", "invoice numbering", "webhook retries", "session expiry", "csv import", "tax rounding", "refund flow", "search index", "rate limiting", "audit log"]
KINDS = ["fact", "gotcha", "lesson", "preference"]


def note(i):
    t = TOPICS[i % len(TOPICS)]
    k = KINDS[i % len(KINDS)]
    body = (f"Note {i:03d} about {t}. " + " ".join(f"detail-{i}-{j}" for j in range(12)) +
            f" The team decided on approach {i % 7} for {t}; see the discussion on ticket FAKE-{1000 + i}.")
    return f"{t.capitalize()} note {i:03d}", body, k


def parse_verify(text):
    m = re.search(r"restores: (\d+) note", text)
    n = re.search(r"(\d+) note\(s\) here are not on the remote yet", text)
    return (int(m.group(1)) if m else None, int(n.group(1)) if n else 0)


def main():
    LOG.write_text("")
    a = Sandbox("soak-a")
    b = Sandbox("soak-b")
    for sb in (a, b):
        sb.env.update({"AUTOMEMORY_SECRETS_BACKEND": "file", "AUTOMEMORY_DEVICE_NAME": sb.root.name})
        sb.env.pop("AUTOMEMORY_KEY", None)
    summary = {"hook_ms": [], "save_ms": [], "checkpoints": []}
    try:
        # the fake repository: a git project with a few commits and a (fake) origin, so it has a real project key
        app = a.root / "fakeapp"
        app.mkdir()
        git(app, "init", "-q", "-b", "main")
        for n_, text in (("README.md", "# fakeapp\nA pretend shop used to soak-test the backup.\n"), ("src.py", "print('hello')\n"), ("tests.py", "assert True\n")):
            (app / n_).write_text(text)
            git(app, "add", "-A")
            git(app, "-c", "user.name=t", "-c", "user.email=t@t", "commit", "-qm", f"add {n_}")
        git(app, "remote", "add", "origin", "git@github.com:fake/fakeapp.git")
        a.mcp("project.register", slug="fakeapp", display_name="fakeapp", path=str(app))

        remote = a.root / "remote.git"
        remote.mkdir()
        git(remote, "init", "-q", "--bare", "-b", "main")
        r = a.run("backup", "git", "setup", str(remote), "--yes")
        out = r.stdout + r.stderr
        phrase = re.search(r"\n\s+((?:[a-z]+ ){23}[a-z]+)\n", out).group(1)
        say(f"fake repo ready, backup set up to a git remote (setup exit {r.returncode}); saving {N} memories over {DURATION:.0f}s, one every {INTERVAL:.1f}s")

        t0 = time.time()
        a.hook("claude", "session-start", {"session_id": "soak1", "hook_event_name": "SessionStart", "cwd": str(app)})
        for i in range(1, N + 1):
            due = t0 + (i - 1) * INTERVAL
            time.sleep(max(0, due - time.time()))
            title, body, kind = note(i)
            s = time.time()
            a.mcp("memory.save", summary=title, content=body, type=kind, scope="project", project_id="fakeapp", agent="claude")
            summary["save_ms"].append((time.time() - s) * 1000)
            # the agent's turn ends: the Stop hook runs and, when due, starts the background backup
            h = time.time()
            a.hook("claude", "stop", {"session_id": "soak1", "hook_event_name": "Stop", "stop_hook_active": False, "cwd": str(app)})
            summary["hook_ms"].append((time.time() - h) * 1000)
            if i % 20 == 0:
                st = json.loads(a.run("backup", "git", "status", "--json").stdout)
                commits = git(remote, "rev-list", "--count", "main").strip()
                v = a.run("backup", "git", "verify")
                remote_notes, not_yet = parse_verify(v.stdout + v.stderr)
                cp = {"memory": i, "elapsed_s": round(time.time() - t0), "remote_notes": remote_notes, "not_yet_on_remote": not_yet, "state": st["state"], "remote_commits": int(commits), "last_ok": (st["last_ok"] or "")[11:19]}
                summary["checkpoints"].append(cp)
                say(f"checkpoint {i:3d}: remote holds {remote_notes} notes, {not_yet} here not on it yet | state {st['state']} | {commits} commits | last ok {cp['last_ok']}")
        say(f"all {N} saved after {time.time() - t0:.0f}s; waiting 30s for the automatic backup to catch up (no manual backup)")
        time.sleep(30)

        st = json.loads(a.run("backup", "git", "status", "--json").stdout)
        v = a.run("backup", "git", "verify")
        remote_notes, not_yet = parse_verify(v.stdout + v.stderr)
        summary["final"] = {"remote_notes": remote_notes, "not_yet_on_remote": not_yet, "state": st["state"], "gaps": len(st["gaps"])}
        say(f"FINAL before any manual backup: remote holds {remote_notes} notes, {not_yet} not yet on it, state {st['state']}, gaps {len(st['gaps'])}")

        # a second computer restores from the git remote alone
        r = b.run("backup", "git", "restore", str(remote), "--phrase", phrase)
        import sqlite3
        db_a = sqlite3.connect(str(a.home / ".automemory" / "automemory.db"))
        db_b = sqlite3.connect(str(b.home / ".automemory" / "automemory.db"))
        ids_a = {x[0] for x in db_a.execute("select id from memories where summary like '% note %'")}
        ids_b = {x[0] for x in db_b.execute("select id from memories where summary like '% note %'")}
        texts_b = [x[0] for x in db_b.execute("select content from memories where summary like '% note %'")]
        summary["restore"] = {"exit": r.returncode, "on_A": len(ids_a), "on_B": len(ids_b), "missing_on_B": len(ids_a - ids_b), "duplicates_on_B": len(texts_b) - len(set(texts_b)),
                              "content_intact": all(t.startswith("Note ") for t in texts_b)}
        say(f"RESTORE on a second computer: exit {r.returncode}; A has {len(ids_a)} notes, B has {len(ids_b)}, missing on B {len(ids_a - ids_b)}, duplicates {summary['restore']['duplicates_on_B']}")

        vault = a.home / ".automemory" / "vault"
        size = sum(p.stat().st_size for p in vault.rglob("*") if p.is_file() and ".git" not in p.parts)
        files = len(list((vault / "deltas").glob("*.enc"))) if (vault / "deltas").exists() else 0
        gsize = int(subprocess.run(["du", "-sk", str(remote)], capture_output=True, text=True).stdout.split()[0])
        h = summary["hook_ms"]
        summary["growth"] = {"delta_files": files, "delta_bytes": size, "remote_kb": gsize, "commits": int(git(remote, "rev-list", "--count", "main").strip())}
        summary["hook_stats"] = {"mean_ms": round(statistics.mean(h), 1), "p95_ms": round(sorted(h)[int(len(h) * 0.95) - 1], 1), "max_ms": round(max(h), 1)}
        summary["save_stats"] = {"mean_ms": round(statistics.mean(summary["save_ms"]), 1), "max_ms": round(max(summary["save_ms"]), 1)}
        say("SUMMARY " + json.dumps({k: v for k, v in summary.items() if k not in ("hook_ms", "save_ms")}))
        say("DONE")
    except Exception as e:  # noqa: BLE001
        say(f"ERROR {type(e).__name__}: {e}")
        raise
    finally:
        a.cleanup()
        b.cleanup()


if __name__ == "__main__":
    main()
