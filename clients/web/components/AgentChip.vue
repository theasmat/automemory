<template>
  <div class="inline-flex items-center gap-2 px-2.5 py-1 rounded-pill bg-surface-2 border border-border text-xs">
    <span
      class="w-2 h-2 rounded-full flex-shrink-0"
      :class="[
        status === 'editing' ? 'bg-warn animate-pulse' :
        status === 'active' ? 'bg-ok' :
        status === 'idle' ? 'bg-accent' :
        'bg-text-muted/40'
      ]"
      :title="status"
    />
    <span class="font-medium text-text">{{ name }}</span>
    <span v-if="detail" class="text-text-muted text-xs">&bull; {{ detail }}</span>
  </div>
</template>

<script setup lang="ts">
defineProps<{
  name: string;
  status?: 'editing' | 'active' | 'idle' | 'offline' | string;
  detail?: string;
}>();
</script>
