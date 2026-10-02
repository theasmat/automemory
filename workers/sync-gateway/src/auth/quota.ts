import type { Env, UserContext, PricingPlan, UserEntitlement, DeviceLease } from '../types';

/**
 * Resolves the effective pricing tier and dynamic quotas for a user.
 * Resolution hierarchy:
 * 1. Direct individual override in `user_entitlements`
 * 2. Organization tier in `organizations` + `pricing_plans` (if member of an org)
 * 3. Individual user tier in `pricing_plans`
 * 4. Fallback to default 'free' plan
 */
export async function resolveUserContext(
  db: D1Database,
  uid: string,
  email?: string
): Promise<UserContext> {
  // Ensure user record exists
  await db
    .prepare(
      `INSERT INTO users (uid, email, plan_id, created_at, updated_at)
       VALUES (?1, ?2, 'free', datetime('now'), datetime('now'))
       ON CONFLICT(uid) DO UPDATE SET updated_at = datetime('now'), email = COALESCE(?2, users.email)`
    )
    .bind(uid, email || null)
    .run();

  // 1. Fetch user record
  const user = await db
    .prepare(`SELECT uid, email, plan_id FROM users WHERE uid = ?1`)
    .bind(uid)
    .first<{ uid: string; email: string | null; plan_id: string }>();

  // 2. Fetch direct user entitlements (custom overrides)
  const entitlement = await db
    .prepare(`SELECT * FROM user_entitlements WHERE uid = ?1`)
    .bind(uid)
    .first<UserEntitlement>();

  // 3. Fetch org membership
  const orgMember = await db
    .prepare(
      `SELECT om.org_id, o.plan_id
       FROM org_members om
       JOIN organizations o ON om.org_id = o.org_id
       WHERE om.uid = ?1
       LIMIT 1`
    )
    .bind(uid)
    .first<{ org_id: string; plan_id: string }>();

  const effectivePlanId = orgMember?.plan_id || user?.plan_id || 'free';

  // 4. Fetch pricing plan defaults
  let plan = await db
    .prepare(`SELECT * FROM pricing_plans WHERE plan_id = ?1 AND is_active = 1`)
    .bind(effectivePlanId)
    .first<PricingPlan>();

  if (!plan) {
    plan = await db
      .prepare(`SELECT * FROM pricing_plans WHERE plan_id = 'free'`)
      .first<PricingPlan>();
  }

  const defaultMaxDevices = plan ? plan.max_active_devices : 1;
  const defaultMaxStorage = plan ? plan.max_storage_bytes : 52428800; // 50MB
  const defaultFeatures: string[] = plan
    ? JSON.parse(plan.allowed_features || '["basic_sync"]')
    : ['basic_sync'];

  // Apply custom entitlement overrides if set by administrator
  const effectiveMaxDevices =
    entitlement?.custom_max_devices != null
      ? entitlement.custom_max_devices
      : defaultMaxDevices;

  const effectiveMaxStorage =
    entitlement?.custom_max_storage_bytes != null
      ? entitlement.custom_max_storage_bytes
      : defaultMaxStorage;

  const customFeatures: string[] = entitlement?.custom_features
    ? JSON.parse(entitlement.custom_features)
    : [];

  const combinedFeatures = Array.from(new Set([...defaultFeatures, ...customFeatures]));

  return {
    uid,
    email: user?.email || undefined,
    plan_id: effectivePlanId,
    org_id: orgMember?.org_id,
    max_active_devices: effectiveMaxDevices,
    max_storage_bytes: effectiveMaxStorage,
    allowed_features: combinedFeatures,
  };
}

export type LeaseCheckResult =
  | { ok: true; lease_id: string; expires_at: string }
  | {
      ok: false;
      error: 'DEVICE_QUOTA_EXCEEDED';
      current_devices: number;
      max_allowed: number;
      upgrade_url: string;
    };

/**
 * Checks or renews a device lease against the user's dynamic quota.
 */
export async function checkOrAcquireDeviceLease(
  db: D1Database,
  uid: string,
  deviceId: string,
  deviceName: string,
  userCtx: UserContext
): Promise<LeaseCheckResult> {
  const now = new Date();
  const expiresAt = new Date(now.getTime() + 30 * 24 * 60 * 60 * 1000).toISOString(); // 30 days

  // Check if device already has a lease
  const existingLease = await db
    .prepare(`SELECT lease_id FROM device_leases WHERE uid = ?1 AND device_id = ?2`)
    .bind(uid, deviceId)
    .first<DeviceLease>();

  if (existingLease) {
    // Refresh lease
    await db
      .prepare(
        `UPDATE device_leases
         SET last_seen_at = ?1, expires_at = ?2, device_name = ?3
         WHERE lease_id = ?4`
      )
      .bind(now.toISOString(), expiresAt, deviceName, existingLease.lease_id)
      .run();

    return { ok: true, lease_id: existingLease.lease_id, expires_at: expiresAt };
  }

  // New device: count active, non-expired device leases
  const countRow = await db
    .prepare(
      `SELECT COUNT(*) as cnt FROM device_leases
       WHERE uid = ?1 AND expires_at > ?2`
    )
    .bind(uid, now.toISOString())
    .first<{ cnt: number }>();

  const currentActiveDevices = countRow ? countRow.cnt : 0;

  if (currentActiveDevices >= userCtx.max_active_devices) {
    return {
      ok: false,
      error: 'DEVICE_QUOTA_EXCEEDED',
      current_devices: currentActiveDevices,
      max_allowed: userCtx.max_active_devices,
      upgrade_url: 'https://automemory.dev/pricing',
    };
  }

  // Create new lease
  const newLeaseId = `lease_${crypto.randomUUID()}`;
  await db
    .prepare(
      `INSERT INTO device_leases (lease_id, uid, device_id, device_name, last_seen_at, expires_at)
       VALUES (?1, ?2, ?3, ?4, ?5, ?6)`
    )
    .bind(newLeaseId, uid, deviceId, deviceName, now.toISOString(), expiresAt)
    .run();

  return { ok: true, lease_id: newLeaseId, expires_at: expiresAt };
}
