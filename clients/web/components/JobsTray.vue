<template>
  <div class="relative">
    <button
      type="button"
      class="relative flex items-center justify-center w-9 h-9 rounded-pill bg-surface-2 hover:bg-border/70 text-text-muted hover:text-text transition-colors"
      :aria-label="running.length ? `${running.length} jobs running` : 'Background jobs'"
      :aria-expanded="open"
      @click="toggle"
    >
      <svg class="w-4 h-4" :class="running.length ? 'animate-spin text-accent' : ''" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
        <path v-if="running.length" d="M21 12a9 9 0 11-6.2-8.55" />
        <path v-else d="M4 6h16M4 12h16M4 18h10" />
      </svg>
      <span v-if="running.length" class="absolute -top-1 -right-1 min-w-4 h-4 px-1 rounded-full bg-accent-solid text-accent-fg text-xs font-semibold grid place-items-center">{{ running.length }}</span>
    </button>

    <div v-if="open" class="absolute right-0 mt-2 w-80 max-w-[90vw] rounded-card bg-surface border border-border shadow-2xl z-40 p-3 space-y-2" role="dialog" aria-label="Background jobs">
      <p v-if="!jobs.length" class="text-sm text-text-muted px-1 py-2">Nothing has run yet. Start a job from Tools or Recall settings.</p>
      <ul v-else class="space-y-2 max-h-80 overflow-y-auto">
        <li v-for="j in jobs.slice(0, 8)" :key="j.id" class="rounded-xl bg-surface-2/60 p-3">
          <div class="flex items-center justify-between gap-2">
            <span class="text-sm font-medium text-text truncate">{{ j.title }}</span>
            <button v-if="j.state === 'running'" type="button" class="text-xs text-accent hover:underline" @click="stop(j.id)">Stop</button>
            <span v-else class="text-xs" :class="j.state === 'done' ? 'text-ok' : j.state === 'failed' ? 'text-bad' : 'text-text-muted'">{{ label(j.state) }}</span>
          </div>
          <div v-if="j.state === 'running'" class="mt-2 h-1.5 rounded-pill bg-surface overflow-hidden">
            <div class="h-full rounded-pill bg-ok transition-all" :style="{ width: (j.total ? Math.round((j.done / j.total) * 100) : 15) + '%' }" />
          </div>
          <p v-if="j.message" class="text-xs text-text-muted mt-1 line-clamp-2 whitespace-pre-line">{{ j.message }}</p>
        </li>
      </ul>
    </div>
  </div>
</template>

<script setup lang="ts">
import { ref } from 'vue';
import { safeCall } from '~/utils/api';
import { useJobs } from '~/composables/useJobs';

const { jobs, running, refresh } = useJobs();
const open = ref(false);

const label = (s: string) => ({ done: 'Done', failed: 'Failed', cancelled: 'Stopped' } as Record<string, string>)[s] || s;

function toggle() {
  open.value = !open.value;
  if (open.value) refresh();
}

async function stop(id: string) {
  await safeCall('POST', `/api/jobs/${id}/cancel`);
  await refresh();
}
</script>
