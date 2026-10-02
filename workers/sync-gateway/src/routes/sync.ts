import { Hono } from 'hono';
import type { AppEnv, SyncPushPayload, DeviceLease } from '../types';
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


// POST /api/v1/sync/push - Push manifest and deltas to cloud
syncRouter.post('/sync/push', async (c) => {
  const userCtx = c.get('userCtx');
  const body = await c.req.json<any>();

  const projectSlug = body.project_slug || body.project_id;
  if (!projectSlug) {
    return c.json({ error: 'BAD_REQUEST', message: 'project_slug or project_id is required' }, 400);
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

  // Normalize manifest
  const manifest = body.manifest || {
    version: 1,
    project_slug: projectSlug,
    files: (body.documents || []).map((d: any) => ({
      path: d.relative_path || d.path,
      category: d.category || 'general',
      sha256: d.content_hash || d.sha256,
      size_bytes: d.content ? d.content.length : 0,
      compressed_l0: d.l0_abstract,
      compressed_l1: d.l1_overview,
      updated_at: new Date().toISOString(),
    })),
    memories_count: body.memories?.length || 0,
    skills_count: body.skills?.length || 0,
    created_at: body.timestamp || new Date().toISOString(),
  };

  // Normalize deltas
  const deltas = body.deltas || (body.documents || []).map((d: any) => ({
    path: d.relative_path || d.path,
    sha256: d.content_hash || d.sha256,
    content_base64: d.content ? btoa(d.content) : '',
  }));

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

  // Find the exact project id in case of conflict update
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

  // Store deltas in R2 if provided
  let deltasStored = 0;
  if (deltas && deltas.length > 0 && c.env.STORAGE) {
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

  return c.json({
    success: true,
    project_id: activeProjectId,
    project_slug: projectSlug,
    manifest_hash: manifestHash,
    files_tracked: manifest.files.length,
    memories_count: manifest.memories_count,
    skills_count: manifest.skills_count,
    deltas_stored: deltasStored,
    timestamp: new Date().toISOString(),
  });
});


// GET /api/v1/sync/pull - Pull latest project manifest and metadata
syncRouter.get('/sync/pull', async (c) => {
  const userCtx = c.get('userCtx');
  const projectSlug = c.req.query('project');

  if (!projectSlug) {
    return c.json({ error: 'BAD_REQUEST', message: 'project query param required' }, 400);
  }

  const project = await c.env.DB.prepare(
    `SELECT * FROM cloud_projects WHERE owner_uid = ?1 AND slug = ?2`
  )
    .bind(userCtx.uid, projectSlug)
    .first<any>();

  if (!project || !project.current_manifest_hash) {
    return c.json({ error: 'NOT_FOUND', message: 'No manifest found for project' }, 404);
  }

  const manifestRow = await c.env.DB.prepare(
    `SELECT manifest_json, created_at FROM cloud_manifests WHERE hash = ?1`
  )
    .bind(project.current_manifest_hash)
    .first<{ manifest_json: string; created_at: string }>();

  if (!manifestRow) {
    return c.json({ error: 'NOT_FOUND', message: 'Manifest data missing' }, 404);
  }

  return c.json({
    project_id: project.id,
    project_slug: project.slug,
    manifest_hash: project.current_manifest_hash,
    manifest: JSON.parse(manifestRow.manifest_json),
    updated_at: project.updated_at,
  });
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

  // Return restore manifest bundle
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
