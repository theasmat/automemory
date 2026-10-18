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
    finally:
        s.cleanup()
    return rep.finish()


if __name__ == "__main__":
    sys.exit(main())
