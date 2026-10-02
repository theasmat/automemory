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

-- Seed default configurable pricing plans
INSERT OR IGNORE INTO pricing_plans (plan_id, display_name, max_active_devices, max_team_members, max_storage_bytes, allowed_features, is_active, created_at, updated_at)
VALUES 
    ('free', 'AutoMemory Free', 1, 1, 52428800, '["basic_sync", "local_mcp"]', 1, datetime('now'), datetime('now')),
    ('plus', 'AutoMemory Plus (Multi-Device)', 5, 1, 2147483648, '["basic_sync", "local_mcp", "multi_device", "priority_restore"]', 1, datetime('now'), datetime('now')),
    ('team', 'AutoMemory Team', 50, 10, 53687091200, '["basic_sync", "local_mcp", "multi_device", "team_collaboration", "shared_playbooks", "audit_log"]', 1, datetime('now'), datetime('now')),
    ('enterprise', 'AutoMemory Enterprise', 500, 100, 536870912000, '["all_features", "custom_quotas", "dedicated_r2"]', 1, datetime('now'), datetime('now'));
