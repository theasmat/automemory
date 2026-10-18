//! Swappable embedding providers and the background indexing that fills the vector table.
//!
//! Recall has two modes (`[retrieval] mode`): `keyword`, which never touches this module, and
//! `hybrid`, which adds vector similarity from the provider chosen in `[embeddings]`. Vectors are
//! stored per model id, so changing provider re-embeds gradually and never mixes vector spaces.
//! Every failure degrades to keyword recall; recall itself never fails because of a provider.

use crate::config::EmbeddingSettings;
use crate::db::queries::memories::{embedding_text, text_hash};
use crate::db::Database;
use crate::ports::{EmbeddingProvider, LocalEmbeddings};
use crate::redaction::redact_text;
use anyhow::{anyhow, bail, Context, Result};
use serde_json::{json, Value};
use std::io::Write;
use std::process::{Command, Stdio};
use std::time::Duration;

fn normalize(mut v: Vec<f32>) -> Vec<f32> {
    let norm = v.iter().map(|x| x * x).sum::<f32>().sqrt();
    if norm > 1e-9 {
        v.iter_mut().for_each(|x| *x /= norm);
    }
    v
}

fn vectors_from(value: &Value, key: &str, expected: usize) -> Result<Vec<Vec<f32>>> {
    let list = value.get(key).and_then(|v| v.as_array()).ok_or_else(|| anyhow!("response has no `{key}` array"))?;
    let out: Vec<Vec<f32>> = list
        .iter()
        .map(|row| {
            row.as_array()
                .map(|a| a.iter().filter_map(|x| x.as_f64().map(|f| f as f32)).collect::<Vec<f32>>())
                .map(normalize)
                .unwrap_or_default()
        })
        .collect();
    if out.len() != expected || out.iter().any(|v| v.is_empty()) {
        bail!("provider returned {} usable vector(s) for {} text(s)", out.iter().filter(|v| !v.is_empty()).count(), expected);
    }
    Ok(out)
}

/// reqwest's blocking client must not run inside a tokio worker; a short-lived thread keeps
/// the call safe from both CLI and the async MCP server.
fn off_runtime<T: Send>(f: impl FnOnce() -> Result<T> + Send) -> Result<T> {
    std::thread::scope(|s| s.spawn(f).join().unwrap_or_else(|_| Err(anyhow!("embedding thread panicked"))))
}

fn is_loopback(url: &str) -> bool {
    ["//localhost", "//127.0.0.1", "//[::1]", "//0.0.0.0"].iter().any(|h| url.contains(h))
}

// ── Ollama ──────────────────────────────────────────────────────────────────────────
/// Free open models served locally by Ollama (`nomic-embed-text`, `bge-m3`, `mxbai-embed-large`, …).
pub struct OllamaProvider {
    endpoint: String,
    model: String,
    timeout: Duration,
}

impl EmbeddingProvider for OllamaProvider {
    fn embed(&self, text: &str) -> Result<Vec<f32>> {
        self.embed_batch(&[text.to_string()]).map(|mut v| v.remove(0))
    }

    fn embed_batch(&self, texts: &[String]) -> Result<Vec<Vec<f32>>> {
        let (url, body, timeout, n) = (
            format!("{}/api/embed", self.endpoint.trim_end_matches('/')),
            json!({ "model": self.model, "input": texts }),
            self.timeout,
            texts.len(),
        );
        off_runtime(move || {
            let client = reqwest::blocking::Client::builder().timeout(timeout).build()?;
            let resp = client.post(&url).json(&body).send().with_context(|| format!("cannot reach Ollama at {url}"))?;
            if !resp.status().is_success() {
                bail!("Ollama answered {} for model {:?}: {}", resp.status(), body["model"], resp.text().unwrap_or_default());
            }
            vectors_from(&resp.json::<Value>()?, "embeddings", n)
        })
    }

    fn model_id(&self) -> String {
        format!("ollama:{}", self.model)
    }
}

// ── OpenAI-compatible HTTP ──────────────────────────────────────────────────────────
/// OpenAI, Voyage, Gemini's OpenAI-compatible endpoint, Azure, LiteLLM, LM Studio, vLLM, …
pub struct OpenAiProvider {
    endpoint: String,
    model: String,
    key_env: String,
    dimensions: Option<usize>,
    timeout: Duration,
}

