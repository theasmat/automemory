import { Hono } from 'hono';
import type { AppEnv, SyncPushPayload, DeviceLease, BlindMemoryRecord, BlindSkillRecord } from '../types';
import { verifyFirebaseAuth } from '../auth/firebase';
import { resolveUserContext } from '../auth/quota';

export const syncRouter = new Hono<AppEnv>();

// Middleware: Authenticate user
syncRouter.use('/sync/*', async (c, next) => {
  const auth = await verifyFirebaseAuth(c.req.header('Authorization'), c.env);
  if (!auth) {
    return c.json({ error: 'UNAUTHORIZED', message: 'Valid Firebase Bearer token required' }, 401);
  }
  const userCtx = await resolveUserContext(c.env.DB, auth.uid, auth.email);
  c.set('userCtx', userCtx);
  await next();
});


// POST /api/v1/sync/push - Push manifest, encrypted deltas, and blind memories/skills
syncRouter.post('/sync/push', async (c) => {
  const userCtx = c.get('userCtx');
  const body = await c.req.json<any>();

  const projectSlug = body.project_slug || body.project_id || body.blind_project_id;
  if (!projectSlug) {
    return c.json({ error: 'BAD_REQUEST', message: 'project_slug, project_id, or blind_project_id is required' }, 400);
  }

  // Device verification: enforce that caller has an active device lease if device_id is passed
  if (body.device_id) {
    const now = new Date().toISOString();
    const lease = await c.env.DB.prepare(
      `SELECT lease_id FROM device_leases WHERE uid = ?1 AND device_id = ?2 AND expires_at > ?3`
    )
      .bind(userCtx.uid, body.device_id, now)
      .first<DeviceLease>();

    if (!lease) {
      return c.json(
        {
          error: 'DEVICE_NOT_REGISTERED',
          message: 'Device lease expired or not registered. Call /api/v1/auth/device/register first.',
        },
        403
      );
    }
  }

  // 1. Process and store Blind Memories if provided
  let blindMemoriesStored = 0;
  if (body.blind_memories && Array.isArray(body.blind_memories)) {
    for (const bm of body.blind_memories) {
      await c.env.DB.prepare(
        `INSERT INTO blind_memories (
           id, owner_uid, blind_project_id, ciphertext_base64, nonce_base64,
           blind_scope, blind_type, blind_tags, confidence, importance, status, created_at, updated_at
         ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13)
         ON CONFLICT(id) DO UPDATE SET
           owner_uid = excluded.owner_uid,
           blind_project_id = excluded.blind_project_id,
           ciphertext_base64 = excluded.ciphertext_base64,
           nonce_base64 = excluded.nonce_base64,
           blind_tags = excluded.blind_tags,
           confidence = excluded.confidence,
           importance = excluded.importance,
           status = excluded.status,
           updated_at = excluded.updated_at`
      )
        .bind(
          bm.id,
          userCtx.uid,
          bm.blind_project_id || projectSlug,
          bm.ciphertext_base64,
          bm.nonce_base64,
          bm.blind_scope || 'project',
          bm.blind_type || null,
          JSON.stringify(bm.blind_tags || []),
          bm.confidence ?? 1.0,
          bm.importance ?? 0.8,
          bm.status || 'active',
          bm.created_at || new Date().toISOString(),
          bm.updated_at || new Date().toISOString()
        )
        .run();
      blindMemoriesStored++;

      // If client supplied anonymous embedding vector, index into blind_vector_index
      if (Array.isArray(bm.embedding) && bm.embedding.length > 0) {
        await c.env.DB.prepare(
          `INSERT INTO blind_vector_index (id, owner_uid, blind_project_id, vector_json, created_at)
           VALUES (?1, ?2, ?3, ?4, datetime('now'))
           ON CONFLICT(id) DO UPDATE SET
             owner_uid = excluded.owner_uid,
             blind_project_id = excluded.blind_project_id,
             vector_json = excluded.vector_json`
        )
          .bind(bm.id, userCtx.uid, bm.blind_project_id || projectSlug, JSON.stringify(bm.embedding))
          .run();

        if (c.env.VECTOR_INDEX && typeof c.env.VECTOR_INDEX.upsert === 'function') {
          try {
            await c.env.VECTOR_INDEX.upsert([
              {
                id: bm.id,
                values: bm.embedding,
                metadata: { project: bm.blind_project_id || projectSlug, owner: userCtx.uid },
              },
            ]);
          } catch (_) {}
        }
      }
    }
  }

  // 2. Process and store Blind Skills if provided
  let blindSkillsStored = 0;
  if (body.blind_skills && Array.isArray(body.blind_skills)) {
    for (const bs of body.blind_skills) {
      await c.env.DB.prepare(
        `INSERT INTO blind_skills (
           id, owner_uid, blind_project_id, ciphertext_base64, nonce_base64,
           success_count, failure_count, created_at, updated_at
         ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)
         ON CONFLICT(id) DO UPDATE SET
           owner_uid = excluded.owner_uid,
           blind_project_id = excluded.blind_project_id,
           ciphertext_base64 = excluded.ciphertext_base64,
           nonce_base64 = excluded.nonce_base64,
           success_count = excluded.success_count,
           failure_count = excluded.failure_count,
           updated_at = excluded.updated_at`
      )
        .bind(
          bs.id,
          userCtx.uid,
          bs.blind_project_id || projectSlug,
          bs.ciphertext_base64,
          bs.nonce_base64,
          bs.success_count ?? 0,
          bs.failure_count ?? 0,
          bs.created_at || new Date().toISOString(),
          bs.updated_at || new Date().toISOString()
        )
        .run();
      blindSkillsStored++;
    }
  }

  // 3. Normalize manifest
  const fileItems = body.encrypted_deltas || body.documents || [];
  const manifest = body.manifest || {
    version: 1,
    project_slug: projectSlug,
    files: fileItems.map((d: any) => ({
      path: d.relative_path || d.path,
      category: d.category || 'general',
      sha256: d.content_hash || d.sha256,
      size_bytes: d.size_bytes || (d.ciphertext_base64 ? d.ciphertext_base64.length : (d.content ? d.content.length : 0)),
      nonce_base64: d.nonce_base64,
      compressed_l0: d.l0_abstract,
      compressed_l1: d.l1_overview,
      updated_at: new Date().toISOString(),
    })),
    memories_count: body.blind_memories?.length || body.memories?.length || 0,
    skills_count: body.blind_skills?.length || body.skills?.length || 0,
    created_at: body.timestamp || new Date().toISOString(),
  };

  const manifestStr = JSON.stringify(manifest);
  // Calculate SHA-256 hash
  const hashBuffer = await crypto.subtle.digest(
    'SHA-256',
    new TextEncoder().encode(manifestStr)
  );
  const hashArray = Array.from(new Uint8Array(hashBuffer));
  const manifestHash = hashArray.map((b) => b.toString(16).padStart(2, '0')).join('');

  // Upsert project
  const projectId = body.project_id || `proj_${crypto.randomUUID()}`;
  const projectName = body.project_name || projectSlug;

  await c.env.DB.prepare(
    `INSERT INTO cloud_projects (id, owner_uid, org_id, slug, name, current_manifest_hash, created_at, updated_at)
     VALUES (?1, ?2, ?3, ?4, ?5, ?6, datetime('now'), datetime('now'))
     ON CONFLICT(owner_uid, slug) DO UPDATE SET
       current_manifest_hash = ?6,
       updated_at = datetime('now')`
  )
    .bind(projectId, userCtx.uid, userCtx.org_id || null, projectSlug, projectName, manifestHash)
    .run();

  const projRow = await c.env.DB.prepare(
    `SELECT id FROM cloud_projects WHERE owner_uid = ?1 AND slug = ?2`
  )
    .bind(userCtx.uid, projectSlug)
    .first<{ id: string }>();

  const activeProjectId = projRow ? projRow.id : projectId;

  // Insert manifest
  await c.env.DB.prepare(
    `INSERT OR IGNORE INTO cloud_manifests (hash, project_id, manifest_json, created_at)
     VALUES (?1, ?2, ?3, datetime('now'))`
  )
    .bind(manifestHash, activeProjectId, manifestStr)
    .run();

  // 4. Store encrypted or plaintext deltas in R2
  let deltasStored = 0;
  if (c.env.STORAGE) {
    // Encrypted deltas (Zero-Knowledge Blind Mode)
    if (body.encrypted_deltas && Array.isArray(body.encrypted_deltas)) {
      for (const ed of body.encrypted_deltas) {
        const key = `blobs/${projectSlug}/${ed.sha256}`;
        const binary = Uint8Array.from(atob(ed.ciphertext_base64), (m) => m.charCodeAt(0));
        await c.env.STORAGE.put(key, binary, {
          customMetadata: {
            path: ed.path,
            nonce_base64: ed.nonce_base64,
            project_slug: projectSlug,
            owner_uid: userCtx.uid,
          },
        });
        deltasStored++;
      }
    }

    // Standard plaintext deltas (Legacy / Non-blind fallback)
    const deltas = body.deltas || (body.documents || []).map((d: any) => ({
      path: d.relative_path || d.path,
      sha256: d.content_hash || d.sha256,
      content_base64: d.content ? btoa(d.content) : '',
    }));

    for (const delta of deltas) {
      if (delta.content_base64) {
        const key = `blobs/${projectSlug}/${delta.sha256}`;
        const binary = Uint8Array.from(atob(delta.content_base64), (m) => m.charCodeAt(0));
        await c.env.STORAGE.put(key, binary, {
          customMetadata: {
            path: delta.path,
            project_slug: projectSlug,
            owner_uid: userCtx.uid,
          },
        });
        deltasStored++;
      }
    }
  }

  // 5. Record sync audit event
  const eventId = `ev_${crypto.randomUUID()}`;
  try {
    await c.env.DB.prepare(
      `INSERT INTO sync_events (
        id, owner_uid, project_slug, device_id, device_name,
        event_type, manifest_hash, blind_memories_count,
        blind_skills_count, deltas_count, created_at
      ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, datetime('now'))`
    )
      .bind(
        eventId,
        userCtx.uid,
        projectSlug,
        body.device_id || null,
        body.device_name || 'Agent Device',
        'sync_push',
        manifestHash,
        blindMemoriesStored,
        blindSkillsStored,
        deltasStored
      )
      .run();
  } catch (err) {
    console.error('Failed to log sync_event:', err);
  }

  // 6. Asynchronously trigger active webhooks if configured
  try {
    const webhooks = await c.env.DB.prepare(
      `SELECT url, secret, event_types FROM webhooks WHERE owner_uid = ?1 AND is_active = 1`
    )
      .bind(userCtx.uid)
      .all<{ url: string; secret: string | null; event_types: string }>();

    if (webhooks.results && webhooks.results.length > 0) {
      const webhookPayload = JSON.stringify({
        event: 'sync.push',
        project_slug: projectSlug,
        device_id: body.device_id,
        manifest_hash: manifestHash,
        blind_memories_count: blindMemoriesStored,
        blind_skills_count: blindSkillsStored,
        deltas_count: deltasStored,
        timestamp: new Date().toISOString(),
      });

      for (const hook of webhooks.results) {
        fetch(hook.url, {
          method: 'POST',
          headers: {
            'Content-Type': 'application/json',
            ...(hook.secret ? { 'X-AutoMemory-Signature': hook.secret } : {}),
          },
          body: webhookPayload,
        }).catch((e) => console.error('Webhook dispatch error:', e));
      }
    }
  } catch (err) {
    console.error('Failed to process webhooks:', err);
  }

  return c.json({
    success: true,
    is_blind: !!(body.blind_memories || body.encrypted_deltas),
    project_id: activeProjectId,
    project_slug: projectSlug,
    manifest_hash: manifestHash,
    files_tracked: manifest.files.length,
    blind_memories_stored: blindMemoriesStored,
    blind_skills_stored: blindSkillsStored,
    deltas_stored: deltasStored,
    timestamp: new Date().toISOString(),
  });
});


