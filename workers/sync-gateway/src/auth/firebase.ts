import { createRemoteJWKSet, jwtVerify } from 'jose';
import type { Env } from '../types';

const GOOGLE_JWKS = createRemoteJWKSet(
  new URL('https://www.googleapis.com/service_accounts/v1/jwk/securetoken@system.gserviceaccount.com')
);

export interface DecodedAuth {
  uid: string;
  email?: string;
  isDevBypass?: boolean;
}

/**
 * Validates a Firebase Auth Bearer token.
 * Supports:
 * 1. Full RS256 JWT validation against Google Firebase Public JWKS.
 * 2. Local dev bypass tokens (e.g. `Bearer dev-token:user_123:user@example.com` or `Bearer dev-user`) when ENABLE_DEV_AUTH_BYPASS is enabled.
 */
export async function verifyFirebaseAuth(
  authHeader: string | undefined,
  env: Env
): Promise<DecodedAuth | null> {
  if (!authHeader || !authHeader.startsWith('Bearer ')) {
    return null;
  }

  const token = authHeader.substring(7).trim();
  if (!token) return null;

  // Local development / testing bypass
  if (env.ENABLE_DEV_AUTH_BYPASS === 'true') {
    if (token.startsWith('dev-token:')) {
      const parts = token.split(':');
      const uid = parts[1] || 'dev_user_default';
      const email = parts[2] || `${uid}@automemory.local`;
      return { uid, email, isDevBypass: true };
    }
    if (token === 'dev-token' || token === 'test-token' || token.startsWith('mock_')) {
      return { uid: 'dev_user_default', email: 'dev@automemory.local', isDevBypass: true };
    }
  }

  try {
    const { payload } = await jwtVerify(token, GOOGLE_JWKS, {
      issuer: `https://securetoken.google.com/${env.FIREBASE_PROJECT_ID}`,
      audience: env.FIREBASE_PROJECT_ID,
    });

    if (!payload.sub) {
      return null;
    }

    return {
      uid: payload.sub,
      email: payload.email as string | undefined,
    };
  } catch (err) {
    console.error('Firebase token verification error:', err);
    return null;
  }
}