impl EmbeddingProvider for OpenAiProvider {
    fn embed(&self, text: &str) -> Result<Vec<f32>> {
        self.embed_batch(&[text.to_string()]).map(|mut v| v.remove(0))
    }

    fn embed_batch(&self, texts: &[String]) -> Result<Vec<Vec<f32>>> {
        // Text leaves the machine: secrets are removed first.
        let safe: Vec<String> = if self.is_local() { texts.to_vec() } else { texts.iter().map(|t| redact_text(t)).collect() };
        let mut body = json!({ "model": self.model, "input": safe });
        if let Some(d) = self.dimensions {
            body["dimensions"] = json!(d);
        }
        let key = if self.key_env.is_empty() { None } else { std::env::var(&self.key_env).ok() };
        if !self.key_env.is_empty() && key.is_none() {
            bail!("environment variable {} (api_key_env) is not set", self.key_env);
        }
        let (url, timeout, n) = (format!("{}/embeddings", self.endpoint.trim_end_matches('/')), self.timeout, texts.len());
        off_runtime(move || {
            let client = reqwest::blocking::Client::builder().timeout(timeout).build()?;
            let mut req = client.post(&url).json(&body);
            if let Some(k) = key {
                req = req.bearer_auth(k);
            }
            let resp = req.send().with_context(|| format!("cannot reach {url}"))?;
            if !resp.status().is_success() {
                bail!("{} answered {}: {}", url, resp.status(), resp.text().unwrap_or_default().chars().take(300).collect::<String>());
            }
            let mut value = resp.json::<Value>()?;
            // Rows carry an `index`; order them before reading the vectors.
            let mut rows = value["data"].as_array().cloned().unwrap_or_default();
            rows.sort_by_key(|r| r["index"].as_u64().unwrap_or(0));
            value["vectors"] = Value::Array(rows.into_iter().map(|r| r["embedding"].clone()).collect());
            vectors_from(&value, "vectors", n)
        })
    }

    fn model_id(&self) -> String {
        match self.dimensions {
            Some(d) => format!("openai:{}/{d}", self.model),
            None => format!("openai:{}", self.model),
        }
    }

    fn is_local(&self) -> bool {
        is_loopback(&self.endpoint)
    }
}

// ── Any command ─────────────────────────────────────────────────────────────────────
/// The escape hatch: `{"texts": [...]}` on stdin, `{"vectors": [[...]]}` on stdout.
pub struct ExecProvider {
    command: String,
    model: String,
    timeout: Duration,
}

impl EmbeddingProvider for ExecProvider {
    fn embed(&self, text: &str) -> Result<Vec<f32>> {
        self.embed_batch(&[text.to_string()]).map(|mut v| v.remove(0))
    }

    fn embed_batch(&self, texts: &[String]) -> Result<Vec<Vec<f32>>> {
        let mut child = Command::new("sh")
            .arg("-c")
            .arg(&self.command)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .spawn()
            .with_context(|| format!("cannot run `{}`", self.command))?;
        child.stdin.take().context("no stdin")?.write_all(json!({ "texts": texts }).to_string().as_bytes())?;
        let start = std::time::Instant::now();
        loop {
            if child.try_wait()?.is_some() {
                break;
            }
            if start.elapsed() > self.timeout {
                let _ = child.kill();
                bail!("`{}` timed out after {}s", self.command, self.timeout.as_secs());
            }
            std::thread::sleep(Duration::from_millis(10));
        }
        let out = child.wait_with_output()?;
        if !out.status.success() {
            bail!("`{}` exited with {}", self.command, out.status);
        }
        vectors_from(&serde_json::from_slice::<Value>(&out.stdout).context("command output is not JSON")?, "vectors", texts.len())
    }

    fn model_id(&self) -> String {
        format!("exec:{}", if self.model.is_empty() { &self.command } else { &self.model })
    }

    fn is_local(&self) -> bool {
        true // the command is the user's own; what it does with the text is theirs to decide
    }
}

