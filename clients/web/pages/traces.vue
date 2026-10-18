<template>
  <div class="space-y-6">
    <PageHeader
      title="What worked"
      subtitle="Repeatable execution paths, step-by-step verified procedures, and regression baselines."
    >
      <template #badge>
        <Badge variant="neutral">{{ store.traces.length }} traces</Badge>
      </template>
      <template #actions>
        <Button variant="secondary" size="sm" @click="store.fetchTraces">
          Refresh
        </Button>
      </template>
    </PageHeader>

    <!-- Empty State -->
    <div v-if="store.traces.length === 0" class="py-8">
      <Card>
        <EmptyState
          title="No success traces recorded yet"
          message="When an agent successfully completes a complex task or setup, call trace.save to record the golden path."
        />
      </Card>
    </div>

    <!-- Traces List -->
    <div v-else class="space-y-4">
      <Card
        v-for="t in store.traces"
        :key="t.id"
      >
        <div class="space-y-4">
          <div class="flex items-center justify-between flex-wrap gap-2">
            <div class="flex items-center gap-2">
              <Badge variant="ok">Verified</Badge>
              <h3 class="text-sm font-semibold text-text">{{ t.task_intent }}</h3>
            </div>
            <div class="flex items-center gap-2 text-xs">
              <Badge variant="neutral">
                ~{{ estimateTokens(t.task_intent + ' ' + JSON.stringify(t.golden_steps)) }} tokens
              </Badge>
              <span class="text-text-muted">{{ formatTime(t.created_at) }}</span>
            </div>
          </div>

          <!-- Golden Steps List -->
          <div class="space-y-2">
            <div
              v-for="(step, idx) in t.golden_steps"
              :key="idx"
              class="p-3 bg-surface-2 rounded-card border border-border text-xs space-y-1.5"
            >
              <div class="flex items-center justify-between text-text font-medium">
                <span>Step {{ idx + 1 }}: {{ step.action || 'Action' }}</span>
                <span class="text-[11px] text-text-muted">
                  ~{{ estimateTokens(step.action + ' ' + (step.command || '') + ' ' + (step.diff || '')) }} tokens
                </span>
              </div>
              <div v-if="step.command">
                <CopyCommand :command="step.command" />
              </div>
              <div v-if="step.diff" class="bg-surface p-2 rounded border border-border text-text font-mono text-xs whitespace-pre-wrap">
                {{ step.diff }}
              </div>
            </div>
          </div>

          <!-- Verification Command & Tags -->
          <div class="pt-3 border-t border-border flex items-center justify-between flex-wrap gap-2 text-xs">
            <div v-if="t.verification_command" class="flex items-center gap-2 text-text-muted">
              <span>Verification:</span>
              <code class="text-ok bg-surface-2 px-2 py-0.5 rounded border border-border font-mono">{{ t.verification_command }}</code>
            </div>

            <div v-if="t.tags && t.tags.length > 0" class="flex items-center gap-1.5 flex-wrap">
              <Badge
                v-for="tag in t.tags"
                :key="tag"
                variant="neutral"
              >
                #{{ tag }}
              </Badge>
            </div>
          </div>
        </div>
      </Card>
    </div>
  </div>
</template>

<script setup lang="ts">
import { useAutoMemoryStore } from '~/stores/automemory';
import { estimateTokens } from '~/utils/tokens';

const store = useAutoMemoryStore();

const formatTime = (iso: string) => {
  try {
    const d = new Date(iso);
    return d.toLocaleDateString(undefined, { month: 'short', day: 'numeric', hour: '2-digit', minute: '2-digit' });
  } catch {
    return iso;
  }
};
</script>
