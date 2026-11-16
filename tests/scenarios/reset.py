#!/usr/bin/env python3
"""`automemory reset`: needs a password that exists and matches, refuses an agent, and removes everything but settings, password and backups."""
import subprocess
import sys
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parents[1] / "lib"))
from harness import Report, Sandbox  # noqa: E402


def main():
    rep = Report("reset all data")
    sb = Sandbox("reset")
    sb.env.update({"AUTOMEMORY_SECRETS_BACKEND": "file", "AUTOMEMORY_KEY": "reset"})
    try:
        root = sb.project.resolve()
        db = sb.home / ".automemory" / "automemory.db"
        sb.mcp("project.register", slug="shop", display_name="shop", path=str(root))
        sb.mcp("memory.save", project_id="shop", type="fact", summary="Money is integer cents", content="never floats", agent="claude")
        rep.check(db.exists(), "there is data to reset")

        def reset(*args, env=None, stdin=""):
            return subprocess.run([sb.bin, "reset", *args], input=stdin, capture_output=True, text=True, cwd=sb.project, env=env or sb.env, timeout=60)

        # No password set: a destructive command refuses instead of running open.
        sb.env.pop("AUTOMEMORY_PASSWORD", None)
        r = reset("--yes")
        rep.check(r.returncode != 0 and "needs a human password" in (r.stdout + r.stderr) and db.exists(), "with no password set it refuses and removes nothing", r.stdout + r.stderr)

        subprocess.run([sb.bin, "rule", "passwd", "--from-env"], env={**sb.env, "AUTOMEMORY_PASSWORD": "correct horse"}, cwd=sb.project, capture_output=True, text=True)

        # An agent has no terminal and no password in its environment.
        r = reset("--yes")
        rep.check(r.returncode != 0 and db.exists(), "an agent (no terminal, no password) is refused", r.stdout + r.stderr)
        r = reset("--yes", env={**sb.env, "AUTOMEMORY_PASSWORD": "wrong"})
        rep.check(r.returncode != 0 and db.exists(), "a wrong password is refused", r.stdout + r.stderr)

        good = {**sb.env, "AUTOMEMORY_PASSWORD": "correct horse"}
        r = reset(env=good, stdin="")
        rep.check(r.returncode != 0 and db.exists(), "without --yes and without a terminal it will not run", r.stdout + r.stderr)
        r = reset(env=good, stdin="no\n")
        rep.check(db.exists(), "the typed confirmation is required", r.stdout + r.stderr)

        sb.run("backup")
        r = reset("--yes", env=good)
        rep.check(r.returncode == 0 and "Reset" in r.stdout, "the right password with --yes resets", r.stdout + r.stderr)
        home = sb.home / ".automemory"
        rep.check(not db.exists(), "the database is gone")
        rep.check((home / "config.toml").exists() and "password_hash" in (home / "config.toml").read_text(), "settings and the password stay")
        rep.check(any((home / "backups").glob("automemory-*.db")), "backups stay by default")
        rep.check(sb.run("memory", "list").returncode == 0, "the next command starts a fresh database")
        r = reset("--yes", "--include-backups", env=good)
        rep.check(r.returncode == 0 and not (home / "backups").exists(), "--include-backups removes the backups too", r.stdout + r.stderr)
    finally:
        sb.cleanup()
    return rep.finish()


if __name__ == "__main__":
    sys.exit(main())
