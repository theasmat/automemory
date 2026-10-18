<template>
  <div class="space-y-6">
    <PageHeader
      title="Rules of the project"
      subtitle="Inviolable codebase invariants, safety rules, and top architectural decisions loaded at the start of every session."
    >
      <template #badge>
        <Badge variant="neutral">CONSTITUTION.md</Badge>
      </template>
      <template #actions>
        <Button variant="secondary" size="sm" @click="store.fetchConstitution">
          Refresh
        </Button>
      </template>
    </PageHeader>

    <Card>
      <template #header>
        <div class="flex items-center justify-between">
          <span class="text-sm font-semibold text-text">Active rules payload</span>
          <Badge v-if="store.constitution" variant="neutral">
            ~{{ estimateTokens(store.constitution) }} / 500 max tokens
          </Badge>
        </div>
      </template>

      <div v-if="!store.constitution" class="py-8">
        <EmptyState
          title="No project rules distilled yet"
          message="As you record decisions and mark invariants, AutoMemory distills them into CONSTITUTION.md."
          action-label="Open Project brain"
          action-to="/brain"
        />
      </div>
      <div v-else class="space-y-3">
        <div class="text-xs text-text-muted">
          Every agent connected to this project receives these invariants on launch:
        </div>
        <pre class="text-xs text-text font-mono whitespace-pre-wrap leading-relaxed bg-surface-2 p-4 rounded-card border border-border overflow-x-auto">{{ store.constitution }}</pre>
      </div>
    </Card>
  </div>
</template>

<script setup lang="ts">
import { onMounted } from 'vue';
import { useAutoMemoryStore } from '~/stores/automemory';
import { estimateTokens } from '~/utils/tokens';

const store = useAutoMemoryStore();

onMounted(() => {
  store.fetchConstitution();
});
</script>
