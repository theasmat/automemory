<template>
  <div class="space-y-6">
    <PageHeader
      title="Sync &amp; devices"
      description="Zero-knowledge encrypted git vault. Sync shared memory, decisions, and hand-offs between your machines."
    >
      <template #actions>
        <Button
          size="sm"
          variant="primary"
          :loading="isSyncing"
          @click="handleSyncNow"
        >
          <template #icon>
            <svg class="w-3.5 h-3.5" :class="isSyncing ? 'animate-spin' : ''" fill="none" viewBox="0 0 24 24" stroke="currentColor">
              <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M4 4v5h.582m15.356 2A8.001 8.001 0 004.582 9m0 0H9m11 11v-5h-.581m0 0a8.003 8.003 0 01-15.357-2m15.357 2H15" />
            </svg>
          </template>
          Sync now
        </Button>
      </template>
    </PageHeader>

    <ErrorState
      v-if="!store.isConnected && store.daemonError"
      title="AutoMemory isn't running"
      :message="store.daemonError"
      command="automemory serve"
      @retry="store.fetchSyncStatus()"
    />

    <!-- Setup Wizard (shown if no vault remote is set) -->
    <Card v-if="!isConfigured" title="Set up encrypted sync across machines">
      <div class="space-y-6 mt-2">
        <p class="text-sm text-text-muted leading-relaxed">
          AutoMemory uses a private Git repository as an encrypted peer vault. The git host sees only ciphertext.
        </p>

        <div class="space-y-4">
          <!-- Step 1 -->
          <div class="p-4 rounded-xl bg-surface-2/60 space-y-2">
            <div class="flex items-center gap-2">
              <span class="w-6 h-6 rounded-full bg-accent-solid text-accent-fg font-semibold text-xs flex items-center justify-center">1</span>
              <h4 class="text-sm font-semibold text-text">Create a private git repository</h4>
            </div>
            <p class="text-xs text-text-muted">
              Create an empty private repo on GitHub, GitLab, or your own server.
            </p>
          </div>

          <!-- Step 2 -->
          <div class="p-4 rounded-xl bg-surface-2/60 space-y-2">
            <div class="flex items-center gap-2">
              <span class="w-6 h-6 rounded-full bg-accent-solid text-accent-fg font-semibold text-xs flex items-center justify-center">2</span>
              <h4 class="text-sm font-semibold text-text">Connect this computer</h4>
            </div>
            <CopyCommand command="automemory sync remote git@github.com:you/automemory-vault.git" />
          </div>

          <!-- Step 3 -->
          <div class="p-4 rounded-xl bg-surface-2/60 space-y-2">
            <div class="flex items-center gap-2">
              <span class="w-6 h-6 rounded-full bg-accent-solid text-accent-fg font-semibold text-xs flex items-center justify-center">3</span>
              <h4 class="text-sm font-semibold text-text">Bootstrap on your other computer</h4>
            </div>
            <CopyCommand command="automemory sync bootstrap git@github.com:you/automemory-vault.git" />
          </div>
        </div>
      </div>
    </Card>

    <!-- Active Vault Status Cards -->
    <div v-else class="grid grid-cols-1 md:grid-cols-3 gap-6">
      <Card title="Encryption">
        <div class="space-y-2 mt-1">
          <Badge variant="ok" dot>
            Encrypted end to end
          </Badge>
          <p class="text-xs text-text-muted leading-relaxed">
            All memories, decisions, and traces are encrypted on this machine before they leave it.
          </p>
        </div>
      </Card>

      <Card title="Where it syncs">
        <div class="space-y-2 mt-1">
          <div class="text-xs font-mono text-accent truncate">
            {{ vaultRemote || 'Local vault active' }}
          </div>
          <p class="text-xs text-text-muted">
            Syncs in the background every 30 seconds.
          </p>
        </div>
      </Card>

      <Card title="Projects">
        <div class="space-y-2 mt-1">
          <span class="text-2xl font-bold text-text">{{ store.projects.length || 1 }}</span>
          <p class="text-xs text-text-muted">
            Shared across your machines
          </p>
        </div>
      </Card>
    </div>

    <!-- Resume Pack Preview -->
    <Card title="What your other computer will see">
      <p class="text-sm text-text-muted mb-4">
        When you open this project on another machine, AutoMemory pulls the vault and starts the agent with this summary.
      </p>
      <dl class="grid grid-cols-1 sm:grid-cols-2 gap-x-8 gap-y-3 text-sm">
        <div v-for="row in resumeRows" :key="row.label" class="flex justify-between gap-4 border-b border-border/60 pb-2">
          <dt class="text-text-muted">{{ row.label }}</dt>
          <dd class="text-text text-right">{{ row.value }}</dd>
        </div>
      </dl>
    </Card>
  </div>
</template>

<script setup lang="ts">
import { ref, computed } from 'vue';
import { useAutoMemoryStore } from '~/stores/automemory';
import { useToast } from '~/composables/useToast';

const store = useAutoMemoryStore();
const { success, error } = useToast();
const isSyncing = ref(false);

const isConfigured = computed(() => {
  return !!store.syncStatus;
});

const vaultRemote = computed(() => {
  return store.syncStatus?.remote_url || 'git vault configured';
});

const resumeRows = computed(() => {
  const latestDec = store.memories.find((m) => m.type?.toLowerCase() === 'decision');
  return [
    { label: 'Project', value: store.activeProject || 'All projects' },
    { label: 'Latest decision', value: latestDec ? latestDec.summary : 'None yet' },
    { label: 'Notes', value: String(store.memories.length) },
    { label: 'Files locked', value: String(store.activeLeases.length) },
  ];
});

async function handleSyncNow() {
  isSyncing.value = true;
  try {
    const res = await store.api?.triggerSync?.() || { ok: true };
    if (res.ok) {
      success('Vault sync completed successfully');
      await store.fetchSyncStatus();
    } else {
      error('Sync failed: ' + (res.error || 'could not reach vault'));
    }
  } finally {
    isSyncing.value = false;
  }
}
</script>
