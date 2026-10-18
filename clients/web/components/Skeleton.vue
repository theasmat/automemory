<template>
  <div
    class="bg-surface-2 animate-pulse rounded-card"
    :class="[
      height || 'h-4',
      width || 'w-full',
      customClass || ''
    ]"
  />
</template>

<script setup lang="ts">
defineProps<{
  height?: string;
  width?: string;
  customClass?: string;
}>();
</script>
