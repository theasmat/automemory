<template>
  <div class="space-y-6">
    <div class="flex items-center justify-end gap-3">
      <Button variant="secondary" size="sm" @click="store.fetchTraces">Refresh</Button>
    </div>

    <EmptyState
      v-if="store.traces.length === 0"
      title="Nothing recorded yet"
      description="When an agent finishes a tricky task, the steps that worked are saved here so the next agent can repeat them."
    />

    <div v-else class="space-y-4">
      <Card v-for="t in store.traces" :key="t.id">
        <div class="space-y-4">
          <div class="flex items-start justify-between gap-4">
            <div class="min-w-0">
              <h3 class="text-[17px] font-semibold text-text">{{ titleOf(t) }}</h3>
              <p class="text-sm text-text-muted mt-0.5">
                {{ stepsOf(t).length }} step{{ stepsOf(t).length === 1 ? '' : 's' }}
                <template v-if="t.category"> · {{ t.category }}</template>
                · {{ formatTime(t.created_at) }}
              </p>
            </div>
            <Badge v-if="typeof t.reproducibility_score === 'number'" variant="ok">
              {{ Math.round(t.reproducibility_score * 100) }}% repeatable
            </Badge>
          </div>

          <ol class="space-y-3">
            <li v-for="(step, idx) in stepsOf(t)" :key="idx" class="flex gap-3">
              <span class="w-6 h-6 rounded-full bg-surface-2 text-xs font-semibold text-text-muted grid place-items-center flex-shrink-0 mt-0.5">
                {{ idx + 1 }}
              </span>
              <div class="min-w-0 flex-1">
                <p class="text-[15px] text-text">{{ step.action || 'Action' }}</p>
                <div v-if="step.command" class="mt-1.5"><CopyCommand :command="step.command" /></div>
                <p v-if="step.verification" class="text-sm text-text-muted mt-1">Check: {{ step.verification }}</p>
              </div>
            </li>
          </ol>

          <p v-if="t.caveats?.length" class="text-sm text-text-muted rounded-xl bg-warn-bg px-4 py-3">
            Good to know: {{ t.caveats.join(' · ') }}
          </p>
        </div>
      </Card>
    </div>
  </div>
</template>

<script setup lang="ts">
import { useAutoMemoryStore } from '~/stores/automemory';

const store = useAutoMemoryStore();

/** The API returns `title` and `steps`; older records used `task_intent` and `golden_steps`. */
const titleOf = (t: any): string => t.title || t.task_intent || 'Untitled run';
const stepsOf = (t: any): any[] => (Array.isArray(t.steps) ? t.steps : Array.isArray(t.golden_steps) ? t.golden_steps : []);

const formatTime = (iso: string) => {
  try {
    return new Date(iso).toLocaleDateString(undefined, { month: 'short', day: 'numeric' });
  } catch {
    return iso;
  }
};
</script>
