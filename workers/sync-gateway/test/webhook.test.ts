import { describe, expect, it } from 'vitest';
import { safeEqual, verifyLemonSignature, verifyStripeSignature } from '../src/auth/webhook';

async function hmac(secret: string, msg: string) {
  const key = await crypto.subtle.importKey('raw', new TextEncoder().encode(secret), { name: 'HMAC', hash: 'SHA-256' }, false, ['sign']);
  const sig = await crypto.subtle.sign('HMAC', key, new TextEncoder().encode(msg));
  return [...new Uint8Array(sig)].map((b) => b.toString(16).padStart(2, '0')).join('');
}

describe('webhook signatures', () => {
  const body = '{"type":"checkout.session.completed"}';
  it('accepts a valid Stripe signature and rejects tampering, replay and a wrong secret', async () => {
    const now = 1_800_000_000;
    const sig = await hmac('whsec', `${now}.${body}`);
    expect(await verifyStripeSignature(body, `t=${now},v1=${sig}`, 'whsec', now)).toBe(true);
    expect(await verifyStripeSignature(body + ' ', `t=${now},v1=${sig}`, 'whsec', now)).toBe(false);
    expect(await verifyStripeSignature(body, `t=${now},v1=${sig}`, 'other', now)).toBe(false);
    expect(await verifyStripeSignature(body, `t=${now},v1=${sig}`, 'whsec', now + 301)).toBe(false);
    expect(await verifyStripeSignature(body, undefined, 'whsec', now)).toBe(false);
  });
  it('checks LemonSqueezy HMAC and compares in constant time', async () => {
    const sig = await hmac('lemon', body);
    expect(await verifyLemonSignature(body, sig, 'lemon')).toBe(true);
    expect(await verifyLemonSignature(body, sig, 'nope')).toBe(false);
    expect(await verifyLemonSignature(body, undefined, 'lemon')).toBe(false);
    expect(safeEqual('abc', 'abc')).toBe(true);
    expect(safeEqual('abc', 'abd')).toBe(false);
    expect(safeEqual('abc', 'ab')).toBe(false);
  });
});
