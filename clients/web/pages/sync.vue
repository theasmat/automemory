<template>
  <div class="p-8 space-y-6 max-w-7xl mx-auto overflow-y-auto h-full">
    <!-- Header Controls & Trigger -->
    <div class="flex flex-col md:flex-row md:items-center justify-between gap-4 pb-4 border-b border-dark-700">
      <div>
        <div class="flex items-center gap-2 flex-wrap">
          <Cloud class="w-5 h-5 text-gold-500" />
          <h1 class="text-xl font-bold font-mono text-white">Cloud Sync & Multi-Device Fleet</h1>
          <span class="px-2 py-0.5 text-xs font-mono bg-dark-800 text-gold-400 border border-dark-700">
            {{ syncData?.project_id || 'Active Project' }}
          </span>
          <span
            class="px-2 py-0.5 text-xs font-mono border flex items-center gap-1.5"
            :class="[
              (syncData?.pending_deltas || 0) === 0
                ? 'bg-emerald-500/10 text-emerald-400 border-emerald-500/30'
                : 'bg-amber-500/10 text-amber-400 border-amber-500/30'
            ]"
          >
            <span
              class="w-1.5 h-1.5 rounded-full"
              :class="(syncData?.pending_deltas || 0) === 0 ? 'bg-emerald-400' : 'bg-amber-400 animate-pulse'"
            ></span>
            <span>{{ (syncData?.pending_deltas || 0) === 0 ? 'All Synchronized' : `${syncData?.pending_deltas} Pending Deltas` }}</span>
          </span>
        </div>
        <p class="text-xs font-mono text-gray-500 mt-1">
          SHA-256 incremental delta replication, multi-device fleet leasing, and disaster recovery.
        </p>
      </div>

      <!-- Action Buttons -->
      <div class="flex items-center gap-3">
        <button
          @click="triggerSyncPush"
          :disabled="isPushing"
          class="px-3.5 py-1.5 text-xs font-mono bg-emerald-950 text-emerald-300 border border-emerald-800 hover:bg-emerald-900 transition-colors flex items-center gap-1.5 font-bold shadow-sm shadow-emerald-500/20"
        >
          <UploadCloud class="w-3.5 h-3.5" :class="{ 'animate-bounce': isPushing }" />
          <span>{{ isPushing ? 'Pushing...' : 'Push Deltas Now' }}</span>
        </button>
        <button
          @click="refreshStatus"
          :disabled="isRefreshing"
          class="px-3 py-1.5 text-xs font-mono bg-dark-900 text-gray-300 border border-dark-700 hover:text-white hover:border-dark-600 transition-colors flex items-center gap-1.5"
        >
          <RefreshCw class="w-3.5 h-3.5" :class="{ 'animate-spin': isRefreshing }" />
          <span>Refresh</span>
        </button>
      </div>
    </div>

    <!-- Active Device Fleet & Hardware Fingerprint Card -->
    <div class="border border-dark-700 bg-dark-950 p-5 space-y-4">
      <div class="flex items-center justify-between border-b border-dark-800 pb-3 flex-wrap gap-2">
        <div class="flex items-center gap-2">
          <Laptop class="w-4 h-4 text-cyan-400" />
          <h2 class="text-xs font-mono font-bold uppercase tracking-wider text-white">
            Current Machine Hardware Identity & Lease
          </h2>
        </div>
        <span class="text-[11px] font-mono text-cyan-400 bg-cyan-950/60 px-2 py-0.5 border border-cyan-800/80">
          Device Lease Verified
        </span>
      </div>

      <div class="grid grid-cols-1 md:grid-cols-3 gap-4 text-xs font-mono">
        <div class="bg-dark-900 border border-dark-800 p-3 space-y-1">
          <div class="text-gray-500 text-[10px] uppercase">Device Name</div>
          <div class="text-white font-bold">{{ syncData?.device_name || 'Local Workstation' }}</div>
          <div class="text-gray-500 text-[11px]">Auto-detected host identity</div>
        </div>

        <div class="bg-dark-900 border border-dark-800 p-3 space-y-1">
          <div class="text-gray-500 text-[10px] uppercase">Hardware Fingerprint ID</div>
          <code class="text-cyan-400 block bg-dark-950 p-1 border border-dark-850 truncate">
            {{ syncData?.device_id || 'dev_local_machine' }}
          </code>
          <div class="text-gray-500 text-[11px]">Deterministic UUID lease key</div>
        </div>

        <div class="bg-dark-900 border border-dark-800 p-3 space-y-1">
          <div class="text-gray-500 text-[10px] uppercase">Fleet Quota Status</div>
          <div class="text-emerald-400 font-bold flex items-center gap-1.5">
            <span class="w-2 h-2 rounded-full bg-emerald-400"></span>
            <span>Active Device Slot Granted</span>
          </div>
          <div class="text-gray-500 text-[11px]">30-Day Auto-Renewing Lease</div>
        </div>
      </div>
    </div>

    <!-- Metrics Cards Grid -->
    <div class="grid grid-cols-1 sm:grid-cols-2 lg:grid-cols-4 gap-4">
      <div class="border border-dark-700 bg-dark-950 p-4 space-y-1">
        <div class="text-[10px] font-mono text-gray-500 uppercase tracking-wider">Sync Mode</div>
        <div class="text-base font-bold font-mono text-white flex items-center gap-1.5">
          <span>Local-First Single Source</span>
        </div>
        <div class="text-[11px] font-mono text-cyan-400">OpenViking Canonical Root</div>
      </div>

      <div class="border border-dark-700 bg-dark-950 p-4 space-y-1">
        <div class="text-[10px] font-mono text-gray-500 uppercase tracking-wider">Tracked Documents</div>
        <div class="text-base font-bold font-mono text-white">
          {{ syncData?.total_documents || 0 }} files
        </div>
        <div class="text-[11px] font-mono text-emerald-400">
          {{ syncData?.synced_documents || 0 }} Synced • {{ syncData?.pending_deltas || 0 }} Pending
        </div>
      </div>

      <div class="border border-dark-700 bg-dark-950 p-4 space-y-1">
        <div class="text-[10px] font-mono text-gray-500 uppercase tracking-wider">Active Memory Brain</div>
        <div class="text-base font-bold font-mono text-white">
          {{ syncData?.active_memories || 0 }} records
        </div>
        <div class="text-[11px] font-mono text-gold-400">
          {{ syncData?.skills_count || 0 }} Procedural Playbooks
        </div>
      </div>

      <div class="border border-dark-700 bg-dark-950 p-4 space-y-1">
        <div class="text-[10px] font-mono text-gray-500 uppercase tracking-wider">Local Snapshots</div>
        <div class="text-base font-bold font-mono text-white">
          {{ (syncData?.snapshots || []).length }} snapshots
        </div>
        <div class="text-[11px] font-mono text-gray-400">
          Canonical Offline Ledger
        </div>
      </div>
    </div>

    <!-- Disaster Recovery & New Machine Reset Flow Guide -->
    <div class="border border-dark-700 bg-dark-950 p-5 space-y-4">
      <div class="flex items-center justify-between border-b border-dark-800 pb-3">
        <div class="flex items-center gap-2">
          <Terminal class="w-4 h-4 text-gold-500" />
          <h2 class="text-xs font-mono font-bold uppercase tracking-wider text-white">
            Disaster Recovery & Machine Reset ("New Laptop" Flow)
          </h2>
        </div>
        <span class="text-[11px] font-mono text-emerald-400 bg-emerald-500/10 px-2 py-0.5 border border-emerald-500/20">
          Intelligent Fuzzy Path Remapping Active
        </span>
      </div>

      <p class="text-xs font-mono text-gray-400 leading-relaxed">
        When switching computers or resetting your environment, AutoMemory Pro reconstructs all architectural decisions, procedural skills, and product documents with zero loss.
      </p>

      <div class="grid grid-cols-1 md:grid-cols-3 gap-3 text-xs font-mono">
        <div class="bg-dark-900 border border-dark-800 p-3 space-y-1">
          <div class="text-gold-400 font-bold">1. Re-link Project</div>
          <code class="text-gray-300 block bg-dark-950 p-1.5 border border-dark-850 mt-1">automemory link</code>
          <p class="text-gray-500 text-[11px] mt-1">Creates canonical local context and links workspace via .context symlink.</p>
        </div>

        <div class="bg-dark-900 border border-dark-800 p-3 space-y-1">
          <div class="text-cyan-400 font-bold">2. Restore Context</div>
          <code class="text-gray-300 block bg-dark-950 p-1.5 border border-dark-850 mt-1">automemory sync restore</code>
          <p class="text-gray-500 text-[11px] mt-1">Downloads latest manifest and materializes all documents and memories.</p>
        </div>

        <div class="bg-dark-900 border border-dark-800 p-3 space-y-1">
          <div class="text-emerald-400 font-bold">3. Conflict Protection</div>
          <code class="text-gray-300 block bg-dark-950 p-1.5 border border-dark-850 mt-1">*.remote.md isolated</code>
          <p class="text-gray-500 text-[11px] mt-1">If local uncommitted changes exist, cloud version is safely written to .remote.md.</p>
        </div>
      </div>
    </div>

    <!-- Snapshot Ledger -->
    <div class="border border-dark-700 bg-dark-950 p-5 space-y-3">
      <div class="flex items-center justify-between border-b border-dark-800 pb-3">
        <div class="flex items-center gap-2">
          <History class="w-4 h-4 text-gold-500" />
          <h2 class="text-xs font-mono font-bold uppercase tracking-wider text-white">
            Canonical Local Delta Snapshots
          </h2>
        </div>
        <span class="text-xs font-mono text-gray-500">
          ~/.automemory_pro/context/projects/{{ syncData?.project_id || 'c-todo-app' }}/deltas/
        </span>
      </div>

      <div v-if="(syncData?.snapshots || []).length === 0" class="text-xs font-mono text-gray-500 p-4 text-center">
        No delta snapshots generated yet. Run <code class="text-gold-400">automemory sync push</code> to generate an incremental snapshot.
      </div>

      <div v-else class="space-y-2">
        <div
          v-for="snap in syncData.snapshots"
          :key="snap"
          class="flex items-center justify-between p-3 bg-dark-900 border border-dark-800 text-xs font-mono"
        >
          <div class="flex items-center gap-2">
            <span class="text-gold-500 font-bold">📦</span>
            <span class="text-white">{{ snap }}</span>
          </div>
          <span class="text-gray-500 text-[11px]">JSON Delta Package</span>
        </div>
      </div>
    </div>
  </div>
</template>

<script setup lang="ts">
import { ref, onMounted } from 'vue';
import { useAutoMemoryStore } from '~/stores/automemory';
import { Cloud, RefreshCw, Terminal, History, Laptop, UploadCloud } from 'lucide-vue-next';

const store = useAutoMemoryStore();
const syncData = ref<any>(null);
const isRefreshing = ref(false);
const isPushing = ref(false);

const refreshStatus = async () => {
  isRefreshing.value = true;
  try {
    await store.fetchSyncStatus();
    syncData.value = store.syncStatus;
  } finally {
    isRefreshing.value = false;
  }
};

const triggerSyncPush = async () => {
  isPushing.value = true;
  try {
    await fetch('/api/sync/trigger', { method: 'POST' });
    store.showLiveToast('Local deltas queued for cloud synchronization', 'success');
    await refreshStatus();
  } catch {
    store.showLiveToast('Sync trigger request failed', 'error');
  } finally {
    isPushing.value = false;
  }
};

onMounted(async () => {
  await refreshStatus();
});
</script>
