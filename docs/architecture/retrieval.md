# Retrieval, embeddings and secret storage

How recall finds notes, how vector search is switched on and swapped, and where API keys live. Overview of the system: [overview](overview.md).

## Retrieval modes and swappable embeddings

Two modes, one switch, so vectors are never required and never loaded unless asked for:

```toml
[retrieval]
mode = "keyword"            # default. "hybrid" adds vector similarity
[embeddings]                # read only in hybrid mode
provider = "ngram"          # ngram | ollama | onnx | openai | http
model = "nomic-embed-text"
endpoint = ""               # ollama / openai-compatible base URL
api_key_env = ""            # NAME of an environment variable or keychain item, never the key itself
allow_network = false       # hosted providers need this set to true, per project if wanted
fallback = "keyword"        # any provider failure silently degrades to keyword recall
```

* **keyword mode (default):** FTS5 (BM25), every query word must match first; if that finds nothing, notes that contain any of the words that carry meaning (question words such as which/do/we/how are ignored), best match first. A question asked in a sentence finds the note that answers it when they share a word; a synonym ("database" for a note that says "Postgres") needs hybrid mode. + path/tag links + trust tier + recency/usage decay + diversity + token budget. No model, no network, deterministic, same speed as today (about 0.6 ms).
* **hybrid mode:** the same candidates plus vector similarity, merged with reciprocal rank fusion so a bad embedding cannot override an exact keyword or path match.

**Providers (all behind the existing `EmbeddingProvider` port, extended with `id`, `dimensions`, `embed_batch`, `local: bool`):**

| Provider | Cost | Runs | Notes |
|---|---|---|---|
| `ngram` | free | in process, offline | Today's character n-gram projection; fuzzy spelling, no semantics. Default in hybrid until another is configured |
| `ollama` | free | local server (`localhost:11434`) | Free open models (e.g. nomic-embed-text 768-d, bge, mxbai); no runtime bundled in our binary |
| `onnx` | free | in process | Optional cargo feature `embeddings-onnx`; small 384-d model (MiniLM / bge-small class) downloaded once to `~/.automemory/models/` with a checksum and an explicit prompt; keeps the default binary at about 10 MB |
| `openai` | paid | network | Any OpenAI-compatible endpoint (OpenAI, Voyage, Gemini-compat, LM Studio, LiteLLM, Azure) via `endpoint` + `model` + `api_key_env`; text is secret-redacted before it leaves the machine |
| `http` | any | external command or URL | JSON in, vector out, like exec hook handlers; the escape hatch for anything else |

**Swapping safely:** vectors are stored per `(memory_id, model_id)`; a query only ever compares vectors of the active
model. Changing provider or model starts a resumable background re-embed in `serve` (rate-limited);
until it finishes, hybrid recall uses what is embedded plus keyword. Commands: `automemory embeddings status | test |
reindex | eval`. `eval` runs a labelled query set (from `tests/eval`) per configured provider and prints
precision@3, MRR and latency, so a provider is adopted on evidence.

**Where embeddings may run:** never in hooks (separate processes; loading a model per call costs tens to hundreds of
ms). Path-triggered recall in hooks stays keyword/links. Query embedding happens in the long-lived MCP server
process (`automemory mcp` / `serve`) for `memory.ask|recall|about`; document embedding happens in `serve`.

**Search:** brute-force cosine in Rust is enough to about 50k memories at 384 dimensions (single-digit ms); `sqlite-vec` is
an optional later index. No vector database server.

**When to turn hybrid on:** only if `embeddings eval` beats keyword on labelled queries, or the store passes about
2-5k active memories. Evidence from other systems is mixed (docs/research/memory_improvement.md).

**Status (Oct 2026):** built and tested (`embeddings` module, CLI, `tests/scenarios/embeddings_modes.py`). Not built: `onnx` provider, vector sync between machines, background reindex in `serve`.

## Provider profiles, keys and indexing

Today: one provider is set through flat `[embeddings]` fields (`automemory embeddings use ...`), the API key can only come from an
environment variable (`api_key_env`), and the dashboard's **Recall settings** page saves to the browser's `localStorage` only (it changes
nothing in the daemon). Goal: choose and add models freely, keep cloud keys in the OS keychain, and see indexing happen.

### 1 Provider profiles and a model catalog
- `[embeddings] active = "<profile>"` and `[embeddings.profiles.<name>]` with `provider`, `model`, `endpoint`, `dimensions`, `key` (a reference such as
  `keychain:openai`, never the secret), `allow_network`, `batch_size`, `timeout_secs`. The old flat fields migrate into a profile named `default`.
- Several profiles can exist (for example `local` = Ollama and `cloud` = OpenAI); switching is one setting. Vectors are already stored per model id, so switching
  never mixes vector spaces.
- A built-in **preset catalog** (static table, with a "verify before release" note because vendor catalogs change): Ollama `nomic-embed-text` (768),
  `mxbai-embed-large` (1024), `bge-m3` (1024), `all-minilm` (384); OpenAI `text-embedding-3-small` (1536) and `-large` (3072, shortenable);
  Gemini (OpenAI-compatible endpoint), Mistral, Jina, Together, LM Studio, vLLM, LiteLLM through the same OpenAI-compatible adapter; any command (`exec`).
  Free-text model ids are always allowed. `automemory embeddings models` lists presets plus what a local Ollama has installed (`/api/tags`).
