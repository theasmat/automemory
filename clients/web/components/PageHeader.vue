<template>
  <div class="flex flex-col sm:flex-row sm:items-center justify-between gap-4 mb-6 pb-4 border-b border-border">
    <div>
      <div class="flex items-center gap-3">
        <h1 class="text-xl sm:text-2xl font-bold tracking-tight text-text">{{ title }}</h1>
        <slot name="badge" />
      </div>
      <p v-if="description" class="text-xs sm:text-sm text-text-muted mt-1">{{ description }}</p>
    </div>
    <div v-if="$slots.actions" class="flex items-center gap-2 self-start sm:self-auto">
      <slot name="actions" />
    </div>
  </div>
</template>

<script setup lang="ts">
defineProps<{
  title: string;
  description?: string;
}>();
</script>
