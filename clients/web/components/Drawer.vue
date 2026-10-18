<template>
  <Teleport to="body">
    <Transition
      enter-active-class="transition duration-200 ease-out"
      enter-from-class="opacity-0"
      enter-to-class="opacity-100"
      leave-active-class="transition duration-150 ease-in"
      leave-from-class="opacity-100"
      leave-to-class="opacity-0"
    >
      <div
        v-if="isOpen"
        class="fixed inset-0 z-50 bg-overlay backdrop-blur-sm"
        role="dialog"
        aria-modal="true"
        @keydown.esc="$emit('close')"
      >
        <div class="fixed inset-0" @click="$emit('close')" />

        <div
          class="fixed inset-y-0 right-0 z-10 w-full bg-surface border-l border-border shadow-2xl flex flex-col transition-transform duration-200 ease-out"
          :class="widthClass || 'max-w-md sm:max-w-xl'"
        >
          <div class="flex items-center justify-between gap-4 p-5 border-b border-border">
            <div>
              <h2 class="text-base font-bold text-text">{{ title }}</h2>
              <p v-if="description" class="text-xs text-text-muted mt-0.5">{{ description }}</p>
            </div>
            <IconButton aria-label="Close drawer" size="sm" @click="$emit('close')">
              <svg class="w-4 h-4" fill="none" viewBox="0 0 24 24" stroke="currentColor">
                <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M6 18L18 6M6 6l12 12" />
              </svg>
            </IconButton>
          </div>

          <div class="flex-1 overflow-y-auto p-5 space-y-4">
            <slot />
          </div>

          <div v-if="$slots.footer" class="p-4 border-t border-border bg-surface-2/40 flex items-center justify-end gap-3">
            <slot name="footer" />
          </div>
        </div>
      </div>
    </Transition>
  </Teleport>
</template>

<script setup lang="ts">
defineProps<{
  isOpen: boolean;
  title: string;
  description?: string;
  widthClass?: string;
}>();

defineEmits<{
  (e: 'close'): void;
}>();
</script>
