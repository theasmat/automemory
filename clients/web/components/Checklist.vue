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
        class="p-3 rounded-card border transition-colors space-y-2"
        :class="item.done ? 'bg-surface-2/30 border-border/60' : 'bg-surface border-border'"
      >
        <div class="flex items-start justify-between gap-3">
          <div class="flex items-start gap-2.5 min-w-0">
            <div
              class="w-5 h-5 rounded-full flex items-center justify-center text-xs font-bold transition-colors flex-shrink-0 mt-0.5"
              :class="item.done ? 'bg-ok/20 text-ok' : 'border border-border text-text-muted'"
            >
              <span v-if="item.done">&#10003;</span>
              <span v-else class="w-1.5 h-1.5 rounded-full bg-border" />
            </div>
            <div>
              <span class="text-xs sm:text-sm font-semibold" :class="item.done ? 'line-through text-text-muted' : 'text-text'">
                {{ item.title }}
              </span>
              <p v-if="item.why" class="text-xs text-text-muted mt-0.5 leading-relaxed">
                {{ item.why }}
              </p>
            </div>
          </div>

          <div v-if="item.link && !item.done" class="flex-shrink-0">
            <NuxtLink :to="item.link">
              <Button size="sm" variant="secondary" class="text-xs">
                Show me &rarr;
              </Button>
            </NuxtLink>
          </div>
        </div>

        <div v-if="item.command && !item.done" class="pt-1">
          <CopyCommand :command="item.command" />
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
  why?: string;
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
