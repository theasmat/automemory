<template>
  <div class="space-y-6">
    <PageHeader
      title="Sync &amp; devices"
      description="Back up your memory to a private git repository and keep your computers in step. Encrypted before it leaves this computer."
    >
    </PageHeader>

    <ErrorState
      v-if="!store.isConnected && store.daemonError"
      title="AutoMemory isn't running"
      :message="store.daemonError"
      command="automemory serve"
      @retry="store.fetchSyncStatus()"
    />

    <GitBackupCard />

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

const store = useAutoMemoryStore();
const resumeRows = computed(() => {
  const latestDec = store.memories.find((m) => m.type?.toLowerCase() === 'decision');
  return [
    { label: 'Project', value: store.activeProject || 'All projects' },
    { label: 'Latest decision', value: latestDec ? latestDec.summary : 'None yet' },
    { label: 'Notes', value: String(store.memories.length) },
    { label: 'Files locked', value: String(store.activeLeases.length) },
  ];
});
</script>
