import { Hono } from 'hono';
import type { AppEnv, DeviceLease } from '../types';
import { verifyFirebaseAuth } from '../auth/firebase';
import { resolveUserContext, checkOrAcquireDeviceLease } from '../auth/quota';

export const devicesRouter = new Hono<AppEnv>();

// Middleware: Authenticate user
const userAuthMiddleware = async (c: any, next: any) => {
  const auth = await verifyFirebaseAuth(c.req.header('Authorization'), c.env);
  if (!auth) {
    return c.json({ error: 'UNAUTHORIZED', message: 'Valid Firebase Bearer token required' }, 401);
  }
  const userCtx = await resolveUserContext(c.env.DB, auth.uid, auth.email);
  c.set('userCtx', userCtx);
  await next();
};

devicesRouter.use('/auth/device/*', userAuthMiddleware);
devicesRouter.use('/user/quota', userAuthMiddleware);


// GET /api/v1/user/quota - Get dynamic quota info and current plan
devicesRouter.get('/user/quota', async (c) => {
  const userCtx = c.get('userCtx');
  const now = new Date().toISOString();

  const countRow = await c.env.DB.prepare(
    `SELECT COUNT(*) as cnt FROM device_leases WHERE uid = ?1 AND expires_at > ?2`
  )
    .bind(userCtx.uid, now)
    .first<{ cnt: number }>();

  const activeDevices = countRow ? countRow.cnt : 0;

  return c.json({
    uid: userCtx.uid,
    email: userCtx.email,
    plan_id: userCtx.plan_id,
    org_id: userCtx.org_id,
    max_active_devices: userCtx.max_active_devices,
    active_devices_count: activeDevices,
    max_storage_bytes: userCtx.max_storage_bytes,
    allowed_features: userCtx.allowed_features,
  });
});

// POST /api/v1/auth/device/register - Register or renew a device lease
devicesRouter.post('/auth/device/register', async (c) => {
  const userCtx = c.get('userCtx');
  const body = await c.req.json<{ device_id?: string; device_name?: string }>();

  const deviceId = body.device_id || `dev_${crypto.randomUUID()}`;
  const deviceName = body.device_name || 'Agent Device';

  const result = await checkOrAcquireDeviceLease(
    c.env.DB,
    userCtx.uid,
    deviceId,
    deviceName,
    userCtx
  );

  if (!result.ok) {
    return c.json(result, 403);
  }

  return c.json({
    success: true,
    device_id: deviceId,
    device_name: deviceName,
    lease_id: result.lease_id,
    expires_at: result.expires_at,
    plan_id: userCtx.plan_id,
    max_allowed_devices: userCtx.max_active_devices,
  });
});

// GET /api/v1/auth/device/list - List all active device leases for user
devicesRouter.get('/auth/device/list', async (c) => {
  const userCtx = c.get('userCtx');
  const now = new Date().toISOString();

  const rows = await c.env.DB.prepare(
    `SELECT lease_id, device_id, device_name, last_seen_at, expires_at
     FROM device_leases
     WHERE uid = ?1
     ORDER BY last_seen_at DESC`
  )
    .bind(userCtx.uid)
    .all<DeviceLease>();

  const devices = (rows.results || []).map((d) => ({
    ...d,
    is_active: d.expires_at > now,
  }));

  return c.json({
    devices,
    total: devices.length,
    active: devices.filter((d) => d.is_active).length,
    max_allowed: userCtx.max_active_devices,
  });
});

// DELETE /api/v1/auth/device/:device_id - Revoke device lease
devicesRouter.delete('/auth/device/:device_id', async (c) => {
  const userCtx = c.get('userCtx');
  const deviceId = c.req.param('device_id');

  const result = await c.env.DB.prepare(
    `DELETE FROM device_leases WHERE uid = ?1 AND device_id = ?2`
  )
    .bind(userCtx.uid, deviceId)
    .run();

  return c.json({
    success: true,
    device_id: deviceId,
    deleted: result.meta?.changes ?? 1,
  });
});
