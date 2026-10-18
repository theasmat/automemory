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
import { ref } from 'vue';
import { useRouter } from 'vue-router';
import { useAutoMemoryStore } from '~/stores/automemory';
import { useTheme } from '~/composables/useTheme';

const store = useAutoMemoryStore();
const router = useRouter();
const { themeMode, setTheme } = useTheme();
const busy = ref('');

const themes = [
  { id: 'system', label: 'System' },
  { id: 'light', label: 'Light' },
  { id: 'dark', label: 'Dark' },
] as const;

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