/// Builds the provider named in `[embeddings]`. Errors are explicit (missing endpoint, network not
/// allowed, optional feature not built) so `embeddings test` can say what to fix.
pub fn provider_from_settings(s: &EmbeddingSettings) -> Result<Box<dyn EmbeddingProvider>> {
    let timeout = Duration::from_secs(s.timeout_secs.max(1));
    match s.provider.to_ascii_lowercase().as_str() {
        "" | "ngram" | "local" => Ok(Box::new(LocalEmbeddings::new())),
        "ollama" => {
            if s.model.is_empty() {
                bail!("[embeddings] model is required for ollama (for example nomic-embed-text)");
            }
            let endpoint = if s.endpoint.is_empty() { "http://127.0.0.1:11434".to_string() } else { s.endpoint.clone() };
            if !is_loopback(&endpoint) && !s.allow_network {
                bail!("{endpoint} is not on this machine: set allow_network = true to send memory text there");
            }
            Ok(Box::new(OllamaProvider { endpoint, model: s.model.clone(), timeout }))
        }
        "openai" | "http" => {
            if s.model.is_empty() {
                bail!("[embeddings] model is required (for example text-embedding-3-small)");
            }
            let endpoint = if s.endpoint.is_empty() { "https://api.openai.com/v1".to_string() } else { s.endpoint.clone() };
            if !is_loopback(&endpoint) && !s.allow_network {
                bail!("{endpoint} would receive memory text: set allow_network = true to allow it");
            }
            if !is_loopback(&endpoint) && s.api_key_env.is_empty() {
                bail!("[embeddings] api_key_env must name the environment variable holding the API key");
            }
            Ok(Box::new(OpenAiProvider {
                endpoint,
                model: s.model.clone(),
                key_env: s.api_key_env.clone(),
                dimensions: s.dimensions,
                timeout,
            }))
        }
        "exec" => {
            if s.endpoint.trim().is_empty() {
                bail!("[embeddings] endpoint must hold the command for provider = \"exec\"");
            }
            Ok(Box::new(ExecProvider { command: s.endpoint.clone(), model: s.model.clone(), timeout }))
        }
        "onnx" => bail!("the onnx provider is an optional build feature that this binary does not include; use ollama (free, local) or openai"),
        other => bail!("unknown embedding provider {other:?} (ngram, ollama, openai, exec, onnx)"),
    }
}

/// Embeds up to `limit` memories that have no current vector for the provider's model.
/// Returns how many were embedded.
pub fn index_pending(db: &Database, provider: &dyn EmbeddingProvider, limit: usize) -> Result<usize> {
    let model = provider.model_id();
    let pending = db.memories_needing_vectors(&model, limit)?;
    let mut done = 0;
    for chunk in pending.chunks(16) {
        let texts: Vec<String> = chunk.iter().map(|(_, t, _)| t.clone()).collect();
        let vectors = provider.embed_batch(&texts)?;
        for ((id, _, hash), vector) in chunk.iter().zip(vectors) {
            db.upsert_memory_vector(id, &model, &vector, hash)?;
            done += 1;
        }
    }
    Ok(done)
}

/// Recomputes every vector of the provider's model (after changing how text is prepared, say).
pub fn reindex_all(db: &Database, provider: &dyn EmbeddingProvider) -> Result<usize> {
    let model = provider.model_id();
    db.conn().execute("DELETE FROM memory_vectors WHERE model = ?1", rusqlite::params![model])?;
    index_pending(db, provider, usize::MAX)
}

/// One memory's text as it is embedded (exposed for tests and `embeddings test`).
pub fn text_of(summary: &str, content: &str) -> (String, String) {
    let t = embedding_text(summary, content);
    let h = text_hash(&t);
    (t, h)
}

/// A labelled query for `embeddings eval`: the memory ids (or summary fragments) that should rank high.
#[derive(Debug, serde::Deserialize)]
pub struct EvalQuery {
    pub query: String,
    pub expect: Vec<String>,
}

#[derive(Debug, Default, Clone)]
pub struct EvalScore {
    pub queries: usize,
    pub precision_at_3: f64,
    pub mrr: f64,
    pub mean_ms: f64,
}

fn hit(expect: &[String], id: &str, summary: &str) -> bool {
    expect.iter().any(|e| e == id || summary.to_lowercase().contains(&e.to_lowercase()))
}

