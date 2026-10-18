<template>
  <Teleport to="body">
    <div
      v-if="store.isTourActive && currentStep"
      class="fixed inset-0 z-50 pointer-events-auto"
      role="dialog"
      aria-modal="true"
      :aria-label="currentStep.title"
    >
      <!-- Semi-transparent overlay backdrop with cutout effect -->
      <div
        class="fixed inset-0 bg-overlay transition-opacity duration-200"
        @click="skipTour"
      />

      <!-- Spotlight bounding box around target element -->
      <div
        v-if="targetRect"
        class="fixed rounded-card border-2 border-accent transition-all duration-300 pointer-events-none z-50 shadow-[0_0_0_9999px_rgba(0,0,0,0.55)]"
        :style="{
          top: `${Math.max(8, targetRect.top - 6)}px`,
          left: `${Math.max(8, targetRect.left - 6)}px`,
          width: `${targetRect.width + 12}px`,
          height: `${targetRect.height + 12}px`,
        }"
      />

      <!-- Popover Card -->
      <div
        class="fixed z-50 w-[92vw] max-w-sm sm:max-w-md bg-surface border border-border-strong rounded-card shadow-2xl p-5 transition-all duration-200"
        :style="popoverStyle"
      >
        <div class="flex items-center justify-between gap-3 mb-2.5 pb-2 border-b border-border">
          <div class="flex items-center gap-2">
            <span class="text-xs font-semibold px-2 py-0.5 rounded-pill bg-accent-solid text-accent-fg">
              {{ currentStepIndex + 1 }} of {{ totalSteps }}
            </span>
            <span class="text-xs text-text-muted font-medium">Quick Tour</span>
          </div>
          <button
            type="button"
            class="text-xs text-text-muted hover:text-text p-1 transition-colors"
            aria-label="Close tour"
            @click="skipTour"
          >
            ✕
          </button>
        </div>

        <h3 class="text-sm sm:text-base font-bold text-text mb-1.5">
          {{ currentStep.title }}
        </h3>
        <p class="text-xs sm:text-sm text-text-muted leading-relaxed mb-4">
          {{ currentStep.content }}
        </p>

        <!-- Actions Footer -->
        <div class="flex items-center justify-between gap-2 pt-2 border-t border-border">
          <Button
            variant="ghost"
            size="sm"
            class="text-xs"
            @click="skipTour"
          >
            Skip
          </Button>

          <div class="flex items-center gap-2">
            <Button
              v-if="currentStep.tryAction"
              variant="secondary"
              size="sm"
              class="text-xs text-accent border-accent/40"
              @click="currentStep.tryAction"
            >
              {{ currentStep.tryActionLabel || 'Try it' }}
            </Button>

            <Button
              v-if="!isFirst"
              variant="secondary"
              size="sm"
              class="text-xs"
              @click="prev"
            >
              Back
            </Button>

            <Button
              variant="primary"
              size="sm"
              class="text-xs"
              @click="next"
            >
              {{ isLast ? 'Finish Tour' : 'Next →' }}
            </Button>
          </div>
        </div>
      </div>
    </div>
  </Teleport>
</template>

<script setup lang="ts">
import { computed } from 'vue';
import { useAutoMemoryStore } from '~/stores/automemory';
import { useCoachTour } from '~/composables/useCoachTour';

const store = useAutoMemoryStore();
const {
  currentStep,
  currentStepIndex,
  totalSteps,
  isFirst,
  isLast,
  targetRect,
  next,
  prev,
  skipTour,
} = useCoachTour();

const popoverStyle = computed(() => {
  if (typeof window === 'undefined' || !targetRect.value) {
    return {
      top: '50%',
      left: '50%',
      transform: 'translate(-50%, -50%)',
    };
  }

  const rect = targetRect.value;
  const popoverWidth = Math.min(window.innerWidth - 32, 400);
  const popoverHeight = 220;

  // Prefer placing below the target; if no space, place above
  let top = rect.top + rect.height + 16;
  if (top + popoverHeight > window.innerHeight - 20) {
    top = Math.max(16, rect.top - popoverHeight - 16);
  }

  let left = rect.left + rect.width / 2 - popoverWidth / 2;
  // Clamp left to window bounds
  left = Math.max(16, Math.min(window.innerWidth - popoverWidth - 16, left));

  return {
    top: `${top}px`,
    left: `${left}px`,
  };
});
</script>
