import { Hono } from 'hono';
import type { AppEnv } from '../types';
import { verifyFirebaseAuth } from '../auth/firebase';
import { resolveUserContext } from '../auth/quota';

export const vectorRouter = new Hono<AppEnv>();

// Middleware: Authenticate user
vectorRouter.use('/vector/*', async (c, next) => {
  const auth = await verifyFirebaseAuth(c.req.header('Authorization'), c.env);
  if (!auth) {
    return c.json({ error: 'UNAUTHORIZED', message: 'Valid Bearer token required' }, 401);
  }
  const userCtx = await resolveUserContext(c.env.DB, auth.uid, auth.email);
  c.set('userCtx', userCtx);
  await next();
});

// Helper: Cosine similarity calculation
function cosineSimilarity(a: number[], b: number[]): number {
  if (a.length !== b.length || a.length === 0) return 0;
  let dot = 0;
  let normA = 0;
  let normB = 0;
  for (let i = 0; i < a.length; i++) {
    dot += a[i] * b[i];
    normA += a[i] * a[i];
    normB += b[i] * b[i];
  }
  if (normA === 0 || normB === 0) return 0;
  return dot / (Math.sqrt(normA) * Math.sqrt(normB));
}

// POST /api/v1/vector/index - Index anonymous vector coordinates
vectorRouter.post('/vector/index', async (c) => {
  const userCtx = c.get('userCtx');
  const body = await c.req.json<{
    blind_project_id: string;
    items: Array<{ id: string; vector: number[] }>;
  }>();

  if (!body.blind_project_id || !Array.isArray(body.items)) {
    return c.json({ error: 'BAD_REQUEST', message: 'blind_project_id and items array required' }, 400);
  }

  let indexedCount = 0;
  for (const item of body.items) {
    if (item.id && Array.isArray(item.vector)) {
      await c.env.DB.prepare(
        `INSERT INTO blind_vector_index (id, owner_uid, blind_project_id, vector_json, created_at)
         VALUES (?1, ?2, ?3, ?4, datetime('now'))
         ON CONFLICT(id) DO UPDATE SET
           owner_uid = excluded.owner_uid,
           blind_project_id = excluded.blind_project_id,
           vector_json = excluded.vector_json`
      )
        .bind(item.id, userCtx.uid, body.blind_project_id, JSON.stringify(item.vector))
        .run();

      indexedCount++;
    }
  }

  // If Cloudflare Vectorize binding is available, upsert vectors into Vectorize index
  if (c.env.VECTOR_INDEX && typeof c.env.VECTOR_INDEX.upsert === 'function') {
    try {
      const vectorizeVectors = body.items.map((it) => ({
        id: it.id,
        values: it.vector,
        metadata: {
          project: body.blind_project_id,
          owner: userCtx.uid,
        },
      }));
      await c.env.VECTOR_INDEX.upsert(vectorizeVectors);
    } catch (err) {
      console.warn('Vectorize upsert warning:', err);
    }
  }

  return c.json({
    success: true,
    blind_project_id: body.blind_project_id,
    indexed_count: indexedCount,
  });
});

// POST /api/v1/vector/search - Zero-Knowledge Blind Semantic Search
vectorRouter.post('/vector/search', async (c) => {
  const userCtx = c.get('userCtx');
  const body = await c.req.json<{
    vector: number[];
    blind_project_id?: string;
    top_k?: number;
  }>();

  if (!Array.isArray(body.vector) || body.vector.length === 0) {
    return c.json({ error: 'BAD_REQUEST', message: 'Valid float vector array required' }, 400);
  }

  const topK = body.top_k || 5;
  const projectSlug = body.blind_project_id;

  let rankedMatches: Array<{ id: string; score: number }> = [];

  // 1. If Cloudflare Vectorize is available, query HNSW edge index
  if (c.env.VECTOR_INDEX && typeof c.env.VECTOR_INDEX.query === 'function') {
    try {
      const qRes = await c.env.VECTOR_INDEX.query(body.vector, {
        topK,
        filter: projectSlug ? { project: projectSlug, owner: userCtx.uid } : { owner: userCtx.uid },
      });
      if (qRes && Array.isArray(qRes.matches)) {
        rankedMatches = qRes.matches.map((m: any) => ({
          id: m.id,
          score: m.score,
        }));
      }
    } catch (err) {
      console.warn('Vectorize query fallback to D1 index:', err);
    }
  }

  // 2. Fallback / local dev calculation using D1 blind_vector_index
  if (rankedMatches.length === 0) {
    let query = `SELECT id, vector_json FROM blind_vector_index WHERE owner_uid = ?1`;
    const params: any[] = [userCtx.uid];
    if (projectSlug) {
      query += ` AND blind_project_id = ?2`;
      params.push(projectSlug);
    }

    const rows = await c.env.DB.prepare(query)
      .bind(...params)
      .all<{ id: string; vector_json: string }>();

    const scored: Array<{ id: string; score: number }> = [];
    for (const row of rows.results || []) {
      try {
        const storedVec = JSON.parse(row.vector_json);
        const score = cosineSimilarity(body.vector, storedVec);
        scored.push({ id: row.id, score });
      } catch (_) {}
    }

    scored.sort((a, b) => b.score - a.score);
    rankedMatches = scored.slice(0, topK);
  }

  if (rankedMatches.length === 0) {
    return c.json({
      success: true,
      blind_project_id: projectSlug || null,
      count: 0,
      matches: [],
    });
  }

  // 3. Retrieve encrypted ciphertexts and nonces for matching IDs
  const matchIds = rankedMatches.map((m) => m.id);
  const placeholders = matchIds.map(() => '?').join(',');
  const memRows = await c.env.DB.prepare(
    `SELECT id, ciphertext_base64, nonce_base64, blind_scope, blind_type, blind_tags,
            confidence, importance, status, created_at, updated_at
     FROM blind_memories
     WHERE owner_uid = ?1 AND id IN (${placeholders})`
  )
    .bind(userCtx.uid, ...matchIds)
    .all();

  const memMap = new Map((memRows.results || []).map((r: any) => [r.id, r]));

  const results = rankedMatches
    .map((m) => {
      const rec: any = memMap.get(m.id);
      if (!rec) return null;
      let blindTags: string[] = [];
      try {
        blindTags = typeof rec.blind_tags === 'string' ? JSON.parse(rec.blind_tags) : rec.blind_tags || [];
      } catch (_) {}

      return {
        id: rec.id,
        score: Math.round(m.score * 1000) / 1000,
        ciphertext_base64: rec.ciphertext_base64,
        nonce_base64: rec.nonce_base64,
        blind_scope: rec.blind_scope,
        blind_type: rec.blind_type,
        blind_tags: blindTags,
        confidence: rec.confidence,
        importance: rec.importance,
        created_at: rec.created_at,
        updated_at: rec.updated_at,
      };
    })
    .filter(Boolean);

  return c.json({
    success: true,
    blind_project_id: projectSlug || null,
    count: results.length,
    matches: results,
  });
});