/// Scores a ranking function over labelled queries: precision@3 (share of the top 3 that are
/// expected, capped by how many are expected) and mean reciprocal rank of the first hit.
pub fn eval_ranking(
    queries: &[EvalQuery],
    mut rank: impl FnMut(&str) -> Result<Vec<(String, String)>>,
) -> Result<EvalScore> {
    let mut score = EvalScore { queries: queries.len(), ..Default::default() };
    for q in queries {
        let start = std::time::Instant::now();
        let ranked = rank(&q.query)?;
        score.mean_ms += start.elapsed().as_secs_f64() * 1000.0;
        let top3 = ranked.iter().take(3).filter(|(id, s)| hit(&q.expect, id, s)).count();
        score.precision_at_3 += top3 as f64 / 3.0f64.min(q.expect.len().max(1) as f64);
        score.mrr += ranked.iter().position(|(id, s)| hit(&q.expect, id, s)).map_or(0.0, |p| 1.0 / (p as f64 + 1.0));
    }
    let n = queries.len().max(1) as f64;
    score.precision_at_3 /= n;
    score.mrr /= n;
    score.mean_ms /= n;
    Ok(score)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::{Read, Write};
    use std::net::TcpListener;

    /// A one-shot-per-connection HTTP server that records requests and answers with `reply(path, body)`.
    fn mock_server(reply: impl Fn(&str, &Value) -> (u16, Value) + Send + 'static) -> (String, std::sync::mpsc::Receiver<(String, String, Value)>) {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let url = format!("http://{}", listener.local_addr().unwrap());
        let (tx, rx) = std::sync::mpsc::channel();
        std::thread::spawn(move || {
            for stream in listener.incoming().flatten() {
                let mut stream = stream;
                let mut buf = vec![0u8; 65536];
                let mut total = 0;
                loop {
                    let n = stream.read(&mut buf[total..]).unwrap_or(0);
                    total += n;
                    let text = String::from_utf8_lossy(&buf[..total]).to_string();
                    if let Some(split) = text.find("\r\n\r\n") {
                        let len = text
                            .lines()
                            .find_map(|l| l.to_ascii_lowercase().strip_prefix("content-length:").map(|v| v.trim().parse::<usize>().unwrap_or(0)))
                            .unwrap_or(0);
                        if total >= split + 4 + len || n == 0 {
                            break;
                        }
                    } else if n == 0 {
                        break;
                    }
                }
                let text = String::from_utf8_lossy(&buf[..total]).to_string();
                let path = text.split_whitespace().nth(1).unwrap_or("").to_string();
                let auth = text.lines().find(|l| l.to_ascii_lowercase().starts_with("authorization:")).unwrap_or("").to_string();
                let body: Value = text.split("\r\n\r\n").nth(1).and_then(|b| serde_json::from_str(b).ok()).unwrap_or(Value::Null);
                let (status, resp) = reply(&path, &body);
                let _ = tx.send((path, auth, body));
                let payload = resp.to_string();
                let _ = write!(stream, "HTTP/1.1 {status} X\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{payload}", payload.len());
            }
        });
        (url, rx)
    }

    #[test]
    fn defaults_are_keyword_mode_and_the_built_in_provider() {
        let g = crate::config::GlobalConfig::default();
        assert!(!g.retrieval.is_hybrid());
        let p = provider_from_settings(&g.embeddings).unwrap();
        assert_eq!(p.model_id(), "ngram-v1");
        assert!(p.is_local());
        let toml = "[retrieval]\nmode = \"hybrid\"\n[embeddings]\nprovider = \"ollama\"\nmodel = \"nomic-embed-text\"\n";
        let parsed: crate::config::GlobalConfig = toml::from_str(toml).unwrap();
        assert!(parsed.retrieval.is_hybrid() && parsed.embeddings.provider == "ollama");
    }

    #[test]
    fn providers_refuse_unsafe_or_incomplete_settings_with_a_reason() {
        let mk = |provider: &str, model: &str, endpoint: &str, key: &str, net: bool| EmbeddingSettings {
            provider: provider.into(), model: model.into(), endpoint: endpoint.into(), api_key_env: key.into(),
            allow_network: net, ..Default::default()
        };
        let err = |s: EmbeddingSettings| provider_from_settings(&s).err().map(|e| e.to_string()).unwrap_or_default();
        assert!(err(mk("ollama", "", "", "", false)).contains("model is required"));
        assert!(err(mk("ollama", "m", "http://gpu-box:11434", "", false)).contains("allow_network"));
        assert!(provider_from_settings(&mk("ollama", "m", "http://127.0.0.1:11434", "", false)).is_ok());
        assert!(err(mk("openai", "text-embedding-3-small", "", "OPENAI_API_KEY", false)).contains("allow_network"));
        assert!(err(mk("openai", "text-embedding-3-small", "", "", true)).contains("api_key_env"));
        assert!(provider_from_settings(&mk("openai", "m", "", "OPENAI_API_KEY", true)).is_ok());
        assert!(err(mk("exec", "", "", "", false)).contains("command"));
        assert!(err(mk("onnx", "", "", "", false)).contains("optional build feature"));
        assert!(err(mk("magic", "", "", "", false)).contains("unknown embedding provider"));
    }

    #[test]
    fn ollama_batches_and_normalizes() {
        let (url, rx) = mock_server(|path, body| {
            assert_eq!(path, "/api/embed");
            let n = body["input"].as_array().unwrap().len();
            (200, json!({ "embeddings": (0..n).map(|i| vec![3.0, 4.0 * (i as f64 + 1.0)]).collect::<Vec<_>>() }))
        });
        let p = provider_from_settings(&EmbeddingSettings {
            provider: "ollama".into(), model: "nomic-embed-text".into(), endpoint: url, ..Default::default()
        }).unwrap();
        assert_eq!(p.model_id(), "ollama:nomic-embed-text");
        let out = p.embed_batch(&["a".into(), "b".into()]).unwrap();
        assert_eq!(out.len(), 2);
        assert!((out[0][0] - 0.6).abs() < 1e-6 && (out[0][1] - 0.8).abs() < 1e-6, "unit length");
        let (_, _, body) = rx.recv().unwrap();
        assert_eq!(body["model"], "nomic-embed-text");
        assert_eq!(body["input"], json!(["a", "b"]));
    }

    #[test]
    fn openai_sends_a_key_redacts_secrets_and_orders_rows() {
        std::env::set_var("AM_TEST_EMBED_KEY", "sk-test-123");
        let (url, rx) = mock_server(|path, _| {
            assert_eq!(path, "/v1/embeddings");
            // Rows deliberately out of order.
            (200, json!({ "data": [ { "index": 1, "embedding": [0.0, 1.0] }, { "index": 0, "embedding": [1.0, 0.0] } ] }))
        });
        // A loopback endpoint is treated as local; force the hosted code path by checking redaction on is_local=false separately.
        let p = OpenAiProvider {
            endpoint: format!("{url}/v1"), model: "m".into(), key_env: "AM_TEST_EMBED_KEY".into(), dimensions: Some(2),
            timeout: Duration::from_secs(5),
        };
        assert_eq!(p.model_id(), "openai:m/2");
        let out = p.embed_batch(&["first".into(), "second".into()]).unwrap();
        assert_eq!(out[0], vec![1.0, 0.0]);
        assert_eq!(out[1], vec![0.0, 1.0]);
        let (_, auth, body) = rx.recv().unwrap();
        assert!(auth.contains("Bearer sk-test-123"), "{auth}");
        assert_eq!(body["dimensions"], 2);
    }

    #[test]
    fn a_hosted_openai_endpoint_never_receives_secrets() {
        // is_local() is false for a non-loopback endpoint, which turns redaction on.
        let p = OpenAiProvider {
            endpoint: "https://embeddings.example.com/v1".into(), model: "m".into(), key_env: String::new(),
            dimensions: None, timeout: Duration::from_secs(1),
        };
        assert!(!p.is_local());
        let safe = redact_text("config uses OPENAI_API_KEY=sk-proj-abcdefghijklmnopqrstuvwxyz123456");
        assert!(!safe.contains("abcdefghijklmnopqrstuvwxyz123456"), "{safe}");
    }

    #[test]
    fn exec_provider_talks_json_over_stdio() {
        let p = provider_from_settings(&EmbeddingSettings {
            provider: "exec".into(),
            endpoint: r#"cat >/dev/null; printf '{"vectors":[[3,4],[0,2]]}'"#.into(),
            model: "stub".into(),
            ..Default::default()
        }).unwrap();
        let out = p.embed_batch(&["x".into(), "y".into()]).unwrap();
        assert!((out[0][0] - 0.6).abs() < 1e-6 && out[1] == vec![0.0, 1.0]);
        assert_eq!(p.model_id(), "exec:stub");
        let bad = provider_from_settings(&EmbeddingSettings { provider: "exec".into(), endpoint: "echo not-json".into(), ..Default::default() }).unwrap();
        assert!(bad.embed("x").is_err());
    }

    #[test]
    fn indexing_is_incremental_per_model_and_notices_edits() {
        use crate::models::*;
        let db = Database::open_in_memory().unwrap();
        let now = chrono::Utc::now();
        let mem = |id: &str, summary: &str| Memory {
            id: id.into(), scope: MemoryScope::Global, project_id: None, memory_type: MemoryType::Fact, content: format!("about {summary}"),
            summary: summary.into(), confidence: 0.9, importance: 0.5, freshness: 1.0, status: MemoryStatus::Active,
            source_kind: MemorySourceKind::Agent, version: Some(1), created_at: now, updated_at: now, last_recalled_at: None,
            tags: vec![], entities: vec![], metadata: json!({}),
        };
        db.save_memory_direct(&mem("m1", "login uses cookies")).unwrap();
        db.save_memory_direct(&mem("m2", "retries use backoff")).unwrap();

        let a = LocalEmbeddings::new();
        assert_eq!(index_pending(&db, &a, 100).unwrap(), 2);
        assert_eq!(index_pending(&db, &a, 100).unwrap(), 0, "nothing to do twice");
        let (embedded, active, others) = db.vector_coverage(&a.model_id()).unwrap();
        assert_eq!((embedded, active, others.len()), (2, 2, 0));

        // A different model starts from zero and does not disturb the first.
        struct Other;
        impl EmbeddingProvider for Other {
            fn embed(&self, _: &str) -> Result<Vec<f32>> { Ok(vec![1.0, 0.0]) }
            fn model_id(&self) -> String { "other:v1".into() }
        }
        assert_eq!(index_pending(&db, &Other, 100).unwrap(), 2);
        let (embedded, _, others) = db.vector_coverage(&a.model_id()).unwrap();
        assert_eq!((embedded, others), (2, vec![("other:v1".to_string(), 2)]));

        // Editing a memory makes only that memory stale.
        let mut m = db.get_memory_by_id("m1").unwrap().unwrap();
        m.summary = "login uses short-lived tokens".into();
        db.save_memory_direct(&m).unwrap();
        assert_eq!(index_pending(&db, &a, 100).unwrap(), 1);

        // Search only sees vectors of the asked model.
        let q = a.embed("login tokens").unwrap();
        let hits = db.search_memories_vector(&q, &a.model_id(), None, 5).unwrap();
        assert_eq!(hits[0].0.id, "m1");
        assert!(db.search_memories_vector(&q, "nonexistent:model", None, 5).unwrap().is_empty());
        assert_eq!(reindex_all(&db, &a).unwrap(), 2);
    }

    #[test]
    fn eval_scores_rankings() {
        let qs = vec![
            EvalQuery { query: "q1".into(), expect: vec!["m1".into()] },
            EvalQuery { query: "q2".into(), expect: vec!["retries".into()] },
        ];
        let perfect = eval_ranking(&qs, |q| {
            Ok(if q == "q1" { vec![("m1".into(), "login".into())] } else { vec![("m9".into(), "x".into()), ("m2".into(), "retries use backoff".into())] })
        }).unwrap();
        assert!((perfect.mrr - 0.75).abs() < 1e-9, "{perfect:?}"); // ranks 1 and 2
        let none = eval_ranking(&qs, |_| Ok(vec![("zzz".into(), "nothing".into())])).unwrap();
        assert_eq!((none.mrr, none.precision_at_3), (0.0, 0.0));
    }
}
