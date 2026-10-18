<template>
  <div class="flex flex-col items-center justify-center text-center p-8 sm:p-12 border border-dashed border-border rounded-card bg-surface/40 my-4">
    <div v-if="$slots.icon" class="w-12 h-12 rounded-full bg-surface-2 flex items-center justify-center text-text-muted mb-4">
      <slot name="icon" />
    </div>
    <h3 class="text-sm sm:text-base font-semibold text-text mb-1">{{ title }}</h3>
    <p v-if="description" class="text-sm text-text-muted max-w-md mb-6 leading-relaxed">
      {{ description }}
    </p>
    <div v-if="$slots.action" class="flex items-center gap-3">
      <slot name="action" />
    </div>
  </div>
</template>

<script setup lang="ts">
defineProps<{
  title: string;
  description?: string;
}>();
</script>
