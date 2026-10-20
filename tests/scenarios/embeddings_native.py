#!/usr/bin/env python3
"""Native Cohere, Voyage and Gemini embedding adapters against a mock server speaking each API's own shape,
through the real binary: profile, reindex, model id, and the request each service would receive."""
import json
import sys
import threading
from http.server import BaseHTTPRequestHandler, HTTPServer
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parents[1] / "lib"))
from harness import Report, Sandbox  # noqa: E402

SEEN = []


class Api(BaseHTTPRequestHandler):
    def do_POST(self):
        body = json.loads(self.rfile.read(int(self.headers["Content-Length"])))
        SEEN.append((self.path, self.headers.get("Authorization"), self.headers.get("x-goog-api-key"), body))
        if self.path.endswith("/embed"):
            out = {"embeddings": {"float": [[1.0, 0.0, 0.5] for _ in body["texts"]]}}
        elif self.path.endswith("/embeddings"):
            out = {"data": [{"index": i, "embedding": [0.2, 0.9, 0.1]} for i, _ in enumerate(body["input"])]}
        else:
            out = {"embeddings": [{"values": [0.3, 0.3, 0.9]} for _ in body["requests"]]}
        data = json.dumps(out).encode()
        self.send_response(200)
        self.send_header("Content-Type", "application/json")
        self.end_headers()
        self.wfile.write(data)

    def log_message(self, *a):
        pass


def main():
    rep = Report("native embedding adapters")
    s = Sandbox("native")
    srv = HTTPServer(("127.0.0.1", 0), Api)
    threading.Thread(target=srv.serve_forever, daemon=True).start()
    base = f"http://127.0.0.1:{srv.server_port}"
    try:
        s.mcp("project.register", slug="app", display_name="app", path=str(s.project))
        for t in ("Authentication flow", "Network retries"):
            s.mcp("memory.save", summary=t, content=t, type="fact", scope="project", project_id="app")
        presets = s.run("embeddings", "models").stdout
        rep.check(all(x in presets for x in ("cohere", "voyage", "gemini-native")), "the catalog lists the native presets", presets[:300])
        for provider, model, path_part in (("cohere", "embed-v4.0", "/embed"), ("voyage", "voyage-3.5", "/embeddings"), ("gemini", "gemini-embedding-001", ":batchEmbedContents")):
            SEEN.clear()
            r = s.run("embeddings", "profile", "add", provider, "--provider", provider, "--model", model, "--endpoint", base)
            rep.check(r.returncode == 0, f"{provider}: profile saved", (r.stdout + r.stderr)[:200])
            s.run("embeddings", "profile", "use", provider)
            r = s.run("embeddings", "reindex", "--all")
            rep.check("embedded 2 memories" in r.stdout, f"{provider}: reindex embeds both memories", r.stdout + r.stderr)
            rep.check(any(path_part in p for p, *_ in SEEN), f"{provider}: the request went to its own endpoint", str([p for p, *_ in SEEN]))
            st = s.run("embeddings", "status").stdout
            rep.check(f"{provider}:{model}" in st and "2/2" in st, f"{provider}: status shows its model id and full coverage", st)
        gem = [b for p, _, _, b in SEEN if "batchEmbedContents" in p]
        rep.check(gem and gem[0]["requests"][0]["taskType"] == "RETRIEVAL_DOCUMENT", "gemini: documents are indexed with the document task type")
    finally:
        srv.shutdown()
        s.cleanup()
    return rep.finish()


if __name__ == "__main__":
    sys.exit(main())
