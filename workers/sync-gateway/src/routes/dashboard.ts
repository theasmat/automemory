import { Hono } from 'hono';
import type { AppEnv } from '../types';
import { verifyFirebaseAuth } from '../auth/firebase';
import { resolveUserContext } from '../auth/quota';

export const dashboardRouter = new Hono<AppEnv>();

// Middleware: Authenticate user for API routes
const userAuthMiddleware = async (c: any, next: any) => {
  const auth = await verifyFirebaseAuth(c.req.header('Authorization'), c.env);
  if (!auth) {
    return c.json({ error: 'UNAUTHORIZED', message: 'Valid Bearer token required' }, 401);
  }
  const userCtx = await resolveUserContext(c.env.DB, auth.uid, auth.email);
  c.set('userCtx', userCtx);
  await next();
};

dashboardRouter.use('/api/v1/dashboard/*', userAuthMiddleware);

// --- API Endpoints ---

// GET /api/v1/dashboard/stats - Summary counts and quotas
dashboardRouter.get('/api/v1/dashboard/stats', async (c) => {
  const userCtx = c.get('userCtx');
  const now = new Date().toISOString();

  // Active devices
  const devRow = await c.env.DB.prepare(
    `SELECT COUNT(*) as cnt FROM device_leases WHERE uid = ?1 AND expires_at > ?2`
  )
    .bind(userCtx.uid, now)
    .first<{ cnt: number }>();
  const activeDevices = devRow ? devRow.cnt : 0;

  // Projects count
  const projRow = await c.env.DB.prepare(
    `SELECT COUNT(*) as cnt FROM cloud_projects WHERE owner_uid = ?1`
  )
    .bind(userCtx.uid)
    .first<{ cnt: number }>();
  const totalProjects = projRow ? projRow.cnt : 0;

  // Blind memories count
  const memRow = await c.env.DB.prepare(
    `SELECT COUNT(*) as cnt FROM blind_memories WHERE owner_uid = ?1`
  )
    .bind(userCtx.uid)
    .first<{ cnt: number }>();
  const totalBlindMemories = memRow ? memRow.cnt : 0;

  // Blind skills count
  const skillRow = await c.env.DB.prepare(
    `SELECT COUNT(*) as cnt FROM blind_skills WHERE owner_uid = ?1`
  )
    .bind(userCtx.uid)
    .first<{ cnt: number }>();
  const totalBlindSkills = skillRow ? skillRow.cnt : 0;

  // Sync events count
  const eventRow = await c.env.DB.prepare(
    `SELECT COUNT(*) as cnt FROM sync_events WHERE owner_uid = ?1`
  )
    .bind(userCtx.uid)
    .first<{ cnt: number }>();
  const totalSyncEvents = eventRow ? eventRow.cnt : 0;

  return c.json({
    user: {
      uid: userCtx.uid,
      email: userCtx.email,
      plan_id: userCtx.plan_id,
      org_id: userCtx.org_id,
    },
    quota: {
      max_active_devices: userCtx.max_active_devices,
      active_devices_count: activeDevices,
      max_storage_bytes: userCtx.max_storage_bytes,
      allowed_features: userCtx.allowed_features,
    },
    metrics: {
      total_projects: totalProjects,
      blind_memories_count: totalBlindMemories,
      blind_skills_count: totalBlindSkills,
      sync_events_count: totalSyncEvents,
    },
  });
});

// GET /api/v1/dashboard/timeline - Recent 50 sync events
dashboardRouter.get('/api/v1/dashboard/timeline', async (c) => {
  const userCtx = c.get('userCtx');

  const rows = await c.env.DB.prepare(
    `SELECT id, project_slug, device_id, device_name, event_type, manifest_hash,
            blind_memories_count, blind_skills_count, deltas_count, created_at
     FROM sync_events
     WHERE owner_uid = ?1
     ORDER BY created_at DESC
     LIMIT 50`
  )
    .bind(userCtx.uid)
    .all();

  return c.json({
    events: rows.results || [],
    count: (rows.results || []).length,
  });
});

// GET /api/v1/dashboard/webhooks - List active webhooks
dashboardRouter.get('/api/v1/dashboard/webhooks', async (c) => {
  const userCtx = c.get('userCtx');

  const rows = await c.env.DB.prepare(
    `SELECT id, url, secret, event_types, is_active, created_at, updated_at
     FROM webhooks
     WHERE owner_uid = ?1
     ORDER BY created_at DESC`
  )
    .bind(userCtx.uid)
    .all();

  return c.json({
    webhooks: rows.results || [],
  });
});

