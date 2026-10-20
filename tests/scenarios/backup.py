#!/usr/bin/env python3
"""Backup, list, restore and the doctor check, end to end with the real binary."""
import sys
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parents[1] / "lib"))
from harness import Report, Sandbox  # noqa: E402


def main():
    rep = Report("backup and restore")
    s = Sandbox("backup")
    try:
        s.run("note", "Orders use cents")
        r = s.run("backup")
        rep.check("Saved" in r.stdout and "1 active" in r.stdout, "backup saves a copy", r.stdout + r.stderr)
        doc = s.run("doctor").stdout
        rep.check("Checking backups... ✓" in doc and "readable" in doc, "doctor is happy with readable backups", doc[-300:])
        backup = next((s.home / ".automemory/backups").glob("automemory-*.db"))
        s.run("note", "Totals include tax")
        rep.check("2 active" in s.run("backup", "list").stdout or "1 active" in s.run("backup", "list").stdout, "list shows copies and their note counts")

        r = s.run("backup", "restore", str(backup))
        rep.check(r.returncode != 0 and "--yes" in (r.stderr + r.stdout), "restore refuses to run without a terminal or --yes", r.stdout + r.stderr)
        r = s.run("backup", "restore", str(backup), "--yes")
        rep.check(r.returncode == 0 and "Restored" in r.stdout, "restore with --yes works", r.stdout + r.stderr)
        notes = s.run("memory", "list").stdout
        rep.check("Orders use cents" in notes and "Totals include tax" not in notes, "the database is back to the backup", notes[:300])
        kept = list((s.home / ".automemory/backups").glob("before-restore-*.db"))
        rep.check(len(kept) == 1, "the replaced database was kept", str(kept))

        junk = s.root / "junk.db"
        junk.write_text("nope")
        r = s.run("backup", "restore", str(junk), "--yes")
        rep.check(r.returncode != 0 and "Orders use cents" in s.run("memory", "list").stdout, "a damaged file is refused and nothing changes", r.stderr[-200:])

        (s.home / ".automemory/backups/automemory-19990101-000000.db").write_text("damaged")
        doc = s.run("doctor").stdout
        rep.check("Checking backups... ⚠ 1 unreadable" in doc, "doctor warns about an unreadable backup", doc[-300:])
    finally:
        s.cleanup()
    return rep.finish()


if __name__ == "__main__":
    sys.exit(main())
