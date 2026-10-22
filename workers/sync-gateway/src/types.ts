export interface Env {
  DB: D1Database;
  STORAGE: R2Bucket;
  FIREBASE_PROJECT_ID: string;
  ADMIN_SECRET: string;
  ENABLE_DEV_AUTH_BYPASS?: string;
  ENVIRONMENT?: string;
  STRIPE_WEBHOOK_SECRET?: string;
  LEMONSQUEEZY_WEBHOOK_SECRET?: string;
  VECTOR_INDEX?: any;
}

export type AppEnv = {
  Bindings: Env;
  Variables: {
    userCtx: UserContext;
  };
};

export interface UserContext {
  uid: string;
  email?: string;
  plan_id: string;
  org_id?: string;
  max_active_devices: number;
  max_storage_bytes: number;
  allowed_features: string[];
}

export interface PricingPlan {
  plan_id: string;
  display_name: string;
  max_active_devices: number;
  max_team_members: number;
  max_storage_bytes: number;
  allowed_features: string;
  is_active: number;
  created_at: string;
  updated_at: string;
}

export interface UserEntitlement {
  uid: string;
  custom_max_devices: number | null;
  custom_max_storage_bytes: number | null;
  custom_features: string | null;
  notes: string | null;
  updated_at: string;
}

export interface DeviceLease {
  lease_id: string;
  uid: string;
  device_id: string;
  device_name: string;
  last_seen_at: string;
  expires_at: string;
}

export interface BlindMemoryRecord {
  id: string;
  blind_project_id?: string;
  ciphertext_base64: string;
  nonce_base64: string;
  blind_scope: string; // "global" | "project"
  blind_type?: string;
  blind_tags?: string[];
  embedding?: number[];
  confidence?: number;
  importance?: number;
  status?: string; // "active" | "archived"
  created_at: string;
  updated_at: string;
}

export interface BlindSkillRecord {
  id: string;
  blind_project_id?: string;
  ciphertext_base64: string;
  nonce_base64: string;
  success_count?: number;
  failure_count?: number;
  created_at: string;
  updated_at: string;
}

export interface EncryptedDelta {
  path: string;
  sha256: string;
  ciphertext_base64: string;
  nonce_base64: string;
  size_bytes?: number;
}

export interface SyncPushPayload {
  project_id?: string;
  project_slug: string;
  project_name?: string;
  device_id: string;
  is_blind?: boolean;
  manifest?: {
    version: number;
    project_slug: string;
    files: Array<{
      path: string;
      category: string;
      sha256: string;
      size_bytes: number;
      compressed_l0?: string;
      compressed_l1?: string;
      updated_at: string;
    }>;
    memories_count: number;
    skills_count: number;
    created_at: string;
  };
  deltas?: Array<{
    path: string;
    sha256: string;
    content_base64: string;
  }>;
  blind_memories?: BlindMemoryRecord[];
  blind_skills?: BlindSkillRecord[];
  encrypted_deltas?: EncryptedDelta[];
}

