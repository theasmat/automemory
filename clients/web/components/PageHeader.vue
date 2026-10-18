<template>
  <div class="flex flex-col sm:flex-row sm:items-center justify-between gap-4 mb-8">
    <div>
      <div class="flex items-center gap-3">
        <h1 class="text-2xl sm:text-[32px] sm:leading-tight font-semibold tracking-tight text-text">{{ title }}</h1>
        <slot name="badge" />
      </div>
      <p v-if="description" class="text-sm sm:text-base text-text-muted mt-1.5 max-w-2xl">{{ description }}</p>
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
