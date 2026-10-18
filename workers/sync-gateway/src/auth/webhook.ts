/** Webhook authenticity: HMAC-SHA256 signature checks for Stripe and LemonSqueezy. */

const enc = new TextEncoder();

async function hmacHex(secret: string, message: string): Promise<string> {
  const key = await crypto.subtle.importKey('raw', enc.encode(secret), { name: 'HMAC', hash: 'SHA-256' }, false, ['sign']);
  const sig = await crypto.subtle.sign('HMAC', key, enc.encode(message));
  return [...new Uint8Array(sig)].map((b) => b.toString(16).padStart(2, '0')).join('');
}

/** Constant-time string equality (length leak only). */
export function safeEqual(a: string, b: string): boolean {
  if (a.length !== b.length) return false;
  let diff = 0;
  for (let i = 0; i < a.length; i++) diff |= a.charCodeAt(i) ^ b.charCodeAt(i);
  return diff === 0;
}

/**
 * Stripe: header `t=<unix>,v1=<hex>[,v1=...]`, signed payload `${t}.${rawBody}`.
 * Rejects timestamps older than `toleranceSec` to stop replays.
 */
export async function verifyStripeSignature(
  rawBody: string,
  header: string | undefined,
  secret: string,
  nowSec = Math.floor(Date.now() / 1000),
  toleranceSec = 300
): Promise<boolean> {
  if (!header || !secret) return false;
  const parts = header.split(',').map((p) => p.trim().split('=') as [string, string]);
  const t = parts.find(([k]) => k === 't')?.[1];
  const candidates = parts.filter(([k]) => k === 'v1').map(([, v]) => v);
  if (!t || candidates.length === 0 || Math.abs(nowSec - Number(t)) > toleranceSec) return false;
  const expected = await hmacHex(secret, `${t}.${rawBody}`);
  return candidates.some((c) => safeEqual(c, expected));
}

/** LemonSqueezy: `X-Signature` is the hex HMAC-SHA256 of the raw body. */
export async function verifyLemonSignature(rawBody: string, header: string | undefined, secret: string): Promise<boolean> {
  if (!header || !secret) return false;
  return safeEqual(header.trim().toLowerCase(), await hmacHex(secret, rawBody));
}
