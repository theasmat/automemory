import { Hono } from 'hono';
import type { Env, PricingPlan, UserEntitlement } from '../types';

export const adminRouter = new Hono<{ Bindings: Env }>();

// Admin Auth Middleware: verifies X-Admin-Secret header
adminRouter.use('/admin/*', async (c, next) => {
  const secret = c.req.header('X-Admin-Secret');
  if (!secret || secret !== c.env.ADMIN_SECRET) {
    return c.json({ error: 'FORBIDDEN', message: 'Invalid or missing X-Admin-Secret header' }, 403);
  }
  await next();
});


// GET /api/v1/admin/pricing/plans - List all pricing plans
adminRouter.get('/admin/pricing/plans', async (c) => {
  const rows = await c.env.DB.prepare(
    `SELECT * FROM pricing_plans ORDER BY max_active_devices ASC`
  ).all<PricingPlan>();

  return c.json({
    plans: rows.results || [],
    count: rows.results?.length || 0,
  });
});

// POST /api/v1/admin/pricing/plans - Create or dynamically update a pricing plan
adminRouter.post('/admin/pricing/plans', async (c) => {
  const body = await c.req.json<{
    plan_id: string;
    display_name: string;
    max_active_devices: number;
    max_team_members?: number;
    max_storage_bytes?: number;
    allowed_features?: string[];
    is_active?: number;
  }>();

  if (!body.plan_id || !body.display_name || body.max_active_devices == null) {
    return c.json(
      { error: 'BAD_REQUEST', message: 'plan_id, display_name, and max_active_devices are required' },
      400
    );
  }

  const featuresStr = JSON.stringify(body.allowed_features || ['basic_sync']);
  const maxMembers = body.max_team_members ?? 1;
  const maxStorage = body.max_storage_bytes ?? 52428800; // 50MB
  const isActive = body.is_active ?? 1;

  await c.env.DB.prepare(
    `INSERT INTO pricing_plans (plan_id, display_name, max_active_devices, max_team_members, max_storage_bytes, allowed_features, is_active, created_at, updated_at)
     VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, datetime('now'), datetime('now'))
     ON CONFLICT(plan_id) DO UPDATE SET
       display_name = ?2,
       max_active_devices = ?3,
       max_team_members = ?4,
       max_storage_bytes = ?5,
       allowed_features = ?6,
       is_active = ?7,
       updated_at = datetime('now')`
  )
    .bind(body.plan_id, body.display_name, body.max_active_devices, maxMembers, maxStorage, featuresStr, isActive)
    .run();

  return c.json({
    success: true,
    message: `Pricing plan '${body.plan_id}' saved successfully`,
    plan: {
      plan_id: body.plan_id,
      display_name: body.display_name,
      max_active_devices: body.max_active_devices,
      max_team_members: maxMembers,
      max_storage_bytes: maxStorage,
      allowed_features: body.allowed_features || ['basic_sync'],
      is_active: isActive,
    },
  });
});

// POST /api/v1/admin/users/plan - Set a user's subscription plan directly
adminRouter.post('/admin/users/plan', async (c) => {
  const body = await c.req.json<{ uid: string; plan_id: string }>();

  if (!body.uid || !body.plan_id) {
    return c.json({ error: 'BAD_REQUEST', message: 'uid and plan_id are required' }, 400);
  }

  await c.env.DB.prepare(
    `INSERT INTO users (uid, plan_id, created_at, updated_at)
     VALUES (?1, ?2, datetime('now'), datetime('now'))
     ON CONFLICT(uid) DO UPDATE SET plan_id = ?2, updated_at = datetime('now')`
  )
    .bind(body.uid, body.plan_id)
    .run();

  return c.json({
    success: true,
    uid: body.uid,
    plan_id: body.plan_id,
  });
});

// POST /api/v1/admin/users/entitlements - Set custom per-user quota overrides
adminRouter.post('/admin/users/entitlements', async (c) => {
  const body = await c.req.json<{
    uid: string;
    custom_max_devices?: number | null;
    custom_max_storage_bytes?: number | null;
    custom_features?: string[] | null;
    notes?: string | null;
  }>();

  if (!body.uid) {
    return c.json({ error: 'BAD_REQUEST', message: 'uid is required' }, 400);
  }

  // Ensure user exists
  await c.env.DB.prepare(
    `INSERT OR IGNORE INTO users (uid, plan_id, created_at, updated_at)
     VALUES (?1, 'free', datetime('now'), datetime('now'))`
  )
    .bind(body.uid)
    .run();

  const featuresStr = body.custom_features ? JSON.stringify(body.custom_features) : null;

  await c.env.DB.prepare(
    `INSERT INTO user_entitlements (uid, custom_max_devices, custom_max_storage_bytes, custom_features, notes, updated_at)
     VALUES (?1, ?2, ?3, ?4, ?5, datetime('now'))
     ON CONFLICT(uid) DO UPDATE SET
       custom_max_devices = ?2,
       custom_max_storage_bytes = ?3,
       custom_features = ?4,
       notes = ?5,
       updated_at = datetime('now')`
  )
    .bind(
      body.uid,
      body.custom_max_devices != null ? body.custom_max_devices : null,
      body.custom_max_storage_bytes != null ? body.custom_max_storage_bytes : null,
      featuresStr,
      body.notes || null
    )
    .run();

  return c.json({
    success: true,
    message: `Custom quota entitlements configured for user '${body.uid}'`,
    uid: body.uid,
    custom_max_devices: body.custom_max_devices,
    custom_max_storage_bytes: body.custom_max_storage_bytes,
    custom_features: body.custom_features,
    notes: body.notes,
  });
});

// DELETE /api/v1/admin/users/entitlements/:uid - Clear user custom overrides
adminRouter.delete('/admin/users/entitlements/:uid', async (c) => {
  const uid = c.req.param('uid');
  await c.env.DB.prepare(`DELETE FROM user_entitlements WHERE uid = ?1`).bind(uid).run();

  return c.json({
    success: true,
    message: `Custom entitlements cleared for user '${uid}'. Reverted to base plan default.`,
    uid,
  });
});
