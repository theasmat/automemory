<template>
  <div class="flex items-center gap-1 border-b border-border overflow-x-auto no-scrollbar">
    <button
      v-for="tab in tabs"
      :key="tab.id"
      type="button"
      class="px-3.5 py-2 text-sm font-medium border-b-2 -mb-px transition-colors whitespace-nowrap flex items-center gap-1.5"
      :class="[
        modelValue === tab.id
          ? 'border-accent text-accent font-semibold'
          : 'border-transparent text-text-muted hover:text-text hover:border-border'
      ]"
      @click="$emit('update:modelValue', tab.id)"
    >
      <span>{{ tab.label }}</span>
      <span
        v-if="tab.count !== undefined"
        class="text-xs px-1.5 py-0.2 rounded-pill font-mono"
        :class="modelValue === tab.id ? 'bg-accent/20 text-accent' : 'bg-surface-2 text-text-muted'"
      >
        {{ tab.count }}
      </span>
    </button>
  </div>
</template>

<script setup lang="ts">
export interface TabItem {
  id: string;
  label: string;
  count?: number;
}

defineProps<{
  tabs: TabItem[];
  modelValue: string;
}>();

defineEmits<{
  (e: 'update:modelValue', id: string): void;
}>();
</script>