// GET /api/v1/sync/pull - Pull latest project manifest, blind memories & skills
syncRouter.get('/sync/pull', async (c) => {
  const userCtx = c.get('userCtx');
  const projectSlug = c.req.query('project') || c.req.query('blind_project_id');

  if (!projectSlug) {
    return c.json({ error: 'BAD_REQUEST', message: 'project or blind_project_id query param required' }, 400);
  }

  const project = await c.env.DB.prepare(
    `SELECT * FROM cloud_projects WHERE owner_uid = ?1 AND slug = ?2`
  )
    .bind(userCtx.uid, projectSlug)
    .first<any>();

  let manifest: any = null;
  let manifestHash: string | null = null;
  if (project && project.current_manifest_hash) {
    manifestHash = project.current_manifest_hash;
    const manifestRow = await c.env.DB.prepare(
      `SELECT manifest_json FROM cloud_manifests WHERE hash = ?1`
    )
      .bind(manifestHash)
      .first<{ manifest_json: string }>();

    if (manifestRow) {
      manifest = JSON.parse(manifestRow.manifest_json);
    }
  }

  // Fetch Blind Memories for this project + global memories
  const rawMem = await c.env.DB.prepare(
    `SELECT id, blind_project_id, ciphertext_base64, nonce_base64, blind_scope, blind_type, blind_tags, confidence, importance, status, created_at, updated_at
     FROM blind_memories
     WHERE owner_uid = ?1 AND (blind_project_id = ?2 OR blind_scope = 'global')
     ORDER BY updated_at DESC LIMIT 5000`
  )
    .bind(userCtx.uid, projectSlug)
    .all<BlindMemoryRecord>();

  console.log(`[SYNC-PULL] uid=${userCtx.uid} slug=${projectSlug} rawMem=`, JSON.stringify(rawMem));
  const blindMemories = rawMem.results || [];

  // Parse blind_tags JSON
  const parsedMemories = (blindMemories || []).map((m: any) => ({
    ...m,
    blind_tags: typeof m.blind_tags === 'string' ? JSON.parse(m.blind_tags) : m.blind_tags,
  }));

  // Fetch Blind Skills
  const rawSkills = await c.env.DB.prepare(
    `SELECT id, blind_project_id, ciphertext_base64, nonce_base64, success_count, failure_count, created_at, updated_at
     FROM blind_skills
     WHERE owner_uid = ?1 AND (blind_project_id = ?2 OR blind_project_id IS NULL)
     ORDER BY updated_at DESC LIMIT 1000`
  )
    .bind(userCtx.uid, projectSlug)
    .all<BlindSkillRecord>();
  const blindSkills = rawSkills.results || [];

  return c.json({
    success: true,
    project_id: project ? project.id : null,
    project_slug: projectSlug,
    manifest_hash: manifestHash,
    manifest,
    blind_memories: parsedMemories,
    blind_skills: blindSkills || [],
    updated_at: project ? project.updated_at : new Date().toISOString(),
  });
});

