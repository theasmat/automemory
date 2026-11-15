#!/usr/bin/env python3
"""`automemory companion`: status, off/auto, checksummed install from a local fake release server, and removing Tracewise's duplicate hooks on connect."""
import hashlib
import http.server
import io
import json
import os
import stat
import sys
import tarfile
import threading
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parents[1] / "lib"))
from harness import Report, Sandbox  # noqa: E402


def fake_tracewise(dir_: Path, marker: Path, hooks_agent: str = "claude") -> Path:
    p = dir_ / "tracewise"
    p.write_text(f"""#!/bin/sh
cat >/dev/null 2>&1
[ "$1" = -C ] && shift 2
case "$1" in
  companion) echo '{{"api":1,"version":"9.9.9"}}' ;;
  agent) if [ "$2" = status ]; then echo '[{{"name":"{hooks_agent}","hooks":true}},{{"name":"codex","hooks":false}}]'; else echo "$@" >> {marker}; fi ;;
  setup) echo setup-ran >> {marker} ;;
  *) echo '{{"api":1,"event":"none","decision":"allow","reason":null,"context":[]}}' ;;
esac
""")
    p.chmod(p.stat().st_mode | stat.S_IEXEC)
    return p


def main():
    rep = Report("companion command")
    sb = Sandbox("companion-setup")
    try:
        root = Path(sb.root)
        tools = root / "tools"
        tools.mkdir()
        marker = root / "marker.txt"

        out = sb.run("companion").stdout
        rep.check("not installed" in out or "not available" in out, "with no Tracewise the status says nothing is locked", out[-200:])

        fake_tracewise(tools, marker)
        sb.env["AUTOMEMORY_TRACEWISE_BIN"] = str(tools / "tracewise")
        out = sb.run("companion").stdout
        rep.check("ready" in out and "9.9.9" in out and "ms" in out, "with Tracewise the status shows version and probe time", out[-300:])
        rep.check("claude" in out and "run twice" in out, "and warns that Tracewise's own hooks would run twice", out[-300:])
        rep.check("claude" in sb.run("doctor").stdout and "run twice" in sb.run("doctor").stdout, "doctor says the same")

        sb.run("connect", "claude")
        rep.check(marker.exists() and "uninstall claude" in marker.read_text(), "connect removes Tracewise's own hooks for that agent", marker.read_text() if marker.exists() else "no marker")

        out = sb.run("companion", "off").stdout
        rep.check("off" in out and "nothing is locked" in out, "companion off switches it off")
        out = sb.run("companion", "auto").stdout
        rep.check("ready" in out, "companion auto brings it back")

        # install from a local release server: checksum mismatch is refused, a good archive runs `tracewise setup`
        marker.write_text("")
        sb.env.pop("AUTOMEMORY_TRACEWISE_BIN")
        part = "macos-arm64" if os.uname().machine == "arm64" else "macos-x86_64"
        name = f"tracewise-9.9.9-{part}.tar.gz"
        buf = io.BytesIO()
        with tarfile.open(fileobj=buf, mode="w:gz") as t:
            src = fake_tracewise(tools, marker)
            t.add(src, arcname=f"tracewise-9.9.9-{part}/tracewise")
        blob = buf.getvalue()
        good = hashlib.sha256(blob).hexdigest()
        state = {"sum": "0" * 64}

        class H(http.server.BaseHTTPRequestHandler):
            def log_message(self, *a):
                pass

            def do_GET(self):
                port = self.server.server_address[1]
                if self.path == "/releases":
                    body = json.dumps([{"tag_name": "v9.9.9", "draft": False, "assets": [
                        {"name": name, "browser_download_url": f"http://127.0.0.1:{port}/a"},
                        {"name": name + ".sha256", "browser_download_url": f"http://127.0.0.1:{port}/s"}]}]).encode()
                elif self.path == "/a":
                    body = blob
                else:
                    body = f"{state['sum']}  {name}\n".encode()
                self.send_response(200)
                self.end_headers()
                self.wfile.write(body)

        srv = http.server.HTTPServer(("127.0.0.1", 0), H)
        threading.Thread(target=srv.serve_forever, daemon=True).start()
        sb.env["AUTOMEMORY_TRACEWISE_API"] = f"http://127.0.0.1:{srv.server_address[1]}/releases"
        if sys.platform != "darwin":
            print("  SKIP  install needs macOS")
        else:
            r = sb.run("companion", "install", "--yes")
            rep.check(r.returncode != 0 and "checksum mismatch" in (r.stdout + r.stderr) and "setup-ran" not in marker.read_text(), "a wrong checksum installs nothing", (r.stdout + r.stderr)[-200:])
            state["sum"] = good
            r = sb.run("companion", "install", "--yes")
            rep.check(r.returncode == 0 and "setup-ran" in marker.read_text(), "a verified archive runs `tracewise setup`", (r.stdout + r.stderr)[-200:])
        srv.shutdown()
    finally:
        sb.cleanup()
    return rep.finish()


if __name__ == "__main__":
    sys.exit(main())
