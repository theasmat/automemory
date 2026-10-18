<template>
  <section
    v-if="shouldShow"
    class="relative overflow-hidden rounded-card border border-border/70 bg-surface shadow-card px-6 py-8 sm:px-10 sm:py-10 text-center"
  >
    <div class="absolute inset-x-0 -top-24 h-48 bg-accent-solid/10 blur-3xl pointer-events-none" aria-hidden="true" />

    <button
      type="button"
      class="absolute top-4 right-4 w-8 h-8 grid place-items-center rounded-full text-text-muted hover:bg-surface-2 hover:text-text"
      aria-label="Dismiss"
      @click="dismiss"
    >
      <svg viewBox="0 0 24 24" class="w-4 h-4" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round"><path d="M6 6l12 12M18 6L6 18" /></svg>
    </button>

    <div class="relative">
      <div class="mx-auto w-12 h-12 rounded-2xl bg-accent-solid text-accent-fg grid place-items-center text-xl font-bold shadow-card">A</div>
      <h2 class="mt-5 text-2xl sm:text-3xl font-semibold tracking-tight text-text">Welcome to AutoMemory</h2>
      <p class="mt-2 text-base text-text-muted max-w-md mx-auto">
        One shared memory for all your coding agents. Learn it in a minute.
      </p>

      <div class="mt-7 flex flex-col sm:flex-row items-center justify-center gap-3">
        <Button variant="primary" size="lg" @click="onStartTour">Take the tour</Button>
        <Button variant="secondary" size="lg" :loading="isSeeding" @click="onSeedSample">Try with sample data</Button>
      </div>
      <button type="button" class="mt-4 text-sm text-text-muted hover:text-text" @click="dismiss">
        Skip for now
      </button>
    </div>
  </section>
</template>

<script setup lang="ts">
import { ref, computed } from 'vue';
import { useAutoMemoryStore } from '~/stores/automemory';

const store = useAutoMemoryStore();
const isSeeding = ref(false);

const shouldShow = computed(
  () =>
    !store.onboardingState.dismissed &&
    !store.onboardingState.dismissed_flags['welcome_card'] &&
    !store.onboardingState.completed_steps.includes('tour_completed')
);

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
