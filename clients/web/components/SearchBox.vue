<template>
  <div class="relative w-full max-w-md">
    <div class="absolute inset-y-0 left-0 pl-3 flex items-center pointer-events-none text-text-muted">
      <svg class="w-4 h-4" fill="none" viewBox="0 0 24 24" stroke="currentColor">
        <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M21 21l-6-6m2-5a7 7 0 11-14 0 7 7 0 0114 0z" />
      </svg>
    </div>
    <input
      :value="modelValue"
      type="text"
      :placeholder="placeholder || 'Search...'"
      class="w-full bg-surface border border-border-strong rounded-card pl-9 pr-8 py-1.5 text-xs sm:text-sm text-text placeholder-text-muted focus:outline-none focus:ring-2 focus:ring-accent focus:ring-offset-2 focus:ring-offset-bg transition-colors"
      @input="$emit('update:modelValue', ($event.target as HTMLInputElement).value)"
    />
    <button
      v-if="modelValue"
      type="button"
      class="absolute inset-y-0 right-0 pr-2.5 flex items-center text-text-muted hover:text-text"
      @click="$emit('update:modelValue', '')"
    >
      <svg class="w-3.5 h-3.5" fill="none" viewBox="0 0 24 24" stroke="currentColor">
        <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M6 18L18 6M6 6l12 12" />
      </svg>
    </button>
  </div>
</template>

<script setup lang="ts">
defineProps<{
  modelValue: string;
  placeholder?: string;
}>();

defineEmits<{
  (e: 'update:modelValue', val: string): void;
}>();
</script>
