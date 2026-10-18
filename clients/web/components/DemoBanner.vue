<template>
  <div
    v-if="store.isDemoMode"
    class="bg-accent/10 px-4 sm:px-6 py-2.5 flex items-center justify-between gap-3 text-sm text-text"
  >
    <div class="flex items-center gap-2 min-w-0">
      <span class="px-2 py-0.5 rounded-pill text-xs font-semibold bg-accent-solid text-accent-fg whitespace-nowrap">Sample</span>
      <span class="truncate">You are looking at sample data. It is never synced or exported.</span>
    </div>

    <div class="flex items-center gap-2 flex-shrink-0">
      <Button variant="primary" size="sm" @click="store.startTour('showcase')">Walk me through</Button>
      <Button
        variant="secondary"
        size="sm"
        :loading="isRemoving"
        @click="onRemove"
      >
        Remove
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
