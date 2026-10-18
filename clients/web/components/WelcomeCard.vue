<template>
  <div
    v-if="shouldShow"
    class="p-5 sm:p-6 rounded-card bg-surface border-2 border-accent/40 shadow-card relative overflow-hidden transition-all"
  >
    <!-- Background highlight pattern -->
    <div class="absolute -right-10 -bottom-10 w-48 h-48 bg-accent/5 rounded-full blur-2xl pointer-events-none" />

    <div class="flex items-start justify-between gap-4">
      <div class="space-y-1.5 max-w-2xl">
        <div class="flex items-center gap-2">
          <span class="px-2 py-0.5 rounded-pill text-xs font-semibold bg-accent-solid text-accent-fg">
            First Run
          </span>
          <h2 class="text-base sm:text-lg font-bold text-text tracking-tight">
            Let's get you set up (5 min)
          </h2>
        </div>
        <p class="text-xs sm:text-sm text-text-muted leading-relaxed">
          AutoMemory shares verified facts, architectural decisions, and file locks between your coding agents (Claude Code, Cursor, Codex, Antigravity) with zero staleness.
        </p>
      </div>

      <button
        type="button"
        class="text-text-muted hover:text-text p-1 text-xs"
        aria-label="Dismiss welcome card"
        title="Dismiss"
        @click="dismiss"
      >
        ✕
      </button>
    </div>

    <!-- 3 Action Paths -->
    <div class="mt-4 pt-3 border-t border-border flex flex-wrap items-center gap-2 sm:gap-3">
      <Button
        variant="primary"
        size="sm"
        class="text-xs sm:text-sm font-semibold"
        @click="onStartTour"
      >
        <template #icon>
          <svg class="w-4 h-4 text-current" fill="none" viewBox="0 0 24 24" stroke="currentColor">
            <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M14.752 11.168l-3.197-2.132A1 1 0 0010 9.87v4.263a1 1 0 001.555.832l3.197-2.132a1 1 0 000-1.664z" />
            <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M21 12a9 9 0 11-18 0 9 9 0 0118 0z" />
          </svg>
        </template>
        Take the interactive tour
      </Button>

      <Button
        variant="secondary"
        size="sm"
        class="text-xs sm:text-sm"
        :loading="isSeeding"
        @click="onSeedSample"
      >
        <template #icon>
          <svg class="w-4 h-4 text-accent" fill="none" viewBox="0 0 24 24" stroke="currentColor">
            <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M19 11H5m14 0a2 2 0 012 2v6a2 2 0 01-2 2H5a2 2 0 01-2-2v-6a2 2 0 012-2m14 0V9a2 2 0 00-2-2M5 11V9a2 2 0 012-2m0 0V5a2 2 0 012-2h6a2 2 0 012 2v2M7 7h10" />
          </svg>
        </template>
        Try with sample data
      </Button>

      <Button
        variant="ghost"
        size="sm"
        class="text-xs text-text-muted hover:text-text ml-auto"
        @click="dismiss"
      >
        Skip, I know what I'm doing
      </Button>
    </div>
  </div>
</template>

<script setup lang="ts">
import { ref, computed } from 'vue';
import { useAutoMemoryStore } from '~/stores/automemory';

const store = useAutoMemoryStore();
const isSeeding = ref(false);

const shouldShow = computed(() => {
  return (
    !store.onboardingState.dismissed &&
    !store.onboardingState.dismissed_flags['welcome_card'] &&
    !store.onboardingState.completed_steps.includes('tour_completed')
  );
});

function onStartTour() {
  store.startTour();
}

async function onSeedSample() {
  isSeeding.value = true;
  try {
    await store.seedDemoProject();
  } finally {
    isSeeding.value = false;
  }
}

function dismiss() {
  store.updateOnboardingProgress({ action: 'dismiss', flag: 'welcome_card' });
}
</script>
