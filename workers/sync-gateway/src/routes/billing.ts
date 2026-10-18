import { Hono } from 'hono';
import type { Env } from '../types';
import { verifyLemonSignature, verifyStripeSignature } from '../auth/webhook';

export const billingRouter = new Hono<{ Bindings: Env }>();

/**
 * Universal Billing Webhook Endpoint
 * Supports Stripe & LemonSqueezy webhook events to dynamically upgrade or downgrade
 * users across pricing plans (Free -> Plus -> Team -> Enterprise) in Cloudflare D1.
 */
billingRouter.post('/billing/webhook', async (c) => {
  // The raw text is what the provider signed; verify it before trusting any field.
  const raw = await c.req.text();
  let body: any;
  try {
    body = JSON.parse(raw);
  } catch {
    return c.json({ error: 'BAD_REQUEST', message: 'Body is not valid JSON' }, 400);
  }

  const isStripe = body?.object === 'event' || (typeof body?.type === 'string' && !body?.meta);
  const isLemon = !!body?.meta?.event_name;
  const devSimulation = c.env.ENABLE_DEV_AUTH_BYPASS === 'true' && !isStripe && !isLemon;

  if (isStripe) {
    const secret = c.env.STRIPE_WEBHOOK_SECRET;
    if (!secret) return c.json({ error: 'NOT_CONFIGURED', message: 'Stripe webhook secret is not set' }, 503);
    if (!(await verifyStripeSignature(raw, c.req.header('Stripe-Signature'), secret))) {
      return c.json({ error: 'FORBIDDEN', message: 'Invalid Stripe signature' }, 403);
    }
  } else if (isLemon) {
    const secret = c.env.LEMONSQUEEZY_WEBHOOK_SECRET;
    if (!secret) return c.json({ error: 'NOT_CONFIGURED', message: 'LemonSqueezy webhook secret is not set' }, 503);
    if (!(await verifyLemonSignature(raw, c.req.header('X-Signature'), secret))) {
      return c.json({ error: 'FORBIDDEN', message: 'Invalid LemonSqueezy signature' }, 403);
    }
  } else if (!devSimulation) {
    // The unsigned "direct simulation" payload is a local-testing aid only.
    return c.json({ error: 'FORBIDDEN', message: 'Unsigned webhook payloads are not accepted' }, 403);
  }

  let provider = 'unknown';
  let eventType = 'unknown';
  let uid: string | null = null;
  let targetPlanId: string | null = null;

  // 1. Detect Stripe Webhooks
  if (body.object === 'event' || body.type) {
    provider = 'stripe';
    eventType = body.type || 'unknown';
    const obj = body.data?.object || {};

    if (eventType === 'checkout.session.completed') {
      uid = obj.client_reference_id || obj.metadata?.uid || null;
      targetPlanId = obj.metadata?.plan_id || 'plus';
    } else if (eventType === 'customer.subscription.updated') {
      uid = obj.metadata?.uid || null;
      targetPlanId = obj.metadata?.plan_id || 'plus';
    } else if (eventType === 'customer.subscription.deleted') {
      uid = obj.metadata?.uid || null;
      targetPlanId = 'free'; // Downgrade on cancel
    }
  }
  // 2. Detect LemonSqueezy Webhooks
  else if (body.meta?.event_name) {
    provider = 'lemonsqueezy';
    eventType = body.meta.event_name;
    const customData = body.meta.custom_data || {};

    if (eventType === 'order_created' || eventType === 'subscription_created') {
      uid = customData.uid || null;
      targetPlanId = customData.plan_id || 'plus';
    } else if (eventType === 'subscription_cancelled' || eventType === 'subscription_expired') {
      uid = customData.uid || null;
      targetPlanId = 'free';
    } else if (eventType === 'subscription_updated') {
      uid = customData.uid || null;
      targetPlanId = customData.plan_id || 'plus';
    }
  }

  // 3. Fallback direct simulation payload for testing
  if (!uid && body.uid) {
    provider = 'direct_simulation';
    eventType = body.event || 'plan_change';
    uid = body.uid;
    targetPlanId = body.plan_id || 'plus';
  }

  if (!uid) {
    return c.json(
      {
        received: true,
        message: 'Ignored webhook event without resolvable user uid',
        provider,
        event: eventType,
      },
      200
    );
  }

  const effectivePlan = targetPlanId || 'free';

  // Ensure user exists and update plan in D1
  await c.env.DB.prepare(
    `INSERT INTO users (uid, plan_id, created_at, updated_at)
     VALUES (?1, ?2, datetime('now'), datetime('now'))
     ON CONFLICT(uid) DO UPDATE SET
       plan_id = ?2,
       updated_at = datetime('now')`
  )
    .bind(uid, effectivePlan)
    .run();

  return c.json({
    received: true,
    provider,
    event: eventType,
    uid,
    plan_id: effectivePlan,
    timestamp: new Date().toISOString(),
  });
});