// POST /api/v1/dashboard/webhooks - Register new webhook
dashboardRouter.post('/api/v1/dashboard/webhooks', async (c) => {
  const userCtx = c.get('userCtx');
  const body = await c.req.json<{
    url: string;
    secret?: string;
    event_types?: string[];
  }>();

  if (!body.url || !body.url.startsWith('http')) {
    return c.json({ error: 'BAD_REQUEST', message: 'Valid http/https webhook URL required' }, 400);
  }

  const id = `wh_${crypto.randomUUID()}`;
  const eventTypes = JSON.stringify(
    body.event_types || ['sync.push', 'guardrail.review_proposal', 'asan.memory_leak']
  );
  const secret = body.secret || `whsec_${crypto.randomUUID().replace(/-/g, '')}`;

  await c.env.DB.prepare(
    `INSERT INTO webhooks (id, owner_uid, url, secret, event_types, is_active, created_at, updated_at)
     VALUES (?1, ?2, ?3, ?4, ?5, 1, datetime('now'), datetime('now'))`
  )
    .bind(id, userCtx.uid, body.url, secret, eventTypes)
    .run();

  return c.json({
    success: true,
    webhook: {
      id,
      url: body.url,
      secret,
      event_types: JSON.parse(eventTypes),
      is_active: 1,
    },
  });
});

// DELETE /api/v1/dashboard/webhooks/:id - Delete webhook
dashboardRouter.delete('/api/v1/dashboard/webhooks/:id', async (c) => {
  const userCtx = c.get('userCtx');
  const hookId = c.req.param('id');

  const result = await c.env.DB.prepare(
    `DELETE FROM webhooks WHERE id = ?1 AND owner_uid = ?2`
  )
    .bind(hookId, userCtx.uid)
    .run();

  return c.json({
    success: true,
    deleted: result.meta?.changes ?? 1,
  });
});

// POST /api/v1/dashboard/webhooks/test - Dispatch test ping to webhook URL
dashboardRouter.post('/api/v1/dashboard/webhooks/test', async (c) => {
  const userCtx = c.get('userCtx');
  const body = await c.req.json<{ url: string; secret?: string }>();

  if (!body.url) {
    return c.json({ error: 'BAD_REQUEST', message: 'Target URL is required' }, 400);
  }

  const testPayload = JSON.stringify({
    event: 'guardrail.test_ping',
    source: 'automemory-sync-gateway',
    timestamp: new Date().toISOString(),
    user_uid: userCtx.uid,
    message: 'AutoMemory Pro Webhook Connectivity Verification Test',
  });

  const startTime = Date.now();
  try {
    const res = await fetch(body.url, {
      method: 'POST',
      headers: {
        'Content-Type': 'application/json',
        ...(body.secret ? { 'X-AutoMemory-Signature': body.secret } : {}),
      },
      body: testPayload,
    });
    const latencyMs = Date.now() - startTime;
    return c.json({
      success: res.ok,
      status: res.status,
      latency_ms: latencyMs,
      status_text: res.statusText,
    });
  } catch (err: any) {
    const latencyMs = Date.now() - startTime;
    return c.json({
      success: false,
      error: err.message || 'Connection failed',
      latency_ms: latencyMs,
    }, 502);
  }
});

// POST /api/v1/dashboard/upgrade-simulation - Simulate tier change for testing
dashboardRouter.post('/api/v1/dashboard/upgrade-simulation', async (c) => {
  const userCtx = c.get('userCtx');
  const body = await c.req.json<{ plan_id: string }>();

  const allowedPlans = ['free', 'plus', 'team', 'enterprise'];
  if (!allowedPlans.includes(body.plan_id)) {
    return c.json({ error: 'BAD_REQUEST', message: `Invalid plan. Allowed: ${allowedPlans.join(', ')}` }, 400);
  }

  await c.env.DB.prepare(
    `INSERT INTO users (uid, plan_id, created_at, updated_at)
     VALUES (?1, ?2, datetime('now'), datetime('now'))
     ON CONFLICT(uid) DO UPDATE SET
       plan_id = ?2,
       updated_at = datetime('now')`
  )
    .bind(userCtx.uid, body.plan_id)
    .run();

  return c.json({
    success: true,
    previous_plan: userCtx.plan_id,
    new_plan: body.plan_id,
    timestamp: new Date().toISOString(),
  });
});

// --- Web Console HTML UI Endpoint ---

