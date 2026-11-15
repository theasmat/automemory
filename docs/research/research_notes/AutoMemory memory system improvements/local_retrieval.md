# Local retrieval and storage options for a single Rust binary + SQLite (2026)

Scope: raising recall quality in AutoMemory (SQLite WAL, FTS5 BM25 × time decay × confidence, ~0.56 ms recall over 500 notes, hook budget < 5 ms) without adding a service, a network call or an LLM. Numbers below carry their source and date; vendor self-reports are flagged.

## 1. Local embedding models and Rust runtimes

### Takeaway
Two viable tiers exist. (a) Static embeddings (Model2Vec "potion" models, MIT, pure Rust via `model2vec-rs`) load in ~15 ms and embed hundreds of thousands of texts per second on one core at ~93–95% of all-MiniLM-L6-v2 quality; they fit inside a hook budget. (b) Small transformers (bge-small, e5-small, arctic-embed-xs, EmbeddingGemma-300M) score 50–61 on MTEB retrieval but need ONNX Runtime (`ort`/fastembed) or candle, which costs tens of MB of binary or a downloaded shared library and per-query CPU latency in the low-to-mid millisecond range even when well optimised; they belong in a background indexing path, not the hook.

### Cited Findings
- Model2Vec potion-base-32M: MTEB average 52.83 (Avg All) / 52.13 (Avg MTEB), "94.66% of all-MiniLM-L6-v2"; potion-base-8M 51.32, potion-base-4M 50.01, potion-base-2M 47.55; distilled from bge-base-en-v1.5. Self-reported by Minish Lab. — [potion-base-32M model card](https://huggingface.co/minishlab/potion-base-32M); [Model2Vec results page](https://minish.ai/packages/model2vec/results/)
- A retrieval-tuned static model exists: potion-retrieval-32M (32.3M params, English, from bge-base-en-v1.5). — [model2vec-rs README](https://docs.rs/crate/model2vec-rs/latest/source/README.md); [potion-retrieval-32M](https://huggingface.co/minishlab/potion-retrieval-32M)
- `model2vec-rs` 0.3.0 benchmark (Apple M3 Pro, 12 cores, potion-base-8M, 64,738 Wikipedia lines, medians of 25 passes): 671K texts/s single thread, 3.06M texts/s all cores, model load 15 ms; Python model2vec 0.10.0: 40K / 134K texts/s, 312 ms load. Models are safetensors with f32/f16/i8 weights; `local-only` feature disables downloads; `from_bytes` allows embedding the model in the binary; MIT licence. Vendor self-report. — [model2vec-rs README](https://docs.rs/crate/model2vec-rs/latest/source/README.md)
- Model2Vec classification throughput table (vendor): 24,744 samples/s for model2vec vs 716 for SetFit (MiniLM) vs 118 for bge-base + logreg on CPU, multiprocessing disabled. — [Model2Vec results](https://minish.ai/packages/model2vec/results/)
- snowflake-arctic-embed-xs: 22.6M params, 384 dims, MTEB Retrieval NDCG@10 50.15; its card lists bge-small-en-v1.5 at 51.68, e5-small-v2 at 49.04, nomic-embed-text-v1.5 at 53.25, all-MiniLM-L6-v2 at 41.95, gte-tiny 44.92. Vendor table. — [arctic-embed-xs model card](https://huggingface.co/Snowflake/snowflake-arctic-embed-xs)
- Granite Embedding R2 paper table (MTEB v2 retrieval column, header truncated in excerpt): bge-small-en-v1.5 51.7, e5-small-v2 49. — [arXiv 2508.21085](https://arxiv.org/pdf/2508.21085)
- LitSearch nDCG@10 (independent small-model benchmark): e5-small-v2 0.455, arctic-embed-xs 0.448, bge-small-en-v1.5 0.428, gte-small 0.424. — [sotaro.io](https://sotaro.io/posts/litsearch-small-model-benchmark)
- EmbeddingGemma-300M: 61.15 mean on MTEB(Multilingual v2), highest sub-500M model, lead holds under 4-bit quantization and truncation to 128 dims (Matryoshka); the paper gives no CPU latency. — [arXiv 2509.20354](https://arxiv.org/abs/2509.20354v3); [DeepMind page](https://deepmind.google/models/gemma/embeddinggemma/)
- fastembed (Rust, 7.7.0) runs on `pykeio/ort`; default text model bge-small-en-v1.5, default reranker bge-reranker-base; ~30 text models including nomic-embed, arctic-embed xs/s/m/l, gte, multilingual-e5, embeddinggemma-300m (incl. `EmbeddingGemma300MQ4`), Qwen3-Embedding; quantized variants via `Q` suffix (e.g. `BGESmallENV15Q`); models download from HF hub on first use (cache `.fastembed_cache`, `HF_ENDPOINT` mirror, local load via `try_new_from_user_defined`); candle backend behind `bert-text`/`qwen3`/`nomic-v2-moe` features; no model2vec support. — [fastembed README](https://docs.rs/crate/fastembed/latest/source/README.md)
- `ort` linking: `compile-static` "considerably shrinks the binary" but static linking is "not recommended by Microsoft"; `load-dynamic` avoids linking libonnxruntime into the binary; compiling ORT from source is "extremely slow". Community guide notes fastembed pins a specific `ort` 2.0.0-rc version (ABI must match). — [ort 1.14.1 docs](https://docs.rs/crate/ort/1.14.1); [ort README](https://github.com/BloopAI/ort); [ort skill guide](https://www.skillsdirectory.com/skills/bobmatnyc-ort)
- Runtime comparison (blog-grade evidence): Manticore reports ONNX Runtime "a noticeable step up over the Candle path" for all-MiniLM-L12-v2 on CPU and later rebuilt its ORT path for a further 14× gain; tract README says CPU "typically 2–4× slower than ORT" but pure Rust with musl/wasm targets; one blog cites candle binaries at 10–50 MB and tract at ~2 MB (unverified). — [Manticore ONNX speedup](https://manticoresearch.com/blog/onnx-embeddings-speedup); [Manticore 14× faster](https://dev.to/sanikolaev/14-faster-embeddings-how-we-rebuilt-the-onnx-path-in-manticore-4eom); [blazen-embed-tract](https://docs.rs/crate/blazen-embed-tract/0.5.3); [calmops](https://calmops.com/programming/rust/7-best-rust-ml-frameworks-edge-2025/)
- bge-small ONNX per-sentence latency: Infinity benchmark on GCP g2-standard-16 CPU, 256-sentence batches: ONNX ~125 s/10 requests, int8 ONNX ~100 s, torch ~257 s (≈39 ms/sentence quantized, batched); SetFit tutorial 2.19 ms/sample is on a T4 GPU, not CPU. — [Infinity benchmarking](https://michaelfeil.github.io/infinity/0.0.75/benchmarking); [SetFit ONNX tutorial](https://huggingface.co/docs/setfit/en/tutorials/onnx)
- Rust startup advantage: a research note on Rust alternatives to sentence-transformers claims 12× faster startup (claim, methodology thin). — [socratic-shell hippo](https://socratic-shell.github.io/hippo/research/rust-sentence-transformers-alternatives-claude-ai.html)

### Inferences
- For AutoMemory's existing n-gram projection slot, potion-retrieval-32M (or potion-base-8M, 7.5M params ≈ 15 MB f16 / ~8 MB i8 by arithmetic from params × bytes; not a published figure) is the drop-in: same "no service" property, pure Rust, MIT, ~1.5 µs per text single-threaded (from 671K texts/s), load 15 ms. Embedding the query inside a 5 ms hook is feasible if the model is loaded lazily or memory-mapped; the load itself is 15 ms so it must not sit on the hook path cold.
- Transformer models should run only in a background `index` pass (on save or idle), with the query embedding cached or computed by the static model; an ORT dependency roughly doubles binary size/build complexity and adds a shared-library or download step, which conflicts with "single binary" unless statically linked.
- Quality ordering for English short notes, best to worst among shippable sizes: EmbeddingGemma-300M (≈61) ≫ nomic-embed v1.5 (53) ≈ bge-small (51.7) ≈ arctic-xs (50.2) > e5-small (49) > potion-retrieval/base-32M (≈52 MTEB avg but weaker on retrieval than bge-small; no retrieval-only number found) > potion-8M. The gap between potion and bge-small is small relative to the 100–1000× speed difference.

### Gaps
- No measured CPU batch-1 latency for bge-small/arctic-xs/EmbeddingGemma on Apple Silicon or x86 via `ort` was found; the GPU and batched numbers above do not transfer. Must be measured.
- No measured release-binary size for `ort` static vs dynamic, candle, or tract on a BERT model; only blog estimates.
- No MTEB Retrieval-only (as opposed to average) score for potion-retrieval-32M was found.
- gte-small and nomic-embed had no clean same-benchmark comparison against arctic-xs.

## 2. Vector storage in SQLite and HNSW crates (10k–100k vectors)

### Takeaway
At AutoMemory's scale (≤ tens of thousands of notes) brute force is the right design: sqlite-vec full scan over 100k × 384 f32 is ~68 ms on an M1 Pro, so 10k is ~7 ms and 1k ~0.7 ms, linear. int8 or binary quantization with rescoring cuts that 3–17×. HNSW (usearch, hnsw_rs, instant-distance) only pays off above ~100k and costs build time, RAM and a second index to keep consistent.

### Cited Findings
- sqlite-vec, 100,000 × 384 f32, Apple M1 Pro 16 GB (Sep 2025): insert 1,179 ms, full-scan query 67.8 ms. sqlite-vector (competing vendor, same test): insert 563 ms, full scan 56.7 ms, int8 quantize once 225 ms, quantized scan 17.4 ms, quantized preloaded 3.97 ms, 37 MB memory, recall@20 = 1.0. libsql vector index "ran for hours without finishing". Author is sqlite-vector's vendor. — [Bambini, The State of Vector Search in SQLite](https://marcobambini.substack.com/p/the-state-of-vector-search-in-sqlite)
- sqlite-vec is brute force and "query latency scales exactly linearly" (fork maintainer). SimpleVecDB table: sqlite-vec v0.1.6 at 10,000 vectors, f32, k=10: 3.55 ms (hardware unstated). — [sqlite-vector fork](https://github.com/ntklink/sqlite-vector); [SimpleVecDB benchmarks](https://coderdayton.github.io/SimpleVecDB/benchmarks)
- Other sqlite-vec claims disagree: "<12 ms, p99 28 ms at 100k" and "4 ms for 100k 384-d k=10" from dev.to posts with no hardware or method stated; treat as unreliable. — [dev.to a](https://dev.to/hypernexus/sqlite-vector-search-the-dependency-free-ai-memory-stack-that-outperforms-pinecone-5d27); [dev.to b](https://dev.to/hypernexus/sqlite-vector-search-the-dependency-free-ai-memory-stack-28n3)
- sqlite-vec 0.1.7 guidance: prefer float16/int8 scalar quantization over binary for moderate savings; re-normalize after Matryoshka truncation. — [sqlite-vec skill notes](https://skills.sh/harlan-zw/skilld/sqlite-vec-skilld)
- Binary quantization: ~85–92% Recall@10 vs full precision, 96–98% with rescoring (vendor blog, 2026); practitioner guidance to fetch 3–5× candidates by Hamming distance then rescore with int8/f32, and binarize before truncating Matryoshka dims. — [CallSphere](https://callsphere.ai/blog/quantizing-embeddings-int8-binary-matryoshka-2026.md); [dev.to binary embeddings](https://dev.to/ji_ai/binary-quantized-embeddings-32x-smaller-vectors-recall-intact-46lk)
- sqlite-vss is described only as a prior, superseded project (Faiss-based); no current numbers. — [Bambini](https://marcobambini.substack.com/p/the-state-of-vector-search-in-sqlite)
- usearch: header-only C++20 core with Rust crate, Apache-2.0; storage types bf16, u1 (bit), i8, e4m3 by default (f16, u8, i4… in full build); serializable "SafeGraphs" files that can be memory-mapped/viewed from disk; self-reported 192-core Graviton5 numbers: build 161,952 vec/s, search 173,823 QPS vs FAISS HNSW 84,536 / 178,600 at ~0.99 recall@10; no per-vector memory figure. — [usearch repo](https://github.com/unum-cloud/usearch)
- hnsw_rs vs diskann-rs (200K × 128-d, M1 Max; diskann-rs author's numbers): hnsw_rs wins build time and max recall, diskann-rs wins QPS at 93% recall and inserts; hnsw_rs holds the full index in RAM. instant-distance last released June 2023 (0.6.1). No benchmark with all three Rust crates together was found. — [diskann-rs](https://docs.rs/crate/diskann-rs/latest); [instant-distance on lib.rs](https://lib.rs/crates/instant-distance); [generic HNSW in Rust post](https://pub.towardsai.net/building-a-generic-hnsw-index-in-rust-when-cosine-distance-isnt-enough-0679e46f9075)

### Inferences
- Plain BLOB brute force in Rust (read all vectors into a Vec<f32> or i8 once, dot product with auto-vectorised loops) will match or beat sqlite-vec's 68 ms/100k because sqlite-vec's cost is dominated by the same scan; a 384-d i8 dot product over 10k vectors is ~4 MB of memory traffic, well under 5 ms on any laptop. This needs no extension and no new dependency.
- Storing vectors as int8 BLOBs (384 bytes/note) keeps 10k notes at 3.8 MB and 100k at 38 MB, consistent with sqlite-vector's 37 MB figure; binary (48 bytes/note) plus rescoring is the next step if 100k+ is reached.
- HNSW adds a file to persist, a rebuild on restart or mmap, and tuning (M, ef); none is justified below ~100k notes at a 5 ms budget. usearch is the strongest candidate if it ever is (quantized types, mmap view, Apache-2.0) but drags in a C++ build.

### Gaps
- No independent 10k–100k benchmark of usearch/hnsw_rs/instant-distance on a laptop CPU; must be measured if HNSW is ever considered.
- No published memory-per-vector for usearch's Rust binding.
- No sqlite-vec int8/binary recall figures from the project itself.

## 3. Hybrid fusion and local rerankers

### Takeaway
Hybrid lexical+dense reliably beats BM25 alone (Elastic measured +18% avg nDCG@10 for RRF and +24% for a tuned linear blend on BEIR); RRF (k≈20–60) is the safe untuned default, a calibrated weighted blend wins more but needs ~40–300 labelled queries per corpus. Cross-encoder rerankers cost tens to hundreds of ms on CPU and their gain on small corpora with short queries is often nil; they are not worth it inside AutoMemory's budget, but a tiny MiniLM reranker could run in a background "deep recall" mode.

### Cited Findings
- Elastic BEIR study (Jul 2023): RRF (k=20, N=1000) +1.4% avg nDCG@10 over ELSER alone and +18% over BM25 alone; linear combination with one weight tuned per dataset on 300 queries +6% over ELSER and +24% over BM25; weighted sum "needs calibration", "optimal weight is model specific", less consistent than RRF; ~40 annotated queries sufficed to beat RRF on ArguAna. — [Elastic hybrid retrieval](https://www.elastic.co/search-labs/blog/improving-information-retrieval-elastic-stack-hybrid)
- WANDS e-commerce benchmark (secondary blog): nDCG BM25 0.698, vector 0.695, untuned RRF 0.707 (+1.2%), tuned fusion 0.7497 (+7.4%). — [techsy.io](https://techsy.io/fr/blog/hybrid-search-bm25-vs-vector)
- Practitioner report: nDCG@10 0.62 with default RRF → 0.70 with min-max normalised weighted blend; "learned fusion beats tuned weighted by 3–6 points" (unsourced). — [dev.to fusion](https://dev.to/gabrielanhaia/hybrid-retrieval-fusion-rrf-vs-weighted-vs-learned-when-each-wins-26i1)
- Reranker quality (Jina vendor table, BEIR nDCG@10): bge-reranker-base 47.89 (278M params), ms-marco-MiniLM-L-6-v2 48.64 (23M params). — [Jina rerankers turbo/tiny](https://jina.ai/en-US/news/smaller-faster-cheaper-jina-rerankers-turbo-and-tiny/)
- Reranker latency: a BGE reranker over a short candidate list had a 197.3 ms median on CPU (tool-routing paper, 2026); blog estimate "+50–200 ms depending on hardware and chunk count"; MiniLM "mildly underperforms BGE reranker" but "reduces latency significantly" (archives RAG paper). — [arXiv 2609.18672](https://arxiv.org/pdf/2609.18672); [dev.to reranker latency](https://dev.to/gabrielanhaia/rerankers-and-the-latency-budget-when-cross-encoders-are-worth-it-3efc); [arXiv 2607.03440](https://arxiv.org/pdf/2607.03440)
- "Small lexical corpora with short queries often don't need reranking"; sweep rerank depth rather than assume. — [dev.to reranker latency](https://dev.to/gabrielanhaia/rerankers-and-the-latency-budget-when-cross-encoders-are-worth-it-3efc)
- fastembed ships bge-reranker-base, bge-reranker-v2-m3, jina-reranker-v1-turbo-en, jina-reranker-v2-base-multilingual as ONNX. — [fastembed README](https://docs.rs/crate/fastembed/latest/source/README.md)

### Inferences
- Because AutoMemory already multiplies BM25 by decay and confidence, the cleanest hybrid is: run FTS5 top-N and vector top-N, fuse by RRF (k=60) into a single relevance, then apply decay × confidence × diversity as today. Weighted blending should wait until a labelled query set (~50 queries per project type) exists; the Elastic data says that is the threshold where tuning beats RRF.
- A cross-encoder at 23M params (MiniLM-L6) over 20 candidates is on the order of 20 × a bge-small forward pass; with no CPU batch-1 number found it is plausibly 50–200 ms, 10–40× the hook budget. Keep it off the hook; optionally offer it in an explicit `recall --deep` path.
- ColBERT-style late interaction was not covered by any measured source found; it needs per-token vectors (≈ 30–100× storage of one vector per note) and is not justified at this scale.

### Gaps
- No measured hybrid-over-BM25 gain on a short-note (< 100 token) corpus; all measurements are BEIR/e-commerce passages. This is the key experiment to run on AutoMemory data.
- No per-pair CPU latency for bge-reranker-base or MiniLM-L6 found.
- No local ColBERT/late-interaction latency or quality data found.

## 4. Query handling without an LLM (FTS5 capabilities)

### Takeaway
FTS5 natively provides unicode61 (case folding, diacritic removal, configurable token chars), a porter stemmer wrapper (English only), a trigram tokenizer (substring match of ≥3 chars, indexed LIKE/GLOB), prefix queries, NEAR, column filters and bm25() with per-column weights. It has no built-in synonyms or spelling correction; synonyms are done through the custom-tokenizer API (FTS5_TOKEN_COLOCATED), and spelling tolerance comes only from prefix/trigram matching or an external table.

### Cited Findings
- Tokenizers: unicode61 (default; `remove_diacritics 0|1|2`, `categories`, `tokenchars`, `separators`), ascii, porter (wraps another tokenizer, "designed for English"), trigram (`case_sensitive`, `remove_diacritics`; queries shorter than 3 chars match nothing; LIKE/GLOB patterns with no 3-char literal run fall back to a linear scan; with `detail=none|column` queries may not contain tokens longer than 3 chars). — [SQLite FTS5](https://www.sqlite.org/fts5.html)
- Query syntax: prefix `thr*` (optionally accelerated with `prefix='2 3'` indexes), phrases, `NEAR(a b, N)` (default N=10), column filters `col : term`, `{a b} : term`, `-col : term`; NOT > AND > OR precedence. — [SQLite FTS5](https://www.sqlite.org/fts5.html)
- bm25(): negative scores, k1=1.2, b=0.75, per-column weights `bm25(ft, 10.0, 5.0)`; `ORDER BY rank` is faster than `ORDER BY bm25(ft)`; default rank function can be changed via `INSERT INTO ft(ft, rank) VALUES('rank','bm25(10.0,5.0)')`. — [SQLite FTS5](https://www.sqlite.org/fts5.html)
- Synonyms (7.1.1): three approaches via custom tokenizer. (1) map all synonyms to one canonical token at both index and query time: no extra space, cheap, breaks prefix queries, "often the best approach"; (2) expand at query time with `FTS5_TOKEN_COLOCATED`: no extra space, more CPU per lookup; (3) emit synonyms at index time: bigger index, prefix queries work. Never both. — [SQLite FTS5](https://www.sqlite.org/fts5.html)
- Storage knobs: `detail=column` drops offsets, `detail=none` drops offsets and columns (smaller index; phrase/NEAR unavailable); `columnsize=0` drops `%_docsize`; contentless and external-content tables avoid duplicating text; merging is incremental by default (`automerge`, `crisismerge`, `usermerge`, `optimize`). — [SQLite FTS5](https://www.sqlite.org/fts5.html)
- Spelling tolerance: not mentioned in FTS5 docs; prefix and trigram are the only built-in fuzziness. — [SQLite FTS5](https://www.sqlite.org/fts5.html)

### Inferences
- The cheapest quality step is `tokenize='porter unicode61 remove_diacritics 2 tokenchars ''-_'''`, which gives stemming, keeps identifiers like `foo_bar` and `t39-phase1` whole, and costs nothing at query time. Changing the tokenizer requires a rebuild of the FTS table (one-off, seconds at 10k notes).
- A second FTS5 table (or shadow column) with the trigram tokenizer gives typo tolerance and substring matching on identifiers; cost is index size (roughly 3–5× the token index for short text, not measured) and is only needed when the porter query returns < k hits (fallback).
- Sentence-to-keyword reduction without an LLM: strip stopwords, keep nouns/identifiers/CamelCase/snake_case tokens, OR the remainder, and AND the top-2 IDF terms; FTS5 supports this as query rewriting with no extension. Query-time synonym expansion is simplest done in Rust by rewriting the MATCH string to `(a OR b)` groups from a small project dictionary, which equals approach 2 without a custom tokenizer.
- SQLite's spellfix1 extension (edit-distance lookups over a vocabulary) was not confirmed in the fetched content; the `fts5vocab` table can feed such a vocabulary if added.

### Gaps
- No measurements of porter vs plain unicode61 recall on short technical notes; no trigram index-size ratio from the docs.
- spellfix1 availability inside rusqlite's bundled SQLite was not verified.

## 5. Chunking and tiered loading for documents

### Takeaway
The best 2026 evidence (36-strategy benchmark, 6 domains, 5 embedders) finds paragraph-group chunks (2 paragraphs, 1 overlap; nDCG@5 ≈ 0.459) and adaptive 50–200-token chunks (≈ 0.441) retrieve best, while tiny fixed character chunks are worst (0.244). Heading-based splitting for Markdown is well supported but mainly by vendor blogs. OpenViking's L0/L1/L2 (≈100-token abstract, ≈2k-token overview, full file) matches AutoMemory's three tiers; its token-saving claims (91–95%) are self-reported.

### Cited Findings
- arXiv 2603.06976 (2026): 36 strategies, UltraDomain six domains, embedders bge-m3, all-MiniLM-L6-v2 and three potion/Model2Vec variants, top-5 dense retrieval, Mixtral-graded. Paragraph Group Chunking best (nDCG@5 ≈ 0.459, P@1 ≈ 24%, Hit@5 ≈ 59%); Dynamic Token Size 50–200 tokens second (≈ 0.441); fixed 100-char/10-overlap worst (0.244, P@1 2–3%); authors favour structure-preserving and adaptive sizes, note very small chunks inflate index size and latency; no heading-specific or short-text results. — [arXiv 2603.06976](https://arxiv.org/html/2603.06976v1)
- Conflicting vendor results: Chroma reported semantic chunking at 91.9% recall; FloTorch answer accuracy put semantic chunking last (54%) vs fixed 512-token (67%), explained by ~43-token fragments retrieving well but giving too little context; Ertas (vendor) Recall@5 71.3% fixed vs 83.2% semantic/document-aware. Vectara NAACL 2025 is cited secondhand as finding fixed-size ≥ semantic and that chunking config matters as much as embedding model choice. — [PremAI 2026 guide](https://blog.premai.io/rag-chunking-strategies-the-2026-benchmark-guide); [Ertas](https://www.ertas.ai/blog/rag-chunking-strategy-benchmark-fixed-semantic); [fast.io](https://fast.io/resources/best-chunking-strategies-for-rag/)
- OpenViking (ByteDance/Volcengine): L0 abstract ≈100 tokens, L1 overview ≈2k tokens, L2 full file unbounded and loaded on demand; per-directory `.abstract.md` and `.overview.md`; tiers generated at write time; intended flow: scan L0 for relevance, L1 for planning, L2 for execution. Claimed 95% token reduction (~550 tokens/retrieval) and 91% on LoCoMo10 are self-reported; licence reported as both AGPL-3.0 and Apache-2.0 by different reviewers (unresolved). — [OpenViking docs](https://www.mintlify.com/volcengine/OpenViking/introduction); [zby review](https://zby.github.io/commonplace/agent-memory-systems/reviews/openviking/); [hermes atlas](https://hermesatlas.com/projects/volcengine/OpenViking)

### Inferences
- For AutoMemory's documents, index at the heading-section level but merge sections under ~50 tokens into their parent and cap at ~200–400 tokens with one-paragraph overlap; this matches the two best strategies in the 2026 benchmark and keeps the three-tier serving model (title line = L0, outline = L1, full = L2).
- Retrieval unit and serving unit can differ: retrieve on chunks, serve the enclosing tier, so recall is high while the LLM sees enough context (the FloTorch failure mode is small retrieved fragments being served as-is).
- Notes themselves (short) should remain one chunk each; chunking only applies to documents.

### Gaps
- No benchmark isolates heading-based Markdown splitting against paragraph grouping with the same embedder.
- No measured optimal granularity for < 100-token notes; all studies use passage-length documents.
- OpenViking tier generation relies on an LLM to write abstracts; no LLM-free tiering evidence found beyond using title/first paragraph/headings.

## 6. Freshness and decay

### Takeaway
Exponential recency decay combined with relevance and an importance/confidence term is the common design (FadeMem, YourMemory, production systems with ~30-day half-lives), but half-life values are unprincipled (72 h to 30 days quoted) and at least one review argues naive decay buries still-true facts; the mitigations are decay by memory type, reinforcement on access, and treating decay as a tie-breaker rather than a multiplier that can zero out an exact match.

### Cited Findings
- FadeMem (arXiv 2601.18642, 2026): scores memories by relevance, access frequency and recency with exponential decay; two layers, slow-decay long-term and fast-decay short-term; reports 45% storage reduction with superior multi-hop retrieval on Session Chat, LoCoMo and LTI-Bench (self-reported). — [FadeMem](https://arxiv.org/html/2601.18642v2)
- YourMemory (MCP server): Ebbinghaus-style decay modulated by importance and recall count; author reports 100% foundational recall over a 50-session benchmark vs 0% for recency-only baselines (self-built benchmark, counts items). — [dev.to memory decay](https://dev.to/sachit_mishra_686a94d1bb5/i-built-memory-decay-for-ai-agents-using-the-ebbinghaus-forgetting-curve-1b0e)
- Half-life defaults: 72 h in a tutorial (explicitly illustrative), per-type half-lives (preferences months, task notes hours); Nowledge describes a ~30-day half-life in production. — [olostep](https://www.olostep.com/blog/memory-retrieval-strategies-ai-agents); [Nowledge](https://nowledge.co/blog/memory-decay-temporal)
- Counter-view: a memory-decay research report notes naive decay can hurt and cites THEANINE's timeline-based approach as arguing against decay; proposes blending similarity with recency via a weight alpha. — [memory decay research](https://publish.obsidian.md/cameron/reports/memory-decay-research)
- AWS Bedrock AgentCore guidance frames decay as one of several strategies alongside summarisation and importance, not a standalone rule. — [AWS agent memory strategies](https://buildwithaws.substack.com/p/agent-memory-strategies-building)

### Inferences
- AutoMemory's multiplicative decay should have a floor (e.g. never below 0.2–0.3) or switch to an additive bonus so a strong lexical/semantic match on an old fact still surfaces; per-kind half-lives (decisions and preferences: long; task/status notes: short) are the cheapest fix and match every source's advice.
- "Last confirmed/accessed" should reset or slow decay (reinforcement), which FadeMem and YourMemory both do and which costs one timestamp column.

### Gaps
- No controlled benchmark of decay curves on agent memory; all gains are self-reported by the system authors. AutoMemory should measure on its own replayed sessions.

## 7. Operational numbers and the < 5 ms hook budget

### Takeaway
Only static embeddings and SQLite brute force fit a 5 ms hook: potion model load 15 ms (so must be resident or mmap'd), per-query embedding ≈ microseconds, int8 scan of 10k × 384 well under 5 ms by arithmetic, FTS5 already 0.56 ms. ONNX transformer inference has no measured CPU batch-1 figure under 5 ms and its runtime cold start alone is likely above budget, so it belongs in background indexing.

### Cited Findings
- model2vec-rs: model load 15 ms, 671K texts/s single thread (≈1.5 µs/text) on M3 Pro. — [model2vec-rs README](https://docs.rs/crate/model2vec-rs/latest/source/README.md)
- sqlite-vec insert 100k × 384 f32 in 1,179 ms (≈12 µs/vector), full scan 67.8 ms/100k; sqlite-vector int8 quantize 225 ms/100k, 37 MB memory. — [Bambini](https://marcobambini.substack.com/p/the-state-of-vector-search-in-sqlite)
- sqlite-vec 10k f32 k=10: 3.55 ms (hardware unstated). — [SimpleVecDB](https://coderdayton.github.io/SimpleVecDB/benchmarks)
- Python model2vec load 312 ms vs Rust 15 ms (same README); Rust startup "12× faster" than sentence-transformers (claim). — [model2vec-rs README](https://docs.rs/crate/model2vec-rs/latest/source/README.md); [hippo note](https://socratic-shell.github.io/hippo/research/rust-sentence-transformers-alternatives-claude-ai.html)
- ORT from-source compile is "extremely slow"; static linking shrinks binary but is discouraged by Microsoft. — [ort docs](https://docs.rs/crate/ort/1.14.1); [ort README](https://github.com/BloopAI/ort)

### Inferences
- Index build for 10k notes with potion: embedding < 1 s, int8 BLOB writes dominated by SQLite (≈ 12 µs/row ⇒ ~0.1 s). With bge-small via ORT on CPU, 10k notes at an assumed 5–40 ms each is 1–7 minutes, acceptable only in a background pass.
- Memory: 10k × 384 i8 = 3.8 MB vectors + potion-8M weights (~8–15 MB) fits comfortably; a transformer plus ORT adds ~100–300 MB resident while running (not measured).
- Hook path: FTS5 (0.56 ms) + static query embedding (µs) + i8 scan over ≤ 10k (≈ 1–2 ms) + RRF leaves headroom under 5 ms; anything else runs out of band.

### Gaps
- No measured ORT cold-start time or resident memory for bge-small in Rust; no Apple-Silicon vs x86 split found for any model.

## Recommended local stack and the measurements to confirm it

### Takeaway
Ship: FTS5 with `porter unicode61` (plus optional trigram fallback table) → Model2Vec potion-retrieval-32M (or potion-base-8M for size) through `model2vec-rs` with the model embedded via `from_bytes` → int8 vectors in a plain BLOB column scanned by Rust brute force → RRF(k=60) → existing decay (with floor and per-kind half-life) × confidence → diversity → token budget. Offer `ort`/fastembed with EmbeddingGemma-300M-Q4 or arctic-embed-xs as an optional feature for background re-embedding and a `--deep` MiniLM rerank. No HNSW below 100k notes.

### Cited Findings
- All components above are cited in sections 1–7; the stack combines the measured cheapest pieces (model2vec-rs 15 ms load / 671K texts/s; sqlite brute force 68 ms per 100k; Elastic +18% nDCG@10 for RRF over BM25; FTS5 porter/trigram/synonym support). — [model2vec-rs](https://docs.rs/crate/model2vec-rs/latest/source/README.md); [Bambini](https://marcobambini.substack.com/p/the-state-of-vector-search-in-sqlite); [Elastic](https://www.elastic.co/search-labs/blog/improving-information-retrieval-elastic-stack-hybrid); [FTS5](https://www.sqlite.org/fts5.html)

### Inferences
Measurements needed before committing (none exist in the literature for this corpus shape):
1. Build a labelled query set from real AutoMemory sessions (≥ 50 queries/project; Elastic's data says ~40 is where tuned fusion starts to beat RRF) and report Recall@5, nDCG@10 for: BM25 unicode61; BM25 porter; BM25 porter + trigram fallback; + potion hybrid (RRF); + bge-small/arctic-xs hybrid; + MiniLM rerank.
2. Latency p50/p99 on M-series and one x86 laptop for: hook path end to end at 500, 5k, 50k notes; potion embed per query; i8 scan per 10k; ORT batch-1 for bge-small and EmbeddingGemma-Q4; ORT cold start.
3. Binary size and build time with: no embedding; model2vec-rs + embedded potion-8M; + `ort` static; + `ort` load-dynamic.
4. Index build time and resident memory at 10k and 100k notes for potion vs transformer.
5. Decay ablation on replayed sessions: current multiplicative decay vs floored vs per-kind half-life, measuring how often a still-true old fact is missed.

### Gaps
- Everything in the measurement list is unmeasured for short-note corpora; the literature numbers above bound the design but do not confirm it.
