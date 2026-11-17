<template>
  <Teleport to="body">
    <Transition
      enter-active-class="transition duration-150 ease-out"
      enter-from-class="opacity-0"
      enter-to-class="opacity-100"
      leave-active-class="transition duration-100 ease-in"
      leave-from-class="opacity-100"
      leave-to-class="opacity-0"
    >
      <div
        v-if="isOpen"
        class="fixed inset-0 z-50 flex items-end sm:items-center justify-center sm:p-4 bg-overlay backdrop-blur-sm"
        role="dialog"
        aria-modal="true"
        :aria-labelledby="titleId"
        @keydown.esc="onClose"
      >
        <div class="fixed inset-0" @click="onClose" />

        <div
          ref="modalRef"
          class="relative w-full bg-surface border border-border rounded-t-2xl sm:rounded-card shadow-2xl p-4 sm:p-6 pb-[calc(1rem+env(safe-area-inset-bottom))] sm:pb-6 z-10 transition-all max-h-[92dvh] sm:max-h-[90vh] overflow-y-auto"
          :class="maxWidthClass || 'max-w-lg'"
          tabindex="-1"
        >
          <div class="flex items-center justify-between gap-4 mb-4 pb-3 border-b border-border">
            <h2 :id="titleId" class="text-base sm:text-lg font-bold text-text tracking-tight">
              {{ title }}
            </h2>
            <IconButton aria-label="Close dialog" size="sm" @click="onClose">
              <svg class="w-4 h-4" fill="none" viewBox="0 0 24 24" stroke="currentColor">
                <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M6 18L18 6M6 6l12 12" />
              </svg>
            </IconButton>
          </div>

          <div class="space-y-4">
            <slot />
          </div>

          <div v-if="$slots.footer" class="mt-6 pt-4 border-t border-border flex items-center justify-end gap-3">
            <slot name="footer" />
          </div>
        </div>
      </div>
    </Transition>
  </Teleport>
</template>

<script setup lang="ts">
import { ref, watch, nextTick } from 'vue';

const props = defineProps<{
  isOpen: boolean;
  title: string;
  maxWidthClass?: string;
}>();

const emit = defineEmits<{
  (e: 'close'): void;
}>();

const modalRef = ref<HTMLElement | null>(null);
const titleId = `modal-title-${Math.random().toString(36).substring(2, 9)}`;

function onClose() {
  emit('close');
}

watch(
  () => props.isOpen,
  (val) => {
    if (val) {
      nextTick(() => {
        modalRef.value?.focus();
      });
    }
  }
);
</script>
