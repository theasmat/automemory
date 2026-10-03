-- AutoMemory Pro Cloud Gateway Database Schema (Cloudflare D1 / SQLite)

CREATE TABLE IF NOT EXISTS pricing_plans (
    plan_id TEXT PRIMARY KEY,
    display_name TEXT NOT NULL,
    max_active_devices INTEGER NOT NULL DEFAULT 1,
    max_team_members INTEGER NOT NULL DEFAULT 1,
    max_storage_bytes INTEGER NOT NULL DEFAULT 52428800, -- 50 MB
    allowed_features TEXT NOT NULL DEFAULT '["basic_sync"]',
    is_active INTEGER NOT NULL DEFAULT 1,
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL
);

CREATE TABLE IF NOT EXISTS users (
    uid TEXT PRIMARY KEY,
    email TEXT,
    plan_id TEXT NOT NULL DEFAULT 'free' REFERENCES pricing_plans(plan_id),
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL
);

CREATE TABLE IF NOT EXISTS user_entitlements (
    uid TEXT PRIMARY KEY REFERENCES users(uid) ON DELETE CASCADE,
    custom_max_devices INTEGER,
    custom_max_storage_bytes INTEGER,
    custom_features TEXT,
    notes TEXT,
    updated_at TEXT NOT NULL
);

CREATE TABLE IF NOT EXISTS organizations (
    org_id TEXT PRIMARY KEY,
    name TEXT NOT NULL,
    owner_uid TEXT NOT NULL REFERENCES users(uid),
    plan_id TEXT NOT NULL DEFAULT 'team' REFERENCES pricing_plans(plan_id),
    created_at TEXT NOT NULL
);

CREATE TABLE IF NOT EXISTS org_members (
    org_id TEXT NOT NULL REFERENCES organizations(org_id) ON DELETE CASCADE,
    uid TEXT NOT NULL REFERENCES users(uid) ON DELETE CASCADE,
    role TEXT NOT NULL DEFAULT 'member', -- owner, admin, member
    joined_at TEXT NOT NULL,
    PRIMARY KEY (org_id, uid)
);

CREATE TABLE IF NOT EXISTS device_leases (
    lease_id TEXT PRIMARY KEY,
    uid TEXT NOT NULL REFERENCES users(uid) ON DELETE CASCADE,
    device_id TEXT NOT NULL,
    device_name TEXT NOT NULL,
    last_seen_at TEXT NOT NULL,
    expires_at TEXT NOT NULL,
    UNIQUE (uid, device_id)
);

CREATE TABLE IF NOT EXISTS cloud_projects (
    id TEXT PRIMARY KEY,
    owner_uid TEXT NOT NULL REFERENCES users(uid),
    org_id TEXT REFERENCES organizations(org_id),
    slug TEXT NOT NULL,
    name TEXT NOT NULL,
    current_manifest_hash TEXT,
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL,
    UNIQUE (owner_uid, slug)
);

CREATE TABLE IF NOT EXISTS cloud_manifests (
    hash TEXT PRIMARY KEY,
    project_id TEXT NOT NULL REFERENCES cloud_projects(id) ON DELETE CASCADE,
    manifest_json TEXT NOT NULL,
    created_at TEXT NOT NULL
);

-- Zero-Knowledge Blind Storage Tables
CREATE TABLE IF NOT EXISTS blind_memories (
    id TEXT PRIMARY KEY,
    owner_uid TEXT NOT NULL REFERENCES users(uid) ON DELETE CASCADE,
    blind_project_id TEXT,
    ciphertext_base64 TEXT NOT NULL,
    nonce_base64 TEXT NOT NULL,
    blind_scope TEXT NOT NULL DEFAULT 'project',
    blind_type TEXT,
    blind_tags TEXT DEFAULT '[]',
    confidence REAL DEFAULT 1.0,
    importance REAL DEFAULT 0.8,
    status TEXT NOT NULL DEFAULT 'active',
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL
);

CREATE INDEX IF NOT EXISTS idx_blind_memories_proj ON blind_memories(owner_uid, blind_project_id);
CREATE INDEX IF NOT EXISTS idx_blind_memories_status ON blind_memories(owner_uid, status);

CREATE TABLE IF NOT EXISTS blind_skills (
    id TEXT PRIMARY KEY,
    owner_uid TEXT NOT NULL REFERENCES users(uid) ON DELETE CASCADE,
    blind_project_id TEXT,
    ciphertext_base64 TEXT NOT NULL,
    nonce_base64 TEXT NOT NULL,
    success_count INTEGER DEFAULT 0,
    failure_count INTEGER DEFAULT 0,
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL
);

CREATE INDEX IF NOT EXISTS idx_blind_skills_proj ON blind_skills(owner_uid, blind_project_id);

CREATE TABLE IF NOT EXISTS sync_events (
    id TEXT PRIMARY KEY,
    owner_uid TEXT NOT NULL REFERENCES users(uid) ON DELETE CASCADE,
    project_slug TEXT NOT NULL,
    device_id TEXT,
    device_name TEXT,
    event_type TEXT NOT NULL DEFAULT 'sync_push', -- sync_push, sync_pull, disaster_recovery
    manifest_hash TEXT,
    blind_memories_count INTEGER DEFAULT 0,
    blind_skills_count INTEGER DEFAULT 0,
    deltas_count INTEGER DEFAULT 0,
    created_at TEXT NOT NULL
);

CREATE INDEX IF NOT EXISTS idx_sync_events_owner ON sync_events(owner_uid, created_at DESC);

CREATE TABLE IF NOT EXISTS webhooks (
    id TEXT PRIMARY KEY,
    owner_uid TEXT NOT NULL REFERENCES users(uid) ON DELETE CASCADE,
    url TEXT NOT NULL,
    secret TEXT,
    event_types TEXT NOT NULL DEFAULT '["guardrail.review_proposal", "asan.memory_leak", "skill.promoted"]',
    is_active INTEGER NOT NULL DEFAULT 1,
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL
);

CREATE INDEX IF NOT EXISTS idx_webhooks_owner ON webhooks(owner_uid);

-- Seed default configurable pricing plans
INSERT OR IGNORE INTO pricing_plans (plan_id, display_name, max_active_devices, max_team_members, max_storage_bytes, allowed_features, is_active, created_at, updated_at)
VALUES 
    ('free', 'AutoMemory Free', 1, 1, 52428800, '["basic_sync", "local_mcp"]', 1, datetime('now'), datetime('now')),
    ('plus', 'AutoMemory Plus (Multi-Device)', 5, 1, 2147483648, '["basic_sync", "local_mcp", "multi_device", "priority_restore"]', 1, datetime('now'), datetime('now')),
    ('team', 'AutoMemory Team', 50, 10, 53687091200, '["basic_sync", "local_mcp", "multi_device", "team_collaboration", "shared_playbooks", "audit_log"]', 1, datetime('now'), datetime('now')),
    ('enterprise', 'AutoMemory Enterprise', 500, 100, 536870912000, '["all_features", "custom_quotas", "dedicated_r2"]', 1, datetime('now'), datetime('now'));