- Later adapters for providers whose request shape differs (Cohere, Voyage, Gemini native). ONNX stays a cargo feature (above).

### 2 Secrets in the OS keychain
- New `automemory-core/src/secrets.rs`: `get(name)`, `set(name, value)`, `delete(name)`, `status(name)` (where it comes from: env, keychain or none).
  Lookup order: the profile's env var, then the keychain, then nothing. Service `dev.automemory.secret`, account = secret name.
- Backends through the `keyring` crate (macOS Keychain, Windows Credential Manager, Linux Secret Service): one API, and the secret never appears on a command line.
  Where no keychain exists (headless Linux), an encrypted file `~/.automemory/secrets.enc` (mode 0600, sealed with the existing master key) with a visible warning.
- The existing master key code shells out to `security add-generic-password -w <key>`, which exposes the key in the process list for a moment; move it to the same backend.
- The secret is never written to `config.toml`, the database, the audit log, logs, `doctor` output, hand-offs, instruction files or the sync vault (each machine sets its own),
  and error messages are redacted. Hooks never read it (they are keyword only). Redaction is covered by a test that greps all outputs for a planted key.
- CLI: `automemory embeddings key set <profile>` (hidden prompt, or `--stdin`, or `--from-env`), `key status`, `key remove`. Dashboard: a write-only field (the key is never
  shown back), set/remove behind the human password (section on the password gate), and a "Saved in Keychain" state.

### 3 Recall settings page becomes real
- `GET/PUT /api/recall` read and write `[retrieval]` and the profiles (writes behind the human password); the `localStorage` save is removed.
- Layout as steps: 1 Keyword or Hybrid. 2 Where: this computer (Ollama, free), a cloud service, your own endpoint, or a command. 3 Model: dropdown from the catalog
  plus "other", with dimensions. 4 API key (cloud only) stored in the keychain. 5 **Test** (embeds one sentence, shows dimensions, latency, whether text leaves the
  machine) and **Index**.
- A privacy line states exactly what is sent (redacted summary and the first 1500 characters of content) and needs `allow_network` for any non-local endpoint.
  Optional per-project switch: `private` projects never use a cloud profile.

### 4 Indexing you can see and control
- A background worker in `serve` (queue, batch 16 to 64, rate-limit and retry with backoff, resume after restart, cancel) fills missing vectors; progress over the
  WebSocket: "412 of 1,300 memories embedded". `embeddings status --watch` shows the same in the terminal.
- Guards for paid services: an estimate before a large first index (tokens times a stored price table, labelled "estimate"), a daily token cap, and a confirmation above a set size.
- Dimension or model mismatches fail with a clear message; "Remove unused vectors" deletes vectors of models that are no longer active.
- A one-click "Compare keyword and hybrid on my notes" check (queries built from note titles) next to the existing `embeddings eval` file mode.

### 5 Phases and acceptance
| Phase | Scope | Done when |
|---|---|---|
| E1 | `secrets.rs` + keychain/encrypted-file backends, `embeddings key set|status|remove`, master key off the command line | A planted key never appears in any output, file or log; works with a fake backend in tests (`AUTOMEMORY_SECRETS_BACKEND=file`) |
| E2 | Profiles, presets, `embeddings profile add|list|use|remove`, `embeddings models`, migration of the flat fields | Two profiles can be switched and tested from the CLI; old configs still work |
| E3 | `/api/recall`, real Recall page with the five steps, key field, Test button | The page changes the daemon's behaviour and survives a restart; a wrong key shows a clear error |
| E4 | Indexing worker, progress, cost guard, unused-vector cleanup | A 1,000-memory index runs in the background with progress and can be cancelled and resumed |
| E5 | Native adapters (Cohere, Voyage, Gemini), ONNX feature | Optional |

Tests: mock embedding server (exists) with an `Authorization` check, key redaction grep, keychain fake, scenario `tests/scenarios/embeddings_modes.py` extended for profiles
and keys, dashboard scenario for `/api/recall` (human gate, key never returned).

**Status (Oct 2026):** E1 to E4 built and tested (`secrets.rs`, profiles and presets, `/api/recall`, Recall page, indexing with progress; scenarios `embeddings_modes.py` and `dashboard.py`). E5 not started.

**Decisions (settled Oct 2026)**
1. Keys use the **`keyring` crate** (native Keychain / Credential Manager / Secret Service), not per-OS shell shims; the encrypted file is only the fallback where no keychain exists.
2. First cloud support is the **OpenAI-compatible adapter** (OpenAI, Gemini's compatible endpoint, Mistral, Jina, Together, LM Studio, vLLM, LiteLLM). Native Cohere / Voyage / Gemini
   adapters wait for E5.
3. **Provider choices are not synced** between machines; each machine picks its own profile. **Keys are never synced.**
4. The master key moves to the same `keyring` backend in E1, so no secret is ever passed on a command line.
5. Cloud use is **off by default**: a non-local endpoint needs `allow_network`, and a `private` project can forbid cloud profiles.
