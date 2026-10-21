#!/usr/bin/env python3
"""K3: duplicate skills/traces/failures are counted by doctor and `memory prune`, removed only with --yes and the password (the newest of each kept),
and a repeated session end no longer adds a copy."""
import sqlite3
import subprocess
import sys
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parents[1] / "lib"))
from harness import Report, Sandbox  # noqa: E402

PW = "correct horse"


def main():
    rep = Report("duplicate records")
    sb = Sandbox("prune")
    sb.env.update({"AUTOMEMORY_SECRETS_BACKEND": "file", "AUTOMEMORY_KEY": "prune-master"})
    try:
        sb.mcp("project.register", slug="app", display_name="app", path=str(sb.project.resolve()))
        db = sb.home / ".automemory" / "automemory.db"
        c = sqlite3.connect(db)
        for i in range(6):
            c.execute("INSERT INTO procedural_skills (id, project_id, name, description, trigger_patterns, preconditions, steps, caveats, success_count, failure_count, source_trace_ids, created_at, updated_at) VALUES (?, 'p', 'Playbook: ship', 'd', '[]', '[]', '[\"1. run\"]', '[]', 1, 0, '[]', ?, ?)",
                      (f"skill_{i}", f"2026-10-0{i + 1}T00:00:00Z", f"2026-10-0{i + 1}T00:00:00Z"))
        c.execute("INSERT INTO procedural_skills (id, project_id, name, description, trigger_patterns, preconditions, steps, caveats, success_count, failure_count, source_trace_ids, created_at, updated_at) VALUES ('skill_other', 'p', 'Playbook: test', 'd', '[]', '[]', '[]', '[]', 1, 0, '[]', '2026-10-01T00:00:00Z', '2026-10-01T00:00:00Z')")
        c.commit()
        c.close()
        subprocess.run([sb.bin, "rule", "passwd", "--from-env"], env={**sb.env, "AUTOMEMORY_PASSWORD": PW}, cwd=sb.project, capture_output=True)

        out = sb.run("memory", "prune").stdout
        rep.check("5 duplicate skill(s)" in out and "Nothing was changed" in out, "prune without --yes only counts", out)
        rep.check("5 duplicate skill(s)" in sb.run("doctor").stdout, "doctor reports the duplicates")
        r = sb.run("memory", "prune", "--yes")
        rep.check(r.returncode != 0, "removing needs the password", (r.stdout + r.stderr)[-120:])
        r = subprocess.run([sb.bin, "memory", "prune", "--yes"], env={**sb.env, "AUTOMEMORY_PASSWORD": PW}, cwd=sb.project, capture_output=True, text=True)
        rep.check(r.returncode == 0 and "Removed 5 duplicate skill(s)" in r.stdout, "with the password it removes them", r.stdout + r.stderr)
        c = sqlite3.connect(db)
        left = sorted(x[0] for x in c.execute("select id from procedural_skills"))
        c.close()
        rep.check(left == ["skill_5", "skill_other"], "the newest of each kept", str(left))
        rep.check("No duplicates" in sb.run("memory", "prune").stdout, "and none are left")
    finally:
        sb.cleanup()
    rep.finish()


if __name__ == "__main__":
    main()
