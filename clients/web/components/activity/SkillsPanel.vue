<template>
  <div class="space-y-6">
    <div class="flex items-center justify-between gap-3">
      <Badge variant="neutral">{{ store.skills.length }} skills</Badge>
      <Button variant="secondary" size="sm" @click="store.fetchSkills">Refresh</Button>
    </div>

    <!-- Empty State -->
    <div v-if="store.skills.length === 0" class="py-8">
      <Card>
        <EmptyState
          title="No learned skills yet"
          description="As agents run tasks and record what worked, AutoMemory synthesizes reusable skills."
        />
      </Card>
    </div>

    <!-- Skills List -->
    <div v-else class="space-y-4">
      <Card
        v-for="s in store.skills"
        :key="s.id"
      >
        <div class="space-y-4">
          <div class="flex items-center justify-between flex-wrap gap-2">
            <div class="flex items-center gap-2">
              <Badge variant="neutral">Skill</Badge>
              <h3 class="text-sm font-semibold text-text">{{ s.name }}</h3>
            </div>
            <div class="flex items-center gap-2 text-xs">
              <Badge variant="ok">{{ s.success_count }} runs</Badge>
              <Badge variant="neutral">~{{ estimateTokens(s.name + ' ' + s.description + ' ' + JSON.stringify(s.steps)) }} tokens</Badge>
            </div>
          </div>

          <p class="text-xs text-text-muted leading-relaxed">{{ s.description }}</p>

          <!-- Trigger Patterns -->
          <div v-if="s.trigger_patterns && s.trigger_patterns.length > 0" class="flex items-center gap-2 flex-wrap">
            <span class="text-xs text-text-muted font-medium">Triggers:</span>
            <Badge
              v-for="pat in s.trigger_patterns"
              :key="pat"
              variant="neutral"
            >
              {{ pat }}
            </Badge>
          </div>

          <!-- Execution Steps -->
          <div class="space-y-2">
            <div class="text-xs font-semibold text-text">Execution steps:</div>
            <div class="space-y-1.5">
              <div
                v-for="(step, idx) in s.steps"
                :key="idx"
                class="flex items-start gap-2 p-2.5 bg-surface-2 rounded-card border border-border text-xs text-text font-mono"
              >
                <span class="text-text-muted font-semibold w-4 text-right flex-shrink-0">{{ idx + 1 }}.</span>
                <span class="flex-1 whitespace-pre-wrap">{{ step }}</span>
              </div>
            </div>
          </div>
        </div>
      </Card>
    </div>
  </div>
</template>

<script setup lang="ts">
import { onMounted } from 'vue';
import { useAutoMemoryStore } from '~/stores/automemory';
import { estimateTokens } from '~/utils/tokens';

const store = useAutoMemoryStore();

onMounted(() => {
  store.fetchSkills();
});
</script>
