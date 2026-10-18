<template>
  <div class="bg-surface border border-border rounded-card p-4 transition-colors hover:border-accent/40">
    <div class="flex items-center justify-between gap-2 mb-1.5">
      <span class="text-xs font-medium text-text-muted">{{ label }}</span>
      <div v-if="$slots.icon" class="text-text-muted">
        <slot name="icon" />
      </div>
    </div>
    <div class="flex items-baseline gap-2">
      <span class="text-2xl font-bold tracking-tight text-text">{{ value }}</span>
      <span v-if="unit" class="text-xs text-text-muted font-normal">{{ unit }}</span>
    </div>
    <p v-if="subtext" class="text-xs text-text-muted mt-1">{{ subtext }}</p>
  </div>
</template>

<script setup lang="ts">
defineProps<{
  label: string;
  value: string | number;
  unit?: string;
  subtext?: string;
}>();
</script>
