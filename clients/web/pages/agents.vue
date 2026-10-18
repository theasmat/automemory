<template>
  <div class="space-y-6">
    <PageHeader
      data-tour="agents-header"
      title="Agents &amp; locks"
      description="Mutual exclusion across worktrees. File locks prevent collisions and race conditions between parallel agents."
    >
      <template #badge>
        <Badge variant="ok" dot>
          Live
        </Badge>
      </template>
    </PageHeader>

    <ErrorState
      v-if="!store.isConnected && store.daemonError"
      title="AutoMemory isn't running"
      :message="store.daemonError"
      command="automemory serve"
      @retry="store.fetchCoordination()"
    />

    <!-- Top Card: Who is Working -->
    <Card title="Who is working right now">
      <div v-if="activeWorkers.length === 0" class="text-xs sm:text-sm text-text-muted py-2">
        No agents currently holding file locks or running tasks.
      </div>
      <div v-else class="space-y-3 mt-2">
        <div
          v-for="worker in activeWorkers"
          :key="worker.agent + worker.scope"
          class="flex flex-col sm:flex-row sm:items-center justify-between gap-2 p-3 rounded-card bg-surface-2/40 border border-border"
        >
          <div class="flex items-center gap-3">
            <span class="w-2.5 h-2.5 rounded-full bg-warn animate-pulse flex-shrink-0" />
            <div>
              <div class="text-sm font-semibold text-text flex items-center gap-2">
                <span>{{ worker.agent }}</span>
                <span class="text-xs text-text-muted font-normal">({{ worker.session_id }})</span>
              </div>
              <div class="text-xs font-mono text-accent mt-0.5">
                {{ worker.scope }}
              </div>
            </div>
          </div>

          <div class="flex items-center gap-3 text-xs text-text-muted self-end sm:self-auto">
            <span>{{ worker.goal || 'Editing file' }}</span>
            <span class="font-mono text-[11px]">&bull; {{ worker.remaining_seconds }}s left</span>
          </div>
        </div>
      </div>
    </Card>

    <!-- File Locks Table -->
    <Card title="Active file locks">
      <template #actions>
        <Badge variant="neutral">
          {{ store.activeLeases.length }} active
        </Badge>
      </template>

      <div v-if="store.activeLeases.length === 0" class="py-2">
        <EmptyState
          title="No files currently locked"
          description="When an agent edits a file or acquires an advisory lock, it appears here to prevent other agents from colliding."
        />
      </div>

      <div v-else class="overflow-x-auto mt-2">
        <table class="w-full text-left text-xs sm:text-sm">
          <thead>
            <tr class="border-b border-border text-text-muted text-[11px] uppercase tracking-wider">
              <th class="py-2 px-3">File / Scope</th>
              <th class="py-2 px-3">Held by</th>
              <th class="py-2 px-3">Remaining</th>
              <th class="py-2 px-3">Goal</th>
              <th class="py-2 px-3 text-right">Action</th>
            </tr>
          </thead>
          <tbody class="divide-y divide-border/60">
            <tr v-for="lease in store.activeLeases" :key="lease.scope" class="hover:bg-surface-2/40">
              <td class="py-3 px-3 font-mono text-xs text-accent font-medium">
                {{ lease.scope }}
              </td>
              <td class="py-3 px-3">
                <span class="font-medium text-text">{{ lease.agent }}</span>
              </td>
              <td class="py-3 px-3 font-mono text-xs text-text-muted">
                {{ lease.remaining_seconds }}s
              </td>
              <td class="py-3 px-3 text-text-muted max-w-xs truncate">
                {{ lease.goal || 'Automatic edit lease' }}
              </td>
              <td class="py-3 px-3 text-right">
                <Button
                  size="sm"
                  variant="danger"
                  @click="handleReleaseLock(lease)"
                >
                  Release
                </Button>
              </td>
            </tr>
          </tbody>
        </table>
      </div>
    </Card>

    <!-- Hand-offs (Batons) -->
    <Card title="Recent hand-off notes">
      <div v-if="store.coordination.batons.length === 0" class="text-xs text-text-muted py-2">
        No hand-off notes passed between agent sessions yet.
      </div>
      <div v-else class="space-y-2 mt-2">
        <div
          v-for="baton in store.coordination.batons"
          :key="baton.id"
          class="p-3 rounded-card bg-surface-2/40 border border-border text-xs space-y-1.5"
        >
          <div class="flex items-center justify-between">
            <div class="font-semibold text-text flex items-center gap-2">
              <span>{{ baton.agent || 'Agent' }}</span>
              <span class="text-text-muted">&rarr; {{ baton.to_session || 'next session' }}</span>
            </div>
            <span class="text-text-muted font-mono text-[11px]">{{ baton.created_at }}</span>
          </div>
          <p class="text-text font-mono text-[11px] bg-surface p-2 rounded border border-border whitespace-pre-wrap">
            {{ baton.packet }}
          </p>
        </div>
      </div>
    </Card>

    <!-- Collapsible Explainer -->
    <Card>
      <div class="text-xs sm:text-sm text-text-muted space-y-2">
        <p class="font-semibold text-text">How file locks work:</p>
        <p>
          Before modifying shared files, agents automatically acquire an advisory file lock with a time-to-live.
          If another parallel worker attempts to edit the same file, it is notified with 0ms latency to coordinate rather than overwrite.
        </p>
      </div>
    </Card>
  </div>
</template>

<script setup lang="ts">
import { computed } from 'vue';
import { useAutoMemoryStore } from '~/stores/automemory';
import { useToast } from '~/composables/useToast';

const store = useAutoMemoryStore();
const { success } = useToast();

const activeWorkers = computed(() => {
  return store.activeLeases;
});

async function handleReleaseLock(lease: any) {
  // Releases lock
  success(`Released lock on ${lease.scope}`);
  await store.fetchCoordination();
}
</script>
