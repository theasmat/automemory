<template>
  <div class="space-y-8 max-w-3xl">
    <PageHeader title="Settings" description="Appearance, guided learning and sample data." />

    <!-- Guided learning: the first-run tour is shown once, it can be replayed here any time -->
    <section aria-labelledby="learn-h" class="space-y-3">
      <h2 id="learn-h" class="text-lg font-semibold text-text">Guided learning</h2>
      <Card>
        <ul class="divide-y divide-border/60">
          <li class="flex items-center justify-between gap-4 py-4 first:pt-0">
            <div class="min-w-0">
              <p class="text-[15px] font-medium text-text">Take the tour</p>
              <p class="text-sm text-text-muted">A one-minute look at the main pages. Shown once on first visit; replay it any time.</p>
            </div>
            <Button variant="primary" size="sm" @click="startIntro">Start tour</Button>
          </li>
          <li class="flex items-center justify-between gap-4 py-4">
            <div class="min-w-0">
              <p class="text-[15px] font-medium text-text">Walk through every page with sample data</p>
              <p class="text-sm text-text-muted">Loads a pretend online shop, then explains all 14 pages with something to look at.</p>
            </div>
            <Button variant="secondary" size="sm" :loading="busy === 'showcase'" @click="startShowcase">Walk me through</Button>
          </li>
          <li class="flex items-center justify-between gap-4 py-4 last:pb-0">
            <div class="min-w-0">
              <p class="text-[15px] font-medium text-text">Show the welcome card again</p>
              <p class="text-sm text-text-muted">Resets your onboarding progress and brings back the Home welcome and setup steps.</p>
            </div>
            <Button variant="secondary" size="sm" :loading="busy === 'reset'" @click="resetOnboarding">Reset</Button>
          </li>
        </ul>
      </Card>
    </section>

    <!-- Appearance -->
    <section aria-labelledby="look-h" class="space-y-3">
      <h2 id="look-h" class="text-lg font-semibold text-text">Appearance</h2>
      <Card>
        <div class="flex items-center justify-between gap-4">
          <div>
            <p class="text-[15px] font-medium text-text">Theme</p>
            <p class="text-sm text-text-muted">System follows your device.</p>
          </div>
          <div class="inline-flex rounded-pill bg-surface-2 p-1" role="radiogroup" aria-label="Theme">
            <button
              v-for="opt in themes"
              :key="opt.id"
              type="button"
              role="radio"
              :aria-checked="themeMode === opt.id"
              class="px-3.5 py-1.5 text-sm rounded-pill transition-colors"
              :class="themeMode === opt.id ? 'bg-surface text-text font-semibold shadow-card' : 'text-text-muted hover:text-text'"
              @click="setTheme(opt.id)"
            >
              {{ opt.label }}
            </button>
          </div>
        </div>
      </Card>
    </section>

    <!-- Sample data -->
    <section aria-labelledby="demo-h" class="space-y-3">
      <h2 id="demo-h" class="text-lg font-semibold text-text">Sample data</h2>
      <Card>
        <div class="flex items-center justify-between gap-4">
          <div class="min-w-0">
            <p class="text-[15px] font-medium text-text flex items-center gap-2">
              Sample project
              <Badge :variant="store.isDemoMode ? 'accent' : 'neutral'">{{ store.isDemoMode ? 'Loaded' : 'Not loaded' }}</Badge>
            </p>
            <p class="text-sm text-text-muted">A pretend online shop that fills every page. It is never synced or exported.</p>
          </div>
          <Button v-if="store.isDemoMode" variant="danger" size="sm" :loading="busy === 'remove'" @click="removeSample">Remove</Button>
          <Button v-else variant="secondary" size="sm" :loading="busy === 'seed'" @click="loadSample">Load</Button>
        </div>
      </Card>
    </section>

    <!-- Locks -->
    <section aria-labelledby="locks-h" class="space-y-3">
      <h2 id="locks-h" class="text-lg font-semibold text-text">File locks</h2>
      <Card>
        <div class="space-y-4">
          <div>
            <label class="block text-sm text-text-muted mb-1" for="idle">Release a lock after this many minutes without activity</label>
            <input id="idle" v-model.number="lockMinutes" type="number" min="1" max="1440" class="w-32 bg-surface-2 border border-transparent rounded-xl px-3.5 py-2 text-sm text-text" />
          </div>
          <div>
            <label class="block text-sm text-text-muted mb-1" for="ign">Files that never take a lock (one pattern per line)</label>
            <textarea id="ign" v-model="lockIgnore" rows="4" class="w-full bg-surface-2 border border-transparent rounded-xl px-3.5 py-2 text-sm text-text font-mono" />
          </div>
          <label class="flex items-center gap-3 text-sm text-text">
            <input v-model="lockReconcile" type="checkbox" />
            <span>After each shell command, check which files it changed and report the ones nobody had locked</span>
          </label>
          <div class="flex items-center gap-3">
            <Button size="sm" variant="primary" lock="/api/settings/locks" :loading="busy === 'locks'" @click="saveLocks">Save</Button>
            <span class="text-xs text-text-muted">Asks for your password.</span>
          </div>
        </div>
      </Card>
    </section>

    <!-- Activity -->
    <section aria-labelledby="act-h" class="space-y-3">
      <h2 id="act-h" class="text-lg font-semibold text-text">Recent activity</h2>
      <Card>
        <p v-if="!activity.length" class="text-sm text-text-muted">Nothing blocked or changed from the dashboard yet.</p>
        <ul v-else class="divide-y divide-border/60">
          <li v-for="(a, i) in activity.slice(0, 12)" :key="i" class="py-3 first:pt-0 last:pb-0 flex items-start justify-between gap-4">
            <div class="min-w-0">
              <p class="text-sm text-text">{{ describeActivity(a) }}</p>
              <p v-if="a.details?.reason" class="text-xs text-text-muted truncate">{{ a.details.reason }}</p>
            </div>
            <span class="text-xs text-text-muted flex-shrink-0">{{ ago(a.at) }}</span>
          </li>
        </ul>
      </Card>
    </section>

    <!-- Help and about -->
    <section aria-labelledby="about-h" class="space-y-3">
      <h2 id="about-h" class="text-lg font-semibold text-text">Help</h2>
      <Card>
        <ul class="divide-y divide-border/60">
          <li class="flex items-center justify-between gap-4 py-4 first:pt-0">
            <div>
              <p class="text-[15px] font-medium text-text">Glossary, shortcuts and CLI topics</p>
              <p class="text-sm text-text-muted">Also available from the ? button at the top.</p>
            </div>
            <Button variant="secondary" size="sm" @click="store.openHelpDrawer()">Open help</Button>
          </li>
          <li class="flex items-center justify-between gap-4 py-4 last:pb-0">
            <div>
              <p class="text-[15px] font-medium text-text">AutoMemory v0.1</p>
              <p class="text-sm text-text-muted">{{ store.isConnected ? 'Connected to the daemon.' : 'Not connected. Start it with automemory serve.' }}</p>
            </div>
            <Badge :variant="store.isConnected ? 'ok' : 'warn'">{{ store.isConnected ? 'Running' : 'Offline' }}</Badge>
          </li>
        </ul>
      </Card>
    </section>
  </div>
