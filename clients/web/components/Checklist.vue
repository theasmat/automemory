<template>
  <Card :title="title">
    <template #actions>
      <Badge variant="neutral">
        {{ doneCount }} of {{ items.length }} complete
      </Badge>
    </template>

    <div class="space-y-3 mt-2">
      <div
        v-for="item in items"
        :key="item.id"
        class="flex flex-col sm:flex-row sm:items-center justify-between gap-2 p-3 rounded-card border transition-colors"
        :class="item.done ? 'bg-surface-2/30 border-border/60 text-text-muted' : 'bg-surface border-border text-text'"
      >
        <div class="flex items-center gap-2.5">
          <div
            class="w-5 h-5 rounded-full flex items-center justify-center text-xs font-bold transition-colors"
            :class="item.done ? 'bg-ok/20 text-ok' : 'border border-border text-text-muted'"
          >
            <span v-if="item.done">&#10003;</span>
            <span v-else class="w-1.5 h-1.5 rounded-full bg-border" />
          </div>
          <span class="text-xs sm:text-sm font-medium" :class="item.done ? 'line-through text-text-muted' : 'text-text'">
            {{ item.title }}
          </span>
        </div>

        <div class="flex items-center gap-2 self-end sm:self-auto">
          <code v-if="item.command" class="text-[11px] font-mono bg-surface-2 px-2 py-0.5 rounded border border-border text-text-muted">
            {{ item.command }}
          </code>
          <NuxtLink v-if="item.link && !item.done" :to="item.link">
            <Button size="sm" variant="secondary">
              Configure &rarr;
            </Button>
          </NuxtLink>
        </div>
      </div>
    </div>
  </Card>
</template>

<script setup lang="ts">
import { computed } from 'vue';

export interface ChecklistItem {
  id: string;
  title: string;
  done: boolean;
  command?: string;
  link?: string;
}

const props = defineProps<{
  title: string;
  items: ChecklistItem[];
}>();

const doneCount = computed(() => {
  return props.items.filter((i) => i.done).length;
});
</script>
