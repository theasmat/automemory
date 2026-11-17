#!/usr/bin/env python3
"""Git backup (docs/specs/git-backup.md): `automemory backup git setup` against a plain git remote, the recovery phrase, a verified restore on a
second computer, the status when the remote goes away, and the refusals (command-running addresses, tokens shown on screen).

Two computers = two sandboxes with their own HOME; the remote is a bare folder, which is the same code path as GitHub, GitLab or Bitbucket over ssh.
The keys live in a file backend, so the real keychain is never touched."""
import json
import re
import shutil
import subprocess
import sys
import time
import urllib.request
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parents[1] / "lib"))
from harness import Report, Sandbox  # noqa: E402
sys.path.insert(0, str(Path(__file__).parent))
from dashboard import free_port, http  # noqa: E402

PW = {"X-AutoMemory-Password": "correct horse"}


def git(cwd, *args):
    return subprocess.run(["git", *args], cwd=cwd, capture_output=True, text=True, check=True).stdout


def machine(name):
    sb = Sandbox(name)
    sb.env.update({"AUTOMEMORY_SECRETS_BACKEND": "file", "AUTOMEMORY_DEVICE_NAME": name})
    sb.env.pop("AUTOMEMORY_KEY", None)
    return sb


def main():
    rep = Report("git backup")
    a = machine("backup-a")
    b = machine("backup-b")
    try:
        remote = a.root / "vault.git"
        remote.mkdir()
        git(remote, "init", "-q", "--bare", "-b", "main")
        a.mcp("project.register", slug="shop", display_name="shop", path=str(a.project))
        a.mcp("decision.record", topic="payments", decision="Use Stripe webhooks, not polling", reason="polling missed events", project_id="shop", agent="claude")

        r = a.run("backup", "git", "status")
        rep.check("off" in r.stdout, "before setup the status says it is off", r.stdout[:120])
        # looking must not leave a secret behind: status, doctor and verify never create the vault key (found when `doctor` put a key in a real keychain)
        a.run("doctor")
        a.run("backup", "git", "status", "--json")
        r = a.run("backup", "git", "verify")
        rep.check(r.returncode != 0, "verify without a backup says so")
        keyfile = a.home / ".automemory" / "master.key"
        rep.check(not keyfile.exists(), "status, doctor and verify create no vault key", str(keyfile))

        # refusals
        r = a.run("backup", "git", "setup", "ext::sh -c touch /tmp/pwned", "--yes")
        rep.check(r.returncode != 0 and not Path("/tmp/pwned").exists(), "an address that runs commands is refused, and nothing ran", (r.stdout + r.stderr)[:200])
        r = a.run("backup", "git", "setup", "ext::sh", "--yes")
        rep.check(r.returncode != 0 and "runs commands" in (r.stdout + r.stderr), "and so is the bare `ext::` form, with the reason", (r.stdout + r.stderr)[:200])
        r = a.run("backup", "git", "setup", "https://me:s3cr3t-token@127.0.0.1:9/x.git", "--yes")
        out = r.stdout + r.stderr
        rep.check(r.returncode != 0 and "s3cr3t-token" not in out and "***" in out, "an unreachable address fails with the token hidden on screen", out[:300])
        rep.check(not (a.home / ".automemory" / "config.toml").exists() or "s3cr3t" not in (a.home / ".automemory" / "config.toml").read_text(), "and nothing was saved")
        r = a.run("backup", "git", "setup")
        rep.check(r.returncode != 0, "without a terminal and without --yes, setup does not wait for input", (r.stdout + r.stderr)[:160])

        # setup on A
        r = a.run("backup", "git", "setup", str(remote), "--yes")
        out = r.stdout + r.stderr
        rep.check(r.returncode == 0, "setup against a bare repository succeeds", out[-400:])
        m = re.search(r"\n\s+((?:[a-z]+ ){23}[a-z]+)\n", out)
        phrase = m.group(1) if m else ""
        rep.check(len(phrase.split()) == 24, "it shows a 24-word recovery phrase", out[-500:])
        rep.check("Verified: the copy on the remote restores" in out, "and proves the copy on the remote restores", out[-500:])
        files = git(remote, "ls-tree", "-r", "--name-only", "main")
        rep.check(".enc" in files and ".jsonl" not in files, "the remote holds only encrypted files", files[:200])
        st = json.loads(a.run("backup", "git", "status", "--json").stdout)
        rep.check(st["state"] == "ok" and st["provider"] == "local" and st["last_ok"] and st["last_verified"] and st["key_present"], "status is ok, with the last sync and the last verification", json.dumps(st)[:300])

        # B restores from nothing but the address and the phrase
        r = b.run("backup", "git", "restore", str(remote), "--phrase", phrase)
        out = r.stdout + r.stderr
        rep.check(r.returncode == 0 and "This computer now holds" in out, "a second computer restores with the address and the phrase", out[-400:])
        dec = b.run("decision", "list").stdout
        rep.check("Stripe webhooks" in dec, "and has the decision made on the first computer", dec[:200])
        r = b.run("backup", "git", "restore", str(remote), "--phrase", "abandon " * 11 + "about")
        rep.check(r.returncode != 0 and "nothing was changed" in (r.stdout + r.stderr), "a wrong phrase is refused and changes nothing", (r.stdout + r.stderr)[-300:])
        dec = b.run("decision", "list").stdout
        rep.check("Stripe webhooks" in dec and json.loads(b.run("backup", "git", "status", "--json").stdout)["key_present"], "and the computer still has its decision and its own key")

        # the remote goes away: the status says so, and recovers
        gone = a.root / "vault-moved.git"
        remote.rename(gone)
        a.run("backup", "git", "now")
        st = json.loads(a.run("backup", "git", "status", "--json").stdout)
        rep.check(st["state"] == "failing" and st["last_error"], "when the remote disappears the status is failing, with the reason", json.dumps(st)[:300])
        gone.rename(remote)
        a.run("backup", "git", "now")
        st = json.loads(a.run("backup", "git", "status", "--json").stdout)
        rep.check(st["state"] == "ok" and not st["last_error"], "and recovers on the next successful sync", json.dumps(st)[:300])
        r = a.run("backup", "git", "verify")
        rep.check(r.returncode == 0 and "restores" in r.stdout, "`verify` proves it again on demand", r.stdout[:200])

        # the dashboard API on a third computer (not set up yet)
        c = machine("backup-c")
        try:
            subprocess.run([c.bin, "rule", "passwd", "--from-env"], env={**c.env, "AUTOMEMORY_PASSWORD": "correct horse"}, cwd=c.project, capture_output=True)
            remote2 = c.root / "vault2.git"
            remote2.mkdir()
            git(remote2, "init", "-q", "--bare", "-b", "main")
            port = free_port()
            server = subprocess.Popen([c.bin, "serve", "--port", str(port)], cwd=c.project, env=c.env, stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL)
            try:
                for _ in range(50):
                    try:
                        urllib.request.urlopen(f"http://127.0.0.1:{port}/api/stats", timeout=2)
                        break
                    except OSError:
                        time.sleep(0.2)
                code, body = http(port, "/api/backup/git")
                d = json.loads(body)
                rep.check(code == 200 and d["status"]["state"] == "off" and [p["name"] for p in d["providers"]][:3] == ["GitHub", "GitLab", "Bitbucket"], "the API reports the backup off and lists the providers with how to create a repository", body[:200])
                code, _ = http(port, "/api/backup/git/setup", "POST", None, {"url": str(remote2)})
                rep.check(code in (401, 403), "setting it up needs the password", str(code))
                code, body = http(port, "/api/backup/git/setup", "POST", PW, {"url": "ext::sh -c touch /tmp/pwned2"})
                rep.check(code == 409 and json.loads(body)["code"] == "invalid" and not Path("/tmp/pwned2").exists(), "a command-running address is refused with a reason", body[:200])
                code, body = http(port, "/api/backup/git/setup", "POST", PW, {"url": "https://me:tok3n@127.0.0.1:9/x.git"})
                d = json.loads(body)
                rep.check(code == 409 and d["code"] == "secret" and "tok3n" not in body, "an address with a token is stopped, and the token is not echoed", body[:200])
                code, body = http(port, "/api/backup/git/setup", "POST", PW, {"url": str(remote2)})
                d = json.loads(body)
                rep.check(code == 200 and d["ok"] and len(d["recovery_phrase"].split()) == 24 and d["verified"], "setup through the API backs up, verifies and returns the recovery phrase once", body[:200])
                code, body = http(port, "/api/backup/git")
                rep.check(json.loads(body)["status"]["state"] == "ok", "and the status is now ok")
                code, body = http(port, "/api/backup/git/verify", "POST", {}, {})
                rep.check(code == 200 and json.loads(body)["ok"], "check-the-restore works without a password (it only reads)", body[:200])
                code, _ = http(port, "/api/backup/git", "DELETE")
                rep.check(code in (401, 403), "turning it off needs the password", str(code))
                code, body = http(port, "/api/backup/git", "DELETE", PW)
                code2, body2 = http(port, "/api/backup/git")
                rep.check(code == 200 and json.loads(body2)["status"]["state"] == "off", "and then it is off again", body2[:160])
            finally:
                server.terminate()
                server.wait(timeout=5)
        finally:
            c.cleanup()

        # moving the backup to another repository (a recreated repo, another provider): the new one gets everything, not only what is new
        m = machine("backup-m")
        n = machine("backup-n")
        try:
            r1, r2 = m.root / "first.git", m.root / "second.git"
            for rr in (r1, r2):
                rr.mkdir()
                git(rr, "init", "-q", "--bare", "-b", "main")
            m.mcp("project.register", slug="shop", display_name="shop", path=str(m.project))
            m.mcp("decision.record", topic="queue", decision="No background queue", reason="keep it simple", project_id="shop", agent="claude")
            r = m.run("backup", "git", "setup", str(r1), "--yes")
            rep.check(r.returncode == 0 and "Verified" in r.stdout, "the first repository is set up and verified", (r.stdout + r.stderr)[-200:])
            # the database remembers what it sent, but the local vault folder is gone (a database restored from a local backup on a new computer,
            # or a deleted vault folder): the second repository must still receive everything
            shutil.rmtree(m.home / ".automemory" / "vault")
            r = m.run("backup", "git", "setup", str(r2), "--yes")
            out = r.stdout + r.stderr
            rep.check(r.returncode == 0 and "Verified: the copy on the remote restores (" in out and "0 decision(s)" not in out and "1 decision(s)" in out, "pointing it at a second repository sends everything again, and says so", out[-300:])
            phrase2 = re.search(r"\n\s+((?:[a-z]+ ){23}[a-z]+)\n", out).group(1)
            r = n.run("backup", "git", "restore", str(r2), "--phrase", phrase2)
            rep.check(r.returncode == 0 and "No background queue" in n.run("decision", "list").stdout, "and a computer restoring from the second repository has the decision", (r.stdout + r.stderr)[-200:])
        finally:
            m.cleanup()
            n.cleanup()

        # gaps: a project without git, an orphan id and an unregistered folder's scope (the real-world run that found them)
        d = machine("backup-d")
        e = machine("backup-e")
        try:
            remote3 = d.root / "vault3.git"
            remote3.mkdir()
            git(remote3, "init", "-q", "--bare", "-b", "main")
            plain = d.root / "plain-folder"
            plain.mkdir()
            d.mcp("project.register", slug="gitless", display_name="gitless", path=str(plain))
            d.mcp("project.register", slug="withgit", display_name="withgit", path=str(d.project))
            # the same repository registered a second time (what `connect` plus a manual register does): it must not become a gap either
            d.mcp("project.register", slug="withgit-dup", display_name="withgit-dup", path=str(d.project))
            d.mcp("memory.save", summary="Dup project note", content="kept in the second registration", type="gotcha", scope="project", project_id="withgit-dup")
            d.mcp("decision.record", topic="storage", decision="Keep carts in SQLite", reason="local-first", project_id="gitless", agent="claude")
            d.mcp("decision.record", topic="payments", decision="Stripe webhooks", reason="no polling", project_id="withgit", agent="claude")
            d.mcp("memory.save", summary="Orphaned note", content="filed under an id that is no project", type="gotcha", scope="project", project_id="auto")
            d.mcp("memory.save", summary="Scratch folder note", content="from an unregistered folder", type="gotcha", scope="project", project_id="folder-0123456789abcdef")
            r = d.run("backup", "git", "setup", str(remote3), "--yes")
            out = r.stdout + r.stderr
            rep.check(r.returncode == 0, "setup works with a git-less project and orphans present", out[-300:])
            st = json.loads(d.run("backup", "git", "status", "--json").stdout)
            reasons = sorted((g["project"], g["reason"]) for g in st["gaps"])
            rep.check(st["state"] == "partial" and reasons == [("auto", "unknown_project"), ("folder-0123456789abcdef", "unregistered_folder")], "status says partial and names exactly the data that cannot be backed up, with the reasons", json.dumps(st["gaps"])[:300])
            rep.check("NOT backed up" in d.run("backup", "git", "status").stdout and "project adopt auto" in d.run("backup", "git", "status").stdout, "the terminal status lists them with the fix")
            rep.check("gitless" not in json.dumps(st["gaps"]) and "withgit-dup" not in json.dumps(st["gaps"]), "the git-less project and a second registration of the same repository are not gaps: each has a key of its own")
            # restore elsewhere: the git-less project's decision made it
            phrase = re.search(r"\n\s+((?:[a-z]+ ){23}[a-z]+)\n", out).group(1)
            r = e.run("backup", "git", "restore", str(remote3), "--phrase", phrase)
            dec = e.run("decision", "list").stdout
            rep.check(r.returncode == 0 and "Keep carts in SQLite" in dec and "Stripe webhooks" in dec, "a second computer gets the decisions of both the git project and the git-less project", dec[:300])
            rep.check("Orphaned note" not in e.run("memory", "list", "--all").stdout, "and not the orphan (it was never sent)")
            # adopt: move the orphan into a real project; the next pass carries it
            r = d.run("project", "adopt", "auto", "withgit", "--dry-run")
            rep.check("would move" in r.stdout and "memories: 1" in r.stdout, "adopt --dry-run counts what would move", r.stdout[:200])
            r = d.run("project", "adopt", "auto", "withgit")
            rep.check(r.returncode == 0 and "Moved" in r.stdout, "adopt moves it", r.stdout[:200])
            r = d.run("project", "adopt", "withgit", "gitless")
            rep.check(r.returncode != 0, "adopt refuses to move one registered project into another", (r.stdout + r.stderr)[:200])
            d.run("backup", "git", "now")
            st = json.loads(d.run("backup", "git", "status", "--json").stdout)
            rep.check([g["project"] for g in st["gaps"]] == ["folder-0123456789abcdef"], "after adopting, only the unregistered folder's note remains a gap", json.dumps(st["gaps"])[:200])
            e.run("backup", "git", "now")
            rep.check("Orphaned note" in e.run("memory", "list", "--all").stdout or "Orphaned note" in e.run("memory", "list", "--project", "withgit").stdout, "and the adopted note reaches the other computer on the next pass")
        finally:
            d.cleanup()
            e.cleanup()
    finally:
        a.cleanup()
        b.cleanup()
    return rep.finish()


if __name__ == "__main__":
    sys.exit(main())
