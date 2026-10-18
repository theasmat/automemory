<template>
  <div
    class="bg-surface border border-border/70 shadow-card rounded-card p-5 sm:p-6 transition-colors"
    :class="[
      hover ? 'hover:bg-surface-2 cursor-pointer' : '',
      accentBorder ? 'border-accent' : '',
      customClass || ''
    ]"
  >
    <div v-if="$slots.header || title" class="flex items-center justify-between gap-3 mb-3">
      <div class="flex items-center gap-2">
        <slot name="icon" />
        <h3 v-if="title" class="text-[15px] font-semibold text-text">{{ title }}</h3>
      </div>
      <div v-if="$slots.actions" class="flex items-center gap-2">
        <slot name="actions" />
      </div>
    </div>
    <slot />
  </div>
</template>

<script setup lang="ts">
defineProps<{
  title?: string;
  hover?: boolean;
  accentBorder?: boolean;
  customClass?: string;
}>();
</script>
