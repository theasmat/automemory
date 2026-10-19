#!/usr/bin/env python3
"""Recall modes: keyword by default, hybrid with a swappable provider, safe fallbacks.

The provider is a stub command (exec) that maps words to concept axes, so "login" and
"authentication" land close together without any model download or network.
"""
import json
import sys
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parents[1] / "lib"))
from harness import Report, Sandbox  # noqa: E402

STUB = r'''
import json, sys
axes = {"login": 0, "authentication": 0, "signin": 0, "cookie": 1, "session": 1, "retry": 2, "backoff": 2}
out = []
for t in json.load(sys.stdin)["texts"]:
    v = [0.01, 0.01, 0.01]
    for w in t.lower().replace(".", " ").split():
        if w in axes: v[axes[w]] += 1
    out.append(v)
print(json.dumps({"vectors": out}))
'''


def main():
    rep = Report("embeddings modes")
    s = Sandbox("embeddings")
    try:
        stub = s.root / "embed_stub.py"
        stub.write_text(STUB)
        s.mcp("project.register", slug="app", display_name="app", path=str(s.project))
        s.mcp("memory.save", summary="Authentication flow", content="authentication uses signed cookie tokens",
              type="fact", scope="project", project_id="app")
        s.mcp("memory.save", summary="Network retries", content="retry with backoff on 503", type="fact",
              scope="project", project_id="app")

        out = s.run("embeddings", "status").stdout
        rep.check("recall mode: keyword" in out, "default mode is keyword", out)

        r = s.run("embeddings", "use", "ollama", "--model", "x", "--endpoint", "http://gpu-box:11434")
        rep.check(r.returncode != 0 and "allow_network" in (r.stderr + r.stdout), "a remote endpoint is refused without --allow-network", r.stderr[:200])
        out = s.run("embeddings", "status").stdout
        rep.check("recall mode: keyword" in out, "a refused change leaves keyword mode on", out)

        r = s.run("embeddings", "use", "exec", "--model", "stub", "--endpoint", f"python3 {stub}")
        rep.check(r.returncode == 0, "exec provider accepted", r.stderr[:200])
        r = s.run("embeddings", "test")
        rep.check("3-dimension" in r.stdout, "provider test works", r.stdout + r.stderr)
        r = s.run("embeddings", "reindex")
        rep.check("embedded 2 memories" in r.stdout, "reindex embeds both memories", r.stdout + r.stderr)
        r = s.run("embeddings", "reindex")
        rep.check("embedded 0 memories" in r.stdout, "a second reindex does nothing", r.stdout)
        out = s.run("embeddings", "status").stdout
        rep.check("2/2" in out and "exec:stub" in out, "status shows full coverage", out)

        # Keyword search finds nothing for a word that is in no memory; hybrid recall finds the meaning.
        found = s.mcp("memory.search", query="login", project_id="app")
        rep.check("Authentication flow" in json.dumps(found), "hybrid recall finds 'login' -> authentication", json.dumps(found)[:300])

        evalf = s.root / "eval.json"
        evalf.write_text(json.dumps([{"query": "login", "expect": ["Authentication flow"]},
                                     {"query": "retry", "expect": ["Network retries"]}]))
        r = s.run("embeddings", "eval", str(evalf))
        rep.check("hybrid" in r.stdout and "keyword" in r.stdout, "eval prints both modes", r.stdout + r.stderr)

        # A broken provider never breaks recall.
        s.run("embeddings", "use", "exec", "--endpoint", "false")
        found = s.mcp("memory.search", query="retry backoff", project_id="app")
        rep.check("Network retries" in json.dumps(found), "a failing provider falls back to keyword recall", json.dumps(found)[:300])

        s.run("embeddings", "off")
        out = s.run("embeddings", "status").stdout
        rep.check("recall mode: keyword" in out, "off returns to keyword", out)

        # Hooks stay keyword-only: a hanging provider must not slow a prompt hook.
        s.run("embeddings", "use", "exec", "--endpoint", "sleep 30")
        import time
        t = time.time()
        s.hook("claude", "prompt", {"session_id": "e1", "prompt": "how does login work"})
        rep.check(time.time() - t < 5, "hooks never wait on the provider", f"{time.time()-t:.1f}s")

        # ── Profiles and keys: the key lives in the secret store only ──
        import http.server, threading, os
        seen = []

        class Mock(http.server.BaseHTTPRequestHandler):
            def do_POST(self):
                n = int(self.headers.get("Content-Length", 0))
                body = json.loads(self.rfile.read(n) or b"{}")
                seen.append(self.headers.get("Authorization", ""))
                data = [{"index": i, "embedding": [0.6, 0.8]} for i in range(len(body.get("input", [])))]
                out = json.dumps({"data": data}).encode()
                self.send_response(200); self.send_header("Content-Length", str(len(out))); self.end_headers(); self.wfile.write(out)
            def log_message(self, *a): pass

        srv = http.server.HTTPServer(("127.0.0.1", 0), Mock)
        threading.Thread(target=srv.serve_forever, daemon=True).start()
        s.env.update({"AUTOMEMORY_SECRETS_BACKEND": "file", "AUTOMEMORY_KEY": "trial-master"})
        planted = "sk-PLANTED-9f8e7d6c5b4a"
        r = s.run("embeddings", "profile", "add", "cloud", "--preset", "openai-small", "--allow-network")
        rep.check(r.returncode == 0, "a cloud profile is saved from a preset", r.stdout + r.stderr)
        r = s.run("embeddings", "key", "status", "cloud")
        rep.check("not set" in r.stdout, "it starts without a key", r.stdout)
        r = s.run("embeddings", "profile", "add", "mock", "--provider", "openai", "--model", "m",
                  "--endpoint", f"http://127.0.0.1:{srv.server_address[1]}/v1")
        rep.check(r.returncode == 0, "a custom endpoint profile is saved", r.stdout + r.stderr)
        r = s.run("embeddings", "key", "set", "mock", "--stdin", stdin=planted + "\n")
        rep.check(r.returncode == 0 and planted not in r.stdout + r.stderr, "the key is stored and never echoed", r.stdout + r.stderr)
        r = s.run("embeddings", "key", "status", "mock")
        rep.check("encrypted file" in r.stdout or "keychain" in r.stdout, "key status says where it lives", r.stdout)
        r = s.run("embeddings", "profile", "use", "mock")
        rep.check(r.returncode == 0, "the profile becomes active", r.stdout + r.stderr)
        r = s.run("embeddings", "test")
        rep.check("2-dimension" in r.stdout, "the profile works against the endpoint", r.stdout + r.stderr)
        rep.check(seen and seen[-1] == f"Bearer {planted}", "the request carried the stored key", str(seen[-1:]))
        r = s.run("embeddings", "reindex")
        rep.check("embedded 2 memories" in r.stdout, "indexing with the profile", r.stdout + r.stderr)
        r = s.run("embeddings", "status")
        rep.check("profile:     mock" in r.stdout and "2/2" in r.stdout, "status shows the profile and coverage", r.stdout)
        leaked = [str(f) for f in s.home.rglob("*") if f.is_file() and planted.encode() in f.read_bytes()]
        rep.check(not leaked, "the key is in no file under the home folder", str(leaked))
        r = s.run("embeddings", "clean")
        rep.check(r.returncode == 0 and "removed" in r.stdout, "unused vectors can be removed", r.stdout)
        r = s.run("embeddings", "profile", "remove", "mock")
        r2 = s.run("embeddings", "key", "status", "mock")
        rep.check("not set" in r2.stdout, "removing a profile removes its key", r.stdout + r2.stdout)
        srv.shutdown()
    finally:
        s.cleanup()
    return rep.finish()


if __name__ == "__main__":
    sys.exit(main())
