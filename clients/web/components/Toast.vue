<template>
  <div
    class="fixed bottom-5 right-5 z-50 flex flex-col gap-2 max-w-sm w-full pointer-events-none"
    role="region"
    aria-live="polite"
    aria-label="Notifications"
  >
    <TransitionGroup
      enter-active-class="transition duration-200 ease-out"
      enter-from-class="transform translate-y-2 opacity-0"
      enter-to-class="transform translate-y-0 opacity-100"
      leave-active-class="transition duration-150 ease-in"
      leave-from-class="opacity-100"
      leave-to-class="opacity-0"
    >
      <div
        v-for="toast in toasts"
        :key="toast.id"
        role="status"
        class="pointer-events-auto p-3.5 rounded-card bg-surface border border-border-strong shadow-xl flex items-center justify-between gap-3 text-sm font-sans"
        :class="[
          toast.type === 'bad' ? 'border-l-4 border-l-bad text-text' :
          toast.type === 'success' ? 'border-l-4 border-l-ok text-text' :
          toast.type === 'warn' ? 'border-l-4 border-l-warn text-text' :
          'text-text'
        ]"
      >
        <div class="flex items-center gap-2.5 min-w-0">
          <span
            class="w-2 h-2 rounded-full flex-shrink-0"
            :class="[
              toast.type === 'success' ? 'bg-ok' :
              toast.type === 'bad' ? 'bg-bad' :
              toast.type === 'warn' ? 'bg-warn' :
              'bg-info'
            ]"
          />
          <span class="truncate leading-tight">{{ toast.message }}</span>
        </div>

        <div class="flex items-center gap-2 flex-shrink-0">
          <button
            v-if="toast.undoAction"
            type="button"
            class="text-xs font-semibold text-accent hover:underline px-1.5 py-0.5 rounded bg-accent/10"
            @click="handleUndo(toast)"
          >
            {{ toast.undoLabel || 'Undo' }}
          </button>
          <button
            type="button"
            class="text-text-muted hover:text-text p-0.5"
            aria-label="Dismiss notification"
            @click="dismiss(toast.id)"
          >
            <svg class="w-3.5 h-3.5" fill="none" viewBox="0 0 24 24" stroke="currentColor">
              <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M6 18L18 6M6 6l12 12" />
            </svg>
          </button>
        </div>
      </div>
    </TransitionGroup>
  </div>
</template>

<script setup lang="ts">
import { useToast, type ToastItem } from '~/composables/useToast';

const { toasts, dismiss } = useToast();

async function handleUndo(toast: ToastItem) {
  if (toast.undoAction) {
    try {
      await toast.undoAction();
    } finally {
      dismiss(toast.id);
    }
  }
}
</script>
