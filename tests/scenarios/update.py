#!/usr/bin/env python3
"""`automemory update` against a mock release server: reports, asks, verifies the checksum, replaces atomically,
and refuses an unchecked or tampered download. Runs on a COPY of the binary, never the real one."""
import hashlib
import os
import io
import json
import platform
import shutil
import subprocess
import sys
import tarfile
import zipfile
import tempfile
import threading
from http.server import BaseHTTPRequestHandler, HTTPServer
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parents[1] / "lib"))
from harness import Report, find_binary  # noqa: E402

WIN = platform.system() == "Windows"
arch = {"arm64": "aarch64", "aarch64": "aarch64", "x86_64": "x86_64", "AMD64": "x86_64"}[platform.machine()]
osn = {"Darwin": "apple-darwin", "Linux": "unknown-linux-gnu", "Windows": "pc-windows-msvc"}[platform.system()]
ARCHIVE = f"automemory-{arch}-{osn}." + ("zip" if WIN else "tar.gz")
BIN = "automemory.exe" if WIN else "automemory"
STATE = {"tag": "v9.9.9", "tamper": False, "with_sum": True}


def make_archive():
    buf = io.BytesIO()
    if WIN:  # releases are zip files holding automemory.exe
        with zipfile.ZipFile(buf, "w") as z:
            z.writestr(BIN, b"MZ-fake NEW-BINARY")
        return buf.getvalue()
    with tarfile.open(fileobj=buf, mode="w:gz") as t:
        data = b"#!/bin/sh\necho NEW-BINARY\n"
        info = tarfile.TarInfo("automemory")
        info.size, info.mode = len(data), 0o755
        t.addfile(info, io.BytesIO(data))
    return buf.getvalue()


PAYLOAD = make_archive()


class Api(BaseHTTPRequestHandler):
    def do_GET(self):
        base = f"http://127.0.0.1:{self.server.server_port}"
        if self.path == "/latest":
            assets = [{"name": ARCHIVE, "browser_download_url": f"{base}/{ARCHIVE}"}]
            if STATE["with_sum"]:
                assets.append({"name": ARCHIVE + ".sha256", "browser_download_url": f"{base}/sum"})
            body = json.dumps({"tag_name": STATE["tag"], "assets": assets}).encode()
        elif self.path == "/sum":
            digest = "0" * 64 if STATE["tamper"] else hashlib.sha256(PAYLOAD).hexdigest()
            body = f"{digest}  {ARCHIVE}\n".encode()
        else:
            body = PAYLOAD
        self.send_response(200)
        self.end_headers()
        self.wfile.write(body)

    def log_message(self, *a):
        pass


def main():
    rep = Report("update")
    srv = HTTPServer(("127.0.0.1", 0), Api)
    threading.Thread(target=srv.serve_forever, daemon=True).start()
    tmp = Path(tempfile.mkdtemp(prefix="am-update-"))
    exe = tmp / BIN
    shutil.copy(find_binary(), exe)
    env = {"AUTOMEMORY_UPDATE_API": f"http://127.0.0.1:{srv.server_port}/latest", "PATH": "/usr/bin:/bin"}
    if WIN:  # tar.exe lives under System32 and the process needs SystemRoot
        env = {**os.environ, **{k: v for k, v in env.items() if k != "PATH"}}
    run = lambda *a: subprocess.run([str(exe), "update", *a], capture_output=True, text=True, env=env, input="n\n")
    original = exe.read_bytes()
    try:
        r = run("--check")
        rep.check("v9.9.9" in r.stdout and exe.read_bytes() == original, "--check reports the newer version and changes nothing", r.stdout + r.stderr)
        r = run()
        rep.check("Nothing changed" in r.stdout and exe.read_bytes() == original, "it asks first; 'n' changes nothing", r.stdout)
        STATE["tamper"] = True
        r = run("--yes")
        rep.check(r.returncode != 0 and "checksum mismatch" in r.stderr and exe.read_bytes() == original, "a tampered download is refused", r.stderr[-200:])
        STATE.update(tamper=False, with_sum=False)
        r = run("--yes")
        rep.check(r.returncode != 0 and ".sha256" in r.stderr and exe.read_bytes() == original, "a release without a checksum is refused", r.stderr[-200:])
        STATE["with_sum"] = True
        r = run("--yes")
        rep.check(r.returncode == 0 and b"NEW-BINARY" in exe.read_bytes(), "a verified download replaces the binary", r.stdout + r.stderr)
        rep.check(not list(tmp.glob(".automemory-update-*")) and not list(tmp.glob("*.new")), "no temporary files are left")
        STATE["tag"] = "v0.0.1"
        shutil.copy(find_binary(), exe)
        r = run("--check")
        rep.check("Up to date" in r.stdout, "an older release means up to date", r.stdout)
    finally:
        srv.shutdown()
        shutil.rmtree(tmp, ignore_errors=True)
    return rep.finish()


if __name__ == "__main__":
    sys.exit(main())
