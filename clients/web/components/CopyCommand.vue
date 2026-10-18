<template>
  <div class="w-full">
    <div v-if="label" class="text-xs font-medium text-text-muted mb-1">{{ label }}</div>
    <div class="flex items-center justify-between gap-2 bg-surface-2 border border-border rounded-card px-3 py-2 text-[13px] font-mono">
      <code class="text-text select-all overflow-x-auto whitespace-nowrap">{{ command }}</code>
      <button
        type="button"
        class="text-text-muted hover:text-accent p-1 transition-colors flex-shrink-0"
        :title="copied ? 'Copied!' : 'Copy to clipboard'"
        :aria-label="copied ? 'Copied' : 'Copy command'"
        @click="copy"
      >
        <span v-if="copied" class="text-ok text-xs font-sans font-medium">Copied!</span>
        <svg v-else class="w-3.5 h-3.5" fill="none" viewBox="0 0 24 24" stroke="currentColor">
          <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M8 16H6a2 2 0 01-2-2V6a2 2 0 012-2h8a2 2 0 012 2v2m-6 12h8a2 2 0 002-2v-8a2 2 0 00-2-2h-8a2 2 0 00-2 2v8a2 2 0 002 2z" />
        </svg>
      </button>
    </div>
  </div>
</template>

<script setup lang="ts">
import { ref } from 'vue';

const props = defineProps<{
  command: string;
  label?: string;
}>();

const copied = ref(false);

async function copy() {
  try {
    await navigator.clipboard.writeText(props.command);
    copied.value = true;
    setTimeout(() => {
      copied.value = false;
    }, 2000);
  } catch {
    // fallback
  }
}
</script>
