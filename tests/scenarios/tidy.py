#!/usr/bin/env python3
"""`automemory tidy`: repeated notes and notes about files that are gone go to the Inbox; nothing changes until a person approves."""
import sqlite3
import sys
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parents[1] / "lib"))
from harness import Report, Sandbox  # noqa: E402


def main():
    rep = Report("nightly tidy")
    sb = Sandbox("tidy")
    sb.env.update({"AUTOMEMORY_SECRETS_BACKEND": "file", "AUTOMEMORY_KEY": "tidy"})
    try:
        root = sb.project.resolve()
        (root / "src").mkdir()
        (root / "src" / "kept.py").write_text("x")
        sb.mcp("project.register", slug="shop", display_name="shop", path=str(root))
        a = sb.mcp("memory.save", project_id="shop", type="fact", agent="claude", summary="Money is stored as integer cents", content="Prices and totals are integer cents, never floats, in every table")
        b = sb.mcp("memory.save", project_id="shop", type="fact", agent="codex", summary="Money stored as integer cents", content="Prices and totals are integer cents never floats in every table")
        sb.mcp("memory.save", project_id="shop", type="fact", agent="claude", summary="The refund handler lives in the old module", content="Look in the legacy refund module", files_modified=["src/old_refund.py"])
        sb.mcp("memory.save", project_id="shop", type="fact", agent="claude", summary="Cart totals are computed once", content="Totals come from the pricing function", files_modified=["src/kept.py", "src/old_refund.py"])
        db = lambda: sqlite3.connect(sb.home / ".automemory" / "automemory.db")  # noqa: E731

        r = sb.run("tidy", "--dry-run")
        rep.check(r.returncode == 0 and "1 note(s) that repeat another" in r.stdout and "1 note(s) about files that no longer exist" in r.stdout, "a dry run reports one repeat and one note about gone files", r.stdout + r.stderr)
        n = db().execute("select count(*) from review_proposals").fetchone()[0]
        rep.check(n == 0, "and files nothing")

        r = sb.run("tidy")
        pend = db().execute("select proposal_type, target_id from review_proposals where status = 'pending' order by proposal_type").fetchall()
        rep.check(r.returncode == 0 and {t for t, _ in pend} == {"duplicate_merge", "stale_note"}, "a real run files both in the Inbox", f"{pend} {r.stdout}")
        statuses = {row[0]: row[1] for row in db().execute("select summary, status from memories where project_id is not null")}
        rep.check(all(v == "active" for v in statuses.values()), "and no note was changed", str(statuses))
        before = db().execute("select count(*) from review_proposals").fetchone()[0]
        sb.run("tidy")
        rep.check(db().execute("select count(*) from review_proposals").fetchone()[0] == before, "a second run files nothing new")

        dup = next(i for t, i in pend if t == "duplicate_merge")
        sb.run("proposal", "list")
        pid = db().execute("select id from review_proposals where target_id = ? and proposal_type = 'duplicate_merge'", (dup,)).fetchone()[0]
        r = sb.run("proposal", "decide", pid, "--approve")
        st = db().execute("select status from memories where id = ?", (dup,)).fetchone()[0]
        rep.check(r.returncode == 0 and st == "archived", "approving the repeat archives only that note", f"{st} {r.stdout}{r.stderr}")
        rep.check("Checking nightly tidy... ✓" in sb.run("doctor").stdout, "doctor shows the tidy ran")
    finally:
        sb.cleanup()
    return rep.finish()


if __name__ == "__main__":
    sys.exit(main())