dashboardRouter.get('/dashboard', (c) => {
  const html = `<!DOCTYPE html>
<html lang="en" class="h-full bg-slate-950 text-slate-100">
<head>
  <meta charset="UTF-8">
  <meta name="viewport" content="width=device-width, initial-scale=1.0">
  <title>AutoMemory Pro - Edge Control Console</title>
  <script src="https://cdn.tailwindcss.com"></script>
  <script>
    tailwind.config = {
      darkMode: 'class',
      theme: {
        extend: {
          colors: {
            brand: {
              50: '#eef2ff',
              500: '#6366f1',
              600: '#4f46e5',
              700: '#4338ca',
            }
          }
        }
      }
    }
  </script>
  <style>
    [x-cloak] { display: none !important; }
    .glass {
      background: rgba(15, 23, 42, 0.75);
      backdrop-filter: blur(12px);
      border: 1px solid rgba(255, 255, 255, 0.08);
    }
  </style>
</head>
<body class="min-h-full flex flex-col font-sans antialiased text-sm">
  <!-- Navbar -->
  <header class="glass sticky top-0 z-50 border-b border-slate-800/80 px-6 py-3.5 flex items-center justify-between">
    <div class="flex items-center space-x-3">
      <div class="h-8 w-8 rounded-lg bg-indigo-600 flex items-center justify-center font-bold text-white shadow-lg shadow-indigo-500/20">
        Ω
      </div>
      <div>
        <h1 class="font-semibold text-base tracking-tight text-white flex items-center gap-2">
          AutoMemory <span class="text-xs px-2 py-0.5 rounded-full bg-indigo-500/20 text-indigo-400 font-mono border border-indigo-500/30">PRO EDGE</span>
        </h1>
      </div>
    </div>

    <!-- Auth token input bar -->
    <div class="flex items-center space-x-3">
      <div class="flex items-center bg-slate-900 border border-slate-800 rounded-lg px-3 py-1.5 text-xs text-slate-300">
        <span class="text-slate-500 mr-2 font-mono">AUTH:</span>
        <input id="tokenInput" type="text" placeholder="dev-token:uid:email or Bearer..." 
          class="bg-transparent border-0 outline-none w-64 text-slate-200 font-mono placeholder-slate-600 text-xs">
        <button id="saveTokenBtn" class="ml-2 text-indigo-400 hover:text-indigo-300 font-medium">Connect</button>
      </div>
      <div id="planBadge" class="px-2.5 py-1 rounded-md text-xs font-medium bg-slate-800 text-slate-400 border border-slate-700 font-mono">
        Connecting...
      </div>
    </div>
  </header>

  <!-- Main Container -->
  <main class="flex-1 max-w-7xl w-full mx-auto p-6 space-y-6">
    <!-- Zero Knowledge Security Assurance Banner -->
    <div class="glass rounded-xl p-4 flex items-start space-x-4 border-indigo-900/40 bg-gradient-to-r from-indigo-950/40 via-slate-900/50 to-slate-900/50">
      <div class="p-2 rounded-lg bg-indigo-500/10 text-indigo-400 mt-0.5">
        <svg class="w-5 h-5" fill="none" stroke="currentColor" viewBox="0 0 24 24">
          <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M12 15v2m-6 4h12a2 2 0 002-2v-6a2 2 0 00-2-2H6a2 2 0 00-2 2v6a2 2 0 002 2zm10-10V7a4 4 0 00-8 0v4h8z"></path>
        </svg>
      </div>
      <div class="flex-1">
        <h3 class="text-sm font-semibold text-white flex items-center gap-2">
          Zero-Knowledge Blind Architecture Verified
          <span class="inline-flex items-center px-2 py-0.5 rounded text-[10px] font-medium bg-emerald-500/10 text-emerald-400 border border-emerald-500/20">Active</span>
        </h3>
        <p class="text-xs text-slate-400 mt-0.5 leading-relaxed">
          Memory contents, skills, and plans are stored as client-encrypted <code class="text-indigo-300 bg-slate-800/80 px-1 py-0.5 rounded">AES-256-GCM</code> ciphertext with blind HMAC index tokens. The Cloudflare Edge server possesses zero decryption keys and cannot inspect plaintext data.
        </p>
      </div>
    </div>

    <!-- Navigation Tabs -->
    <nav class="flex space-x-2 border-b border-slate-800 pb-2">
      <button class="tab-btn px-4 py-2 rounded-lg text-xs font-medium text-white bg-slate-800 border border-slate-700" data-tab="overview">Overview</button>
      <button class="tab-btn px-4 py-2 rounded-lg text-xs font-medium text-slate-400 hover:text-white" data-tab="devices">Device Fleet</button>
      <button class="tab-btn px-4 py-2 rounded-lg text-xs font-medium text-slate-400 hover:text-white" data-tab="plans">Quotas & Tiers</button>
      <button class="tab-btn px-4 py-2 rounded-lg text-xs font-medium text-slate-400 hover:text-white" data-tab="timeline">Sync Timeline</button>
      <button class="tab-btn px-4 py-2 rounded-lg text-xs font-medium text-slate-400 hover:text-white" data-tab="webhooks">Webhooks</button>
    </nav>

    <!-- TAB 1: OVERVIEW -->
    <section id="tab-overview" class="space-y-6">
      <div class="grid grid-cols-1 md:grid-cols-4 gap-4">
        <div class="glass rounded-xl p-5">
          <p class="text-xs text-slate-400 font-medium">Active Devices</p>
          <div class="mt-2 flex items-baseline gap-2">
            <span id="metricDevices" class="text-2xl font-bold text-white tracking-tight">0</span>
            <span id="metricMaxDevices" class="text-xs text-slate-500 font-mono">/ 1 allowed</span>
          </div>
          <p class="text-[11px] text-slate-500 mt-2">Active leases in Cloudflare D1</p>
        </div>

        <div class="glass rounded-xl p-5">
          <p class="text-xs text-slate-400 font-medium">Blind Memories</p>
          <div class="mt-2 flex items-baseline gap-2">
            <span id="metricMemories" class="text-2xl font-bold text-indigo-400 tracking-tight">0</span>
            <span class="text-xs text-slate-500 font-mono">encrypted</span>
          </div>
          <p class="text-[11px] text-slate-500 mt-2">Zero-knowledge AES ciphertexts</p>
        </div>

        <div class="glass rounded-xl p-5">
          <p class="text-xs text-slate-400 font-medium">Blind Skills & Rules</p>
          <div class="mt-2 flex items-baseline gap-2">
            <span id="metricSkills" class="text-2xl font-bold text-emerald-400 tracking-tight">0</span>
            <span class="text-xs text-slate-500 font-mono">playbooks</span>
          </div>
          <p class="text-[11px] text-slate-500 mt-2">Encrypted agent skills & recipes</p>
        </div>

        <div class="glass rounded-xl p-5">
          <p class="text-xs text-slate-400 font-medium">Active Projects</p>
          <div class="mt-2 flex items-baseline gap-2">
            <span id="metricProjects" class="text-2xl font-bold text-purple-400 tracking-tight">0</span>
            <span class="text-xs text-slate-500 font-mono">vaults</span>
          </div>
          <p class="text-[11px] text-slate-500 mt-2">Registered repos & plans</p>
        </div>
      </div>

      <!-- Quick status & features -->
      <div class="grid grid-cols-1 md:grid-cols-2 gap-4">
        <div class="glass rounded-xl p-5 space-y-3">
          <h4 class="text-sm font-semibold text-white">Storage Quota Consumption</h4>
          <div class="w-full bg-slate-800 rounded-full h-2.5 overflow-hidden">
            <div id="storageProgressBar" class="bg-indigo-500 h-2.5 rounded-full" style="width: 5%"></div>
          </div>
          <div class="flex justify-between text-xs text-slate-400 font-mono">
            <span id="storageUsedText">0 MB used</span>
            <span id="storageMaxText">50 MB Quota</span>
          </div>
        </div>

        <div class="glass rounded-xl p-5 space-y-3">
          <h4 class="text-sm font-semibold text-white">Edge Tier Features</h4>
          <div id="featureTags" class="flex flex-wrap gap-2">
            <!-- Rendered by JS -->
          </div>
        </div>
      </div>
    </section>

    <!-- TAB 2: DEVICE FLEET -->
    <section id="tab-devices" class="space-y-4 hidden">
      <div class="flex items-center justify-between">
        <div>
          <h3 class="text-base font-semibold text-white">Device Fleet Manager</h3>
          <p class="text-xs text-slate-400 mt-0.5">Manage and revoke agent machines connected to your zero-knowledge vault.</p>
        </div>
        <button id="refreshDevicesBtn" class="px-3 py-1.5 rounded-lg bg-slate-800 hover:bg-slate-700 text-slate-300 text-xs font-medium border border-slate-700">
          Refresh Fleet
        </button>
      </div>

      <div class="glass rounded-xl overflow-hidden">
        <table class="w-full text-left border-collapse">
          <thead>
            <tr class="border-b border-slate-800 bg-slate-900/60 text-xs text-slate-400 font-mono">
              <th class="p-3.5 pl-5">Device Name</th>
              <th class="p-3.5">Device ID</th>
              <th class="p-3.5">Last Seen</th>
              <th class="p-3.5">Expires</th>
              <th class="p-3.5">Status</th>
              <th class="p-3.5 pr-5 text-right">Actions</th>
            </tr>
          </thead>
          <tbody id="deviceTableBody" class="divide-y divide-slate-800/60 text-xs">
            <tr><td colspan="6" class="p-6 text-center text-slate-500">Loading device fleet...</td></tr>
          </tbody>
        </table>
      </div>
    </section>

    <!-- TAB 3: PLANS & QUOTAS -->
    <section id="tab-plans" class="space-y-6 hidden">
      <div>
        <h3 class="text-base font-semibold text-white">Quotas & Tier Management</h3>
        <p class="text-xs text-slate-400 mt-0.5">Scale device fleets, encrypted storage, and multi-agent coordination.</p>
      </div>

      <div class="grid grid-cols-1 md:grid-cols-4 gap-4">
        <!-- Free Tier -->
        <div class="glass rounded-xl p-5 flex flex-col justify-between border-slate-800">
          <div class="space-y-3">
            <span class="text-xs font-semibold px-2 py-0.5 rounded bg-slate-800 text-slate-300">FREE</span>
            <h4 class="text-2xl font-bold text-white tracking-tight">$0 <span class="text-xs font-normal text-slate-500">/ forever</span></h4>
            <ul class="text-xs text-slate-400 space-y-2 pt-2">
              <li class="flex items-center gap-2">✓ 1 Active Agent Device</li>
              <li class="flex items-center gap-2">✓ Local SQLite & MCP</li>
              <li class="flex items-center gap-2">✓ 50 MB R2 Plan Storage</li>
              <li class="flex items-center gap-2">✓ Zero-Knowledge Vault</li>
            </ul>
          </div>
          <button onclick="simulateUpgrade('free')" class="mt-6 w-full py-2 rounded-lg bg-slate-800 hover:bg-slate-700 text-slate-200 text-xs font-medium border border-slate-700">
            Current / Switch to Free
          </button>
        </div>

        <!-- Plus Tier -->
        <div class="glass rounded-xl p-5 flex flex-col justify-between border-indigo-500/40 bg-indigo-950/20 relative">
          <div class="absolute -top-2.5 right-4 px-2 py-0.5 rounded-full bg-indigo-500 text-white text-[10px] font-bold uppercase tracking-wider">
            Popular
          </div>
          <div class="space-y-3">
            <span class="text-xs font-semibold px-2 py-0.5 rounded bg-indigo-500/20 text-indigo-300 border border-indigo-500/30">PLUS</span>
            <h4 class="text-2xl font-bold text-white tracking-tight">$12 <span class="text-xs font-normal text-slate-500">/ mo</span></h4>
            <ul class="text-xs text-slate-300 space-y-2 pt-2">
              <li class="flex items-center gap-2 font-medium text-white">✓ 5 Active Agent Devices</li>
              <li class="flex items-center gap-2">✓ Multi-Device Pairing</li>
              <li class="flex items-center gap-2">✓ 2 GB R2 Plan Storage</li>
              <li class="flex items-center gap-2">✓ Priority Restore & Sync</li>
              <li class="flex items-center gap-2">✓ Cloudflare Vectorize ANN</li>
            </ul>
          </div>
          <button onclick="simulateUpgrade('plus')" class="mt-6 w-full py-2 rounded-lg bg-indigo-600 hover:bg-indigo-500 text-white text-xs font-medium shadow-lg shadow-indigo-600/30">
            Upgrade to Plus
          </button>
        </div>

        <!-- Team Tier -->
        <div class="glass rounded-xl p-5 flex flex-col justify-between border-slate-800">
          <div class="space-y-3">
            <span class="text-xs font-semibold px-2 py-0.5 rounded bg-slate-800 text-emerald-400">TEAM</span>
            <h4 class="text-2xl font-bold text-white tracking-tight">$39 <span class="text-xs font-normal text-slate-500">/ mo</span></h4>
            <ul class="text-xs text-slate-400 space-y-2 pt-2">
              <li class="flex items-center gap-2">✓ 50 Connected Devices</li>
              <li class="flex items-center gap-2">✓ 10 Team Members</li>
              <li class="flex items-center gap-2">✓ 50 GB R2 Storage</li>
              <li class="flex items-center gap-2">✓ Shared Playbooks & Skills</li>
              <li class="flex items-center gap-2">✓ Slack/Discord Webhooks</li>
            </ul>
          </div>
          <button onclick="simulateUpgrade('team')" class="mt-6 w-full py-2 rounded-lg bg-slate-800 hover:bg-slate-700 text-slate-200 text-xs font-medium border border-slate-700">
            Upgrade to Team
          </button>
        </div>

        <!-- Enterprise Tier -->
        <div class="glass rounded-xl p-5 flex flex-col justify-between border-slate-800">
          <div class="space-y-3">
            <span class="text-xs font-semibold px-2 py-0.5 rounded bg-slate-800 text-purple-400">ENTERPRISE</span>
            <h4 class="text-2xl font-bold text-white tracking-tight">Custom</h4>
            <ul class="text-xs text-slate-400 space-y-2 pt-2">
              <li class="flex items-center gap-2">✓ 500+ Devices</li>
              <li class="flex items-center gap-2">✓ Dedicated R2 Storage</li>
              <li class="flex items-center gap-2">✓ Custom Guardrail Webhooks</li>
              <li class="flex items-center gap-2">✓ Dedicated SLA Support</li>
            </ul>
          </div>
          <button onclick="simulateUpgrade('enterprise')" class="mt-6 w-full py-2 rounded-lg bg-slate-800 hover:bg-slate-700 text-slate-200 text-xs font-medium border border-slate-700">
            Contact Enterprise
          </button>
        </div>
      </div>
    </section>

    <!-- TAB 4: SYNC TIMELINE -->
    <section id="tab-timeline" class="space-y-4 hidden">
      <div class="flex items-center justify-between">
        <div>
          <h3 class="text-base font-semibold text-white">Blind Sync Audit Timeline</h3>
          <p class="text-xs text-slate-400 mt-0.5">Real-time cryptographic audit log of edge pushes and pull events.</p>
        </div>
        <button id="refreshTimelineBtn" class="px-3 py-1.5 rounded-lg bg-slate-800 hover:bg-slate-700 text-slate-300 text-xs font-medium border border-slate-700">
          Refresh Log
        </button>
      </div>

      <div class="glass rounded-xl overflow-hidden">
        <table class="w-full text-left border-collapse">
          <thead>
            <tr class="border-b border-slate-800 bg-slate-900/60 text-xs text-slate-400 font-mono">
              <th class="p-3.5 pl-5">Timestamp</th>
              <th class="p-3.5">Event</th>
              <th class="p-3.5">Project</th>
              <th class="p-3.5">Device</th>
              <th class="p-3.5">Manifest SHA-256</th>
              <th class="p-3.5 pr-5 text-right">Items Synced</th>
            </tr>
          </thead>
          <tbody id="timelineTableBody" class="divide-y divide-slate-800/60 text-xs font-mono">
            <tr><td colspan="6" class="p-6 text-center text-slate-500 font-sans">Loading sync timeline...</td></tr>
          </tbody>
        </table>
      </div>
    </section>

    <!-- TAB 5: WEBHOOKS -->
    <section id="tab-webhooks" class="space-y-6 hidden">
      <div class="flex items-center justify-between">
        <div>
          <h3 class="text-base font-semibold text-white">Webhook Dispatchers</h3>
          <p class="text-xs text-slate-400 mt-0.5">Broadcast real-time guardrail alerts, memory leaks, and sync events to Slack or Discord.</p>
        </div>
      </div>

      <!-- Add Webhook Form -->
      <div class="glass rounded-xl p-5 space-y-4">
        <h4 class="text-sm font-semibold text-white">Register New Webhook Endpoint</h4>
        <div class="grid grid-cols-1 md:grid-cols-3 gap-3">
          <input id="webhookUrlInput" type="url" placeholder="https://hooks.slack.com/services/..."
            class="bg-slate-900 border border-slate-800 rounded-lg px-3 py-2 text-xs text-slate-200 outline-none focus:border-indigo-500">
          <input id="webhookSecretInput" type="text" placeholder="Signing secret (optional)"
            class="bg-slate-900 border border-slate-800 rounded-lg px-3 py-2 text-xs text-slate-200 outline-none focus:border-indigo-500">
          <button id="addWebhookBtn" class="py-2 px-4 rounded-lg bg-indigo-600 hover:bg-indigo-500 text-white text-xs font-medium shadow-md shadow-indigo-600/20">
            Register Webhook
          </button>
        </div>
      </div>

      <!-- Webhooks Table -->
      <div class="glass rounded-xl overflow-hidden">
        <table class="w-full text-left border-collapse">
          <thead>
            <tr class="border-b border-slate-800 bg-slate-900/60 text-xs text-slate-400 font-mono">
              <th class="p-3.5 pl-5">Target URL</th>
              <th class="p-3.5">Events</th>
              <th class="p-3.5">Secret</th>
              <th class="p-3.5">Status</th>
              <th class="p-3.5 pr-5 text-right">Actions</th>
            </tr>
          </thead>
          <tbody id="webhooksTableBody" class="divide-y divide-slate-800/60 text-xs">
            <tr><td colspan="5" class="p-6 text-center text-slate-500">Loading webhooks...</td></tr>
          </tbody>
        </table>
      </div>
    </section>
  </main>

  <!-- Client-side Logic -->
  <script>
    const DEFAULT_TOKEN = 'dev-token:test_alice:alice@automemory.local';
    let currentToken = localStorage.getItem('automemory_token') || DEFAULT_TOKEN;

    const tokenInput = document.getElementById('tokenInput');
    const saveTokenBtn = document.getElementById('saveTokenBtn');
    const planBadge = document.getElementById('planBadge');

    tokenInput.value = currentToken;

    saveTokenBtn.addEventListener('click', () => {
      const val = tokenInput.value.trim();
      if (val) {
        currentToken = val;
        localStorage.setItem('automemory_token', currentToken);
        loadAll();
      }
    });

    // Tab switcher
    document.querySelectorAll('.tab-btn').forEach(btn => {
      btn.addEventListener('click', () => {
        document.querySelectorAll('.tab-btn').forEach(b => {
          b.classList.remove('bg-slate-800', 'text-white', 'border', 'border-slate-700');
          b.classList.add('text-slate-400');
        });
        btn.classList.add('bg-slate-800', 'text-white', 'border', 'border-slate-700');
        btn.classList.remove('text-slate-400');

        const tab = btn.dataset.tab;
        ['overview', 'devices', 'plans', 'timeline', 'webhooks'].forEach(t => {
          document.getElementById('tab-' + t).classList.toggle('hidden', t !== tab);
        });

        if (tab === 'devices') loadDevices();
        if (tab === 'timeline') loadTimeline();
        if (tab === 'webhooks') loadWebhooks();
      });
    });

    async function fetchApi(path, options = {}) {
      const headers = {
        'Content-Type': 'application/json',
        'Authorization': 'Bearer ' + currentToken,
        ...(options.headers || {})
      };
      const res = await fetch(path, { ...options, headers });
      return res;
    }

    async function loadStats() {
      try {
        const res = await fetchApi('/api/v1/dashboard/stats');
        if (!res.ok) throw new Error('Auth failed');
        const data = await res.json();

        planBadge.textContent = data.user.plan_id.toUpperCase() + ' TIER';
        planBadge.className = 'px-2.5 py-1 rounded-md text-xs font-semibold uppercase border ' + 
          (data.user.plan_id === 'plus' ? 'bg-indigo-500/20 text-indigo-300 border-indigo-500/30' :
           data.user.plan_id === 'team' ? 'bg-emerald-500/20 text-emerald-300 border-emerald-500/30' :
           'bg-slate-800 text-slate-400 border-slate-700');

        document.getElementById('metricDevices').textContent = data.quota.active_devices_count;
        document.getElementById('metricMaxDevices').textContent = '/ ' + data.quota.max_active_devices + ' allowed';
        document.getElementById('metricMemories').textContent = data.metrics.blind_memories_count;
        document.getElementById('metricSkills').textContent = data.metrics.blind_skills_count;
        document.getElementById('metricProjects').textContent = data.metrics.total_projects;

        const maxMb = Math.round(data.quota.max_storage_bytes / (1024 * 1024));
        document.getElementById('storageMaxText').textContent = maxMb + ' MB Quota';

        const featuresEl = document.getElementById('featureTags');
        featuresEl.innerHTML = '';
        (data.quota.allowed_features || []).forEach(f => {
          const span = document.createElement('span');
          span.className = 'px-2 py-1 rounded bg-slate-800 text-[11px] font-mono text-slate-300 border border-slate-700';
          span.textContent = f;
          featuresEl.appendChild(span);
        });
      } catch (err) {
        planBadge.textContent = 'AUTH ERROR';
        planBadge.className = 'px-2.5 py-1 rounded-md text-xs font-semibold bg-red-500/20 text-red-400 border border-red-500/30';
      }
    }

    async function loadDevices() {
      const tbody = document.getElementById('deviceTableBody');
      try {
        const res = await fetchApi('/api/v1/auth/device/list');
        const data = await res.json();
        const devices = data.devices || [];

        if (devices.length === 0) {
          tbody.innerHTML = '<tr><td colspan="6" class="p-6 text-center text-slate-500">No active devices registered. Run automemory sync push to connect.</td></tr>';
          return;
        }

        tbody.innerHTML = devices.map(d => \`
          <tr class="hover:bg-slate-900/40">
            <td class="p-3.5 pl-5 font-medium text-white flex items-center gap-2">
              <span class="w-2 h-2 rounded-full \${d.is_active ? 'bg-emerald-400' : 'bg-slate-500'}"></span>
              \${d.device_name || 'Agent Device'}
            </td>
            <td class="p-3.5 font-mono text-slate-400">\${d.device_id}</td>
            <td class="p-3.5 text-slate-400 font-mono">\${new Date(d.last_seen_at).toLocaleTimeString()}</td>
            <td class="p-3.5 text-slate-400 font-mono">\${new Date(d.expires_at).toLocaleDateString()}</td>
            <td class="p-3.5">
              <span class="px-2 py-0.5 rounded text-[10px] font-medium \${d.is_active ? 'bg-emerald-500/10 text-emerald-400 border border-emerald-500/20' : 'bg-slate-800 text-slate-400'}">
                \${d.is_active ? 'ACTIVE' : 'EXPIRED'}
              </span>
            </td>
            <td class="p-3.5 pr-5 text-right">
              <button onclick="revokeDevice('\${d.device_id}')" class="px-2 py-1 rounded bg-red-500/10 text-red-400 hover:bg-red-500/20 text-xs border border-red-500/20">
                Revoke
              </button>
            </td>
          </tr>
        \`).join('');
      } catch (e) {
        tbody.innerHTML = '<tr><td colspan="6" class="p-6 text-center text-red-400">Failed to load devices.</td></tr>';
      }
    }

    async function revokeDevice(deviceId) {
      if (!confirm('Revoke access for device ' + deviceId + '?')) return;
      await fetchApi('/api/v1/auth/device/' + deviceId, { method: 'DELETE' });
      loadDevices();
      loadStats();
    }

    async function loadTimeline() {
      const tbody = document.getElementById('timelineTableBody');
      try {
        const res = await fetchApi('/api/v1/dashboard/timeline');
        const data = await res.json();
        const events = data.events || [];

        if (events.length === 0) {
          tbody.innerHTML = '<tr><td colspan="6" class="p-6 text-center text-slate-500 font-sans">No sync events recorded yet.</td></tr>';
          return;
        }

        tbody.innerHTML = events.map(e => \`
          <tr class="hover:bg-slate-900/40">
            <td class="p-3.5 pl-5 text-slate-300">\${new Date(e.created_at).toLocaleTimeString()}</td>
            <td class="p-3.5">
              <span class="px-1.5 py-0.5 rounded bg-indigo-500/20 text-indigo-300 border border-indigo-500/30 text-[10px]">
                \${e.event_type}
              </span>
            </td>
            <td class="p-3.5 text-white font-medium">\${e.project_slug}</td>
            <td class="p-3.5 text-slate-400">\${e.device_name || e.device_id || 'Unknown'}</td>
            <td class="p-3.5 text-slate-400 text-[11px]">\${(e.manifest_hash || '').substring(0, 16)}...</td>
            <td class="p-3.5 pr-5 text-right text-slate-300">
              \${e.blind_memories_count} mems · \${e.blind_skills_count} skills · \${e.deltas_count} files
            </td>
          </tr>
        \`).join('');
      } catch (e) {
        tbody.innerHTML = '<tr><td colspan="6" class="p-6 text-center text-red-400 font-sans">Failed to load timeline.</td></tr>';
      }
    }

    async function loadWebhooks() {
      const tbody = document.getElementById('webhooksTableBody');
      try {
        const res = await fetchApi('/api/v1/dashboard/webhooks');
        const data = await res.json();
        const hooks = data.webhooks || [];

        if (hooks.length === 0) {
          tbody.innerHTML = '<tr><td colspan="5" class="p-6 text-center text-slate-500">No webhooks registered. Add one above.</td></tr>';
          return;
        }

        tbody.innerHTML = hooks.map(h => \`
          <tr class="hover:bg-slate-900/40">
            <td class="p-3.5 pl-5 font-mono text-slate-200">\${h.url}</td>
            <td class="p-3.5 font-mono text-slate-400 text-[11px]">\${h.event_types}</td>
            <td class="p-3.5 font-mono text-slate-500 text-[11px]">\${(h.secret || '').substring(0, 10)}...</td>
            <td class="p-3.5">
              <span class="px-2 py-0.5 rounded text-[10px] font-medium \${h.is_active ? 'bg-emerald-500/10 text-emerald-400 border border-emerald-500/20' : 'bg-slate-800 text-slate-400'}">
                \${h.is_active ? 'ACTIVE' : 'DISABLED'}
              </span>
            </td>
            <td class="p-3.5 pr-5 text-right space-x-2">
              <button onclick="testWebhook('\${h.url}', '\${h.secret}')" class="px-2 py-1 rounded bg-slate-800 text-indigo-400 hover:bg-slate-700 text-xs border border-slate-700">
                Ping
              </button>
              <button onclick="deleteWebhook('\${h.id}')" class="px-2 py-1 rounded bg-red-500/10 text-red-400 hover:bg-red-500/20 text-xs border border-red-500/20">
                Delete
              </button>
            </td>
          </tr>
        \`).join('');
      } catch (e) {
        tbody.innerHTML = '<tr><td colspan="5" class="p-6 text-center text-red-400">Failed to load webhooks.</td></tr>';
      }
    }

    async function testWebhook(url, secret) {
      alert('Dispatching test ping to: ' + url);
      const res = await fetchApi('/api/v1/dashboard/webhooks/test', {
        method: 'POST',
        body: JSON.stringify({ url, secret })
      });
      const data = await res.json();
      alert('Test result: ' + (data.success ? 'SUCCESS (Latency: ' + data.latency_ms + 'ms)' : 'FAILED: ' + (data.error || 'HTTP ' + data.status)));
    }

    async function deleteWebhook(id) {
      if (!confirm('Delete webhook?')) return;
      await fetchApi('/api/v1/dashboard/webhooks/' + id, { method: 'DELETE' });
      loadWebhooks();
    }

    document.getElementById('addWebhookBtn').addEventListener('click', async () => {
      const url = document.getElementById('webhookUrlInput').value.trim();
      const secret = document.getElementById('webhookSecretInput').value.trim();
      if (!url) {
        alert('Please enter a valid webhook URL');
        return;
      }
      await fetchApi('/api/v1/dashboard/webhooks', {
        method: 'POST',
        body: JSON.stringify({ url, secret })
      });
      document.getElementById('webhookUrlInput').value = '';
      document.getElementById('webhookSecretInput').value = '';
      loadWebhooks();
    });

    async function simulateUpgrade(planId) {
      const res = await fetchApi('/api/v1/dashboard/upgrade-simulation', {
        method: 'POST',
        body: JSON.stringify({ plan_id: planId })
      });
      if (res.ok) {
        alert('Plan updated to: ' + planId.toUpperCase());
        loadAll();
      }
    }

    document.getElementById('refreshDevicesBtn').addEventListener('click', loadDevices);
    document.getElementById('refreshTimelineBtn').addEventListener('click', loadTimeline);

    function loadAll() {
      loadStats();
      loadDevices();
      loadTimeline();
      loadWebhooks();
    }

    // Initial load
    loadAll();
  </script>
</body>
</html>`;

  return c.html(html);
});
