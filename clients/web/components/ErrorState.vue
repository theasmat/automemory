<template>
  <div class="flex flex-col items-center justify-center text-center p-8 sm:p-10 border border-bad/30 rounded-card bg-bad/5 my-4">
    <div class="w-10 h-10 rounded-full bg-bad/10 text-bad flex items-center justify-center mb-3">
      <svg class="w-5 h-5" fill="none" viewBox="0 0 24 24" stroke="currentColor">
        <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M12 9v2m0 4h.01m-6.938 4h13.856c1.54 0 2.502-1.667 1.732-3L13.732 4c-.77-1.333-2.694-1.333-3.464 0L3.34 16c-.77 1.333.192 3 1.732 3z" />
      </svg>
    </div>
    <h3 class="text-sm sm:text-base font-semibold text-text mb-1">{{ title || 'Failed to load' }}</h3>
    <p class="text-sm text-text-muted max-w-md mb-4">{{ message }}</p>
    
    <div v-if="isDaemonDown" class="w-full max-w-sm mb-4">
      <CopyCommand command="automemory serve" label="Start the daemon:" />
    </div>

    <div class="flex items-center gap-3">
      <Button v-if="retry" variant="secondary" size="sm" @click="$emit('retry')">
        Try again
      </Button>
    </div>
  </div>
</template>

<script setup lang="ts">
import { computed } from 'vue';

const props = defineProps<{
  title?: string;
  message: string;
  retry?: boolean;
}>();

defineEmits<{
  (e: 'retry'): void;
}>();

const isDaemonDown = computed(() => {
  return /unreachable|failed to fetch|network/i.test(props.message);
});
</script>
