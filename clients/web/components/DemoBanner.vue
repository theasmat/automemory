<template>
  <div
    v-if="store.isDemoMode"
    class="bg-accent/10 border-b border-accent/25 px-4 sm:px-6 py-2.5 flex items-center justify-between gap-3 text-xs sm:text-sm text-text"
  >
    <div class="flex items-center gap-2 min-w-0">
      <span class="px-1.5 py-0.5 rounded-pill text-[10px] font-bold uppercase tracking-wider bg-accent-solid text-accent-fg">
        Sample Mode
      </span>
      <span class="truncate">
        You are looking at sample demo data. Sample items are excluded from sync and instruction export.
      </span>
    </div>

    <div class="flex items-center gap-2 flex-shrink-0">
      <Button
        variant="secondary"
        size="sm"
        class="text-xs border-accent/40 hover:bg-accent/15"
        :loading="isRemoving"
        @click="onRemove"
      >
        Remove sample data
      </Button>
    </div>
  </div>
</template>

<script setup lang="ts">
import { ref } from 'vue';
import { useAutoMemoryStore } from '~/stores/automemory';

const store = useAutoMemoryStore();
const isRemoving = ref(false);

async function onRemove() {
  isRemoving.value = true;
  try {
    await store.removeDemoProject();
  } finally {
    isRemoving.value = false;
  }
}
</script>
