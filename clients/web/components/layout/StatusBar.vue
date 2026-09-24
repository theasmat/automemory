<template>
  <header class="h-12 border-b border-dark-700 bg-dark-900 px-6 flex items-center justify-between shrink-0 select-none z-30">
    <!-- Left: Connection Indicator & Active Scope -->
    <div class="flex items-center gap-4">
      <div class="flex items-center gap-2">
        <span
          class="w-2 h-2 rounded-full"
          :class="store.isConnected ? 'bg-emerald-400 animate-pulse shadow-sm shadow-emerald-400/50' : 'bg-amber-400 animate-ping'"
        ></span>
        <span class="text-xs font-mono" :class="store.isConnected ? 'text-emerald-400 font-bold' : 'text-amber-400 font-semibold'">
          {{ store.isConnected ? 'LIVE WEBSOCKET' : 'CONNECTING...' }}
        </span>
      </div>

      <div class="h-3.5 w-px bg-dark-700 hidden sm:block"></div>

      <!-- Project Filter Selector -->
      <div class="flex items-center gap-1.5 text-xs font-mono text-gray-400">
        <FolderGit2 class="w-3.5 h-3.5 text-gold-500" />
        <select
          v-model="store.activeProject"
          @change="onProjectChange"
          class="bg-dark-950 border border-dark-700 text-xs font-mono text-gray-200 px-2 py-0.5 focus:outline-none focus:border-gold-500"
        >
          <option :value="null">All Projects (Global Scope)</option>
          <option v-for="p in store.projects" :key="p.id" :value="p.slug">
            {{ p.display_name }} ({{ p.slug }})
          </option>
        </select>
      </div>
    </div>

    <!-- Center: Interactive Autopilot Mode Switcher -->
    <div class="hidden md:flex items-center bg-dark-950 border border-dark-700 p-0.5 font-mono text-xs">
      <span class="px-2 text-[10px] text-gray-500 font-semibold uppercase">Mode:</span>
      <button
        v-for="m in modes"
        :key="m.id"
        @click="switchMode(m.id)"
        class="px-2.5 py-0.5 text-[11px] font-mono transition-all flex items-center gap-1"
        :class="[
          store.currentMode === m.id
            ? `${m.activeClass} font-bold shadow-sm`
            : 'text-gray-400 hover:text-white'
        ]"
        :title="m.description"
      >
        <span>{{ m.icon }}</span>
        <span>{{ m.label }}</span>
      </button>
    </div>

    <!-- Right: Sync Action & Quick Refresh -->
    <div class="flex items-center gap-3">
      <button
        @click="triggerVaultSync"
        :disabled="isSyncing"
        class="flex items-center gap-1.5 px-3 py-1 bg-dark-800 hover:bg-dark-700 border border-dark-700 text-xs font-mono text-gray-300 transition-colors disabled:opacity-50"
      >
        <RefreshCw class="w-3 h-3 text-gold-500" :class="{ 'animate-spin': isSyncing }" />
        <span>{{ isSyncing ? 'Syncing...' : 'Sync Vault' }}</span>
      </button>

      <button
        @click="store.fetchAll"
        class="p-1 text-gray-500 hover:text-white transition-colors"
        title="Refresh data"
      >
        <RotateCw class="w-3.5 h-3.5" :class="{ 'animate-spin': store.isLoading }" />
      </button>
    </div>
  </header>
</template>

<script setup lang="ts">
import { ref } from 'vue';
import { useAutoMemoryStore } from '~/stores/automemory';
import { FolderGit2, RefreshCw, RotateCw } from 'lucide-vue-next';

const store = useAutoMemoryStore();
const isSyncing = ref(false);

const modes = [
  {
    id: 'vibe' as const,
    label: 'Vibe',
    icon: '⚡',
    activeClass: 'bg-gold-500 text-black',
    description: 'Max developer velocity. Inline non-blocking warnings.',
  },
  {
    id: 'strict' as const,
    label: 'Strict',
    icon: '🔒',
    activeClass: 'bg-red-950 text-red-300 border border-red-800',
    description: 'High safety. Destructive ops & DB migrations require confirmation.',
  },
  {
    id: 'review' as const,
    label: 'Review',
    icon: '🔍',
    activeClass: 'bg-cyan-950 text-cyan-300 border border-cyan-800',
    description: 'Proactive session briefing with past decisions & gotchas.',
  },
];

const switchMode = (mode: 'vibe' | 'strict' | 'review') => {
  store.setMode(mode, store.activeProject || undefined);
};

const onProjectChange = () => {
  store.fetchMode();
  store.fetchMemories();
  store.fetchSessions();
  store.fetchTraces();
  store.fetchFailures();
  store.fetchProposals();
  store.showLiveToast(`Switched project filter to: ${store.activeProject || 'All Projects'}`, 'info');
};

const triggerVaultSync = async () => {
  isSyncing.value = true;
  try {
    const res = await fetch('/api/sync/trigger', { method: 'POST' });
    if (res.ok) {
      store.showLiveToast('Local vault synchronized successfully', 'success');
      store.fetchMemories();
      store.fetchStats();
    }
  } catch (err) {
    store.showLiveToast('Vault sync failed', 'warning');
  } finally {
    isSyncing.value = false;
  }
};
</script>