</template>

<script setup lang="ts">
import { ref, onMounted } from 'vue';
import { protectedCall, request } from '~/utils/api';
import { useRouter } from 'vue-router';
import { useAutoMemoryStore } from '~/stores/automemory';
import { useTheme } from '~/composables/useTheme';
import { useToast } from '~/composables/useToast';

const store = useAutoMemoryStore();
const router = useRouter();
const { themeMode, setTheme } = useTheme();
const busy = ref('');
const toast = useToast();

const themes = [
  { id: 'system', label: 'System' },
  { id: 'light', label: 'Light' },
  { id: 'dark', label: 'Dark' },
] as const;

const lockMinutes = ref(10);
const lockIgnore = ref('');
const lockReconcile = ref(true);
const activity = ref<any[]>([]);

onMounted(async () => {
  const l = await request<any>('/api/settings/locks');
  if (l.ok && l.data) {
    lockMinutes.value = Math.round((l.data.idle_ttl_seconds || 600) / 60);
    lockIgnore.value = (l.data.ignore || []).join('\n');
    lockReconcile.value = l.data.reconcile !== false;
  }
  const a = await request<any[]>('/api/activity');
  if (a.ok && Array.isArray(a.data)) activity.value = a.data;
});

async function saveLocks() {
  busy.value = 'locks';
  try {
    const r = await protectedCall('PUT', '/api/settings/locks', 'change lock settings', {
      idle_ttl_seconds: Math.round(lockMinutes.value * 60),
      ignore: lockIgnore.value.split('\n'),
      reconcile: lockReconcile.value,
    });
    if (r.ok) toast.success('Lock settings saved. They apply from the next agent action.');
    else toast.error(r.error || 'Not saved');
  } finally {
    busy.value = '';
  }
}

function describeActivity(a: any): string {
  if (a.action === 'hook.deny') return `Blocked ${a.details?.agent || 'an agent'}: ${a.target}`;
  if (a.action === 'hook.ask') return `Asked before ${a.target} (${a.details?.agent || 'agent'})`;
  if (a.action === 'ui.action') return `Dashboard: ${a.target}`;
  return a.action;
}

function ago(iso: string): string {
  const m = Math.max(0, Math.round((Date.now() - new Date(iso).getTime()) / 60000));
  if (m < 1) return 'just now';
  if (m < 60) return `${m} min ago`;
  if (m < 1440) return `${Math.round(m / 60)} h ago`;
  return `${Math.round(m / 1440)} d ago`;
}

async function startIntro() {
  await router.push('/');
  store.startTour('intro');
}

async function startShowcase() {
  busy.value = 'showcase';
  try {
    if (!store.isDemoMode && !(await store.seedDemoProject())) return;
    await router.push('/');
    store.startTour('showcase');
  } finally {
    busy.value = '';
  }
}

async function resetOnboarding() {
  busy.value = 'reset';
  try {
    if (await store.updateOnboardingProgress({ action: 'reset' })) await router.push('/');
  } finally {
    busy.value = '';
  }
}

async function loadSample() {
  busy.value = 'seed';
  try {
    await store.seedDemoProject();
  } finally {
    busy.value = '';
  }
}

async function removeSample() {
  busy.value = 'remove';
  try {
    await store.removeDemoProject();
  } finally {
    busy.value = '';
  }
}
</script>
