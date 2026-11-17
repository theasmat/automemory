<template>
  <div class="flex flex-col lg:flex-row lg:items-center justify-between gap-3 sm:gap-4 mb-6 sm:mb-8">
    <div class="min-w-0">
      <div class="flex items-center gap-3">
        <h1 class="text-2xl sm:text-[32px] sm:leading-tight font-semibold tracking-tight text-text [text-wrap:balance]">{{ title }}</h1>
        <slot name="badge" />
      </div>
      <p v-if="description" class="text-sm sm:text-base text-text-muted mt-1.5 max-w-2xl">{{ description }}</p>
    </div>
    <div v-if="$slots.actions" class="flex flex-wrap items-center gap-2 self-start lg:self-auto">
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