// GET /api/v1/sync/blob/:project/:sha256 - Stream encrypted document / plan chunk from R2
syncRouter.get('/sync/blob/:project/:sha256', async (c) => {
  const userCtx = c.get('userCtx');
  const projectSlug = c.req.param('project');
  const sha256 = c.req.param('sha256');

  const key = `blobs/${projectSlug}/${sha256}`;
  const obj = await c.env.STORAGE?.get(key);
  if (!obj) {
    return c.json({ error: 'NOT_FOUND', message: 'Encrypted chunk not found in R2' }, 404);
  }

  const headers = new Headers();
  obj.writeHttpMetadata(headers);
  headers.set('etag', obj.httpEtag);
  headers.set('Content-Type', 'application/octet-stream');
  if (obj.customMetadata?.nonce_base64) {
    headers.set('X-Nonce-Base64', obj.customMetadata.nonce_base64);
  }
  if (obj.customMetadata?.path) {
    headers.set('X-Path', obj.customMetadata.path);
  }

  return new Response(obj.body, { headers });
});

// GET /api/v1/sync/restore/:project_id - Disaster recovery restore endpoint
syncRouter.get('/sync/restore/:project_id', async (c) => {
  const userCtx = c.get('userCtx');
  const projectIdOrSlug = c.req.param('project_id');

  const project = await c.env.DB.prepare(
    `SELECT * FROM cloud_projects
     WHERE (id = ?1 OR slug = ?1) AND (owner_uid = ?2 OR org_id = ?3)`
  )
    .bind(projectIdOrSlug, userCtx.uid, userCtx.org_id || '')
    .first<any>();

  if (!project) {
    return c.json({ error: 'NOT_FOUND', message: 'Project not found' }, 404);
  }

  const manifestRow = await c.env.DB.prepare(
    `SELECT manifest_json, created_at FROM cloud_manifests WHERE hash = ?1`
  )
    .bind(project.current_manifest_hash)
    .first<{ manifest_json: string; created_at: string }>();

  if (!manifestRow) {
    return c.json({ error: 'NOT_FOUND', message: 'Manifest data missing' }, 404);
  }

  const manifest = JSON.parse(manifestRow.manifest_json);

  return c.json({
    project: {
      id: project.id,
      slug: project.slug,
      name: project.name,
    },
    manifest_hash: project.current_manifest_hash,
    manifest,
    restore_ready: true,
  });
});
