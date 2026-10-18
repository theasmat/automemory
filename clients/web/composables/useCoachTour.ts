import { ref, computed, watch, onMounted, onUnmounted, nextTick } from 'vue';
import { useRouter } from 'vue-router';
import { useAutoMemoryStore } from '~/stores/automemory';

export interface CoachStep {
  id: string;
  targetSelector: string;
  title: string;
  content: string;
  route?: string;
  tryActionLabel?: string;
  tryAction?: () => Promise<void> | void;
}

const currentStepIndex = ref(0);
const targetRect = ref<{ top: number; left: number; width: number; height: number } | null>(null);

export function useCoachTour() {
  const router = useRouter();
  const store = useAutoMemoryStore();

  const steps: CoachStep[] = [
    {
      id: 'nav',
      targetSelector: '[data-tour="nav-rail"]',
      route: '/',
      title: '1. Six places, one for each question',
      content: 'AutoMemory organizes your work into 6 task areas: Home, Project brain, Agents & locks, Suggestions inbox, Sync & devices, and Safety.',
    },
    {
      id: 'needs-you',
      targetSelector: '[data-tour="needs-you"]',
      route: '/',
      title: '2. Needs you',
      content: 'The only place with attention color: suggestions waiting for review, long-held file locks, and collisions requiring human attention.',
      tryActionLabel: 'View inbox',
      tryAction: () => {
        router.push('/inbox');
      },
    },
    {
      id: 'brain',
      targetSelector: '[data-tour="brain-card"]',
      route: '/',
      title: '3. Project brain',
      content: 'Verified facts, architectural decisions, and lessons shared with every agent session. Decisions outrank notes and are presented first.',
      tryActionLabel: 'Explore brain',
      tryAction: () => {
        router.push('/brain');
      },
    },
    {
      id: 'agents',
      targetSelector: '[data-tour="agents-card"]',
      route: '/',
      title: '4. Agents & locks',
      content: 'Live coordination and advisory file locks. Automatic leases stop concurrent agents from colliding and stepping on the same files.',
      tryActionLabel: 'See locks',
      tryAction: () => {
        router.push('/agents');
      },
    },
    {
      id: 'setup',
      targetSelector: '[data-tour="setup-card"]',
      route: '/',
      title: '5. Getting started checklist',
      content: 'Dynamic checklist tracking connected agents, instruction files, human password, and multi-machine sync. Ticks live as you run commands.',
    },
    {
      id: 'safety',
      targetSelector: '[data-tour="safety-mode"]',
      route: '/',
      title: '6. Safety & modes',
      content: 'Switch between Relaxed, Balanced, and Strict modes. Set a human password to protect sensitive rule changes and approvals.',
      tryActionLabel: 'Open safety',
      tryAction: () => {
        router.push('/safety');
      },
    },
  ];

  const currentStep = computed(() => steps[currentStepIndex.value]);
  const isFirst = computed(() => currentStepIndex.value === 0);
  const isLast = computed(() => currentStepIndex.value === steps.length - 1);
  const totalSteps = steps.length;

  function updateTargetPosition() {
    if (!store.isTourActive || typeof window === 'undefined') return;
    const step = currentStep.value;
    if (!step) return;

    const el = document.querySelector(step.targetSelector);
    if (el) {
      const rect = el.getBoundingClientRect();
      targetRect.value = {
        top: rect.top,
        left: rect.left,
        width: rect.width,
        height: rect.height,
      };
      el.scrollIntoView({ behavior: 'smooth', block: 'center' });
    } else {
      // Fallback center of screen
      targetRect.value = {
        top: window.innerHeight / 2 - 100,
        left: window.innerWidth / 2 - 150,
        width: 300,
        height: 200,
      };
    }
  }

  async function goToStep(idx: number) {
    if (idx < 0 || idx >= steps.length) return;
    currentStepIndex.value = idx;
    const step = steps[idx];
    if (step.route && router.currentRoute.value.path !== step.route) {
      await router.push(step.route);
    }
    await nextTick();
    setTimeout(() => {
      updateTargetPosition();
    }, 150);
  }

  function startTour() {
    store.startTour();
    goToStep(0);
  }

  function next() {
    if (isLast.value) {
      finishTour();
    } else {
      goToStep(currentStepIndex.value + 1);
    }
  }

  function prev() {
    if (!isFirst.value) {
      goToStep(currentStepIndex.value - 1);
    }
  }

  function skipTour() {
    store.stopTour();
    store.updateOnboardingProgress({ action: 'dismiss', flag: 'tour_skipped' });
  }

  function finishTour() {
    store.stopTour();
    store.updateOnboardingProgress({ action: 'mark_done', step_id: 'tour_completed' });
  }

  function handleKeydown(e: KeyboardEvent) {
    if (!store.isTourActive) return;
    if (e.key === 'Escape') {
      e.preventDefault();
      skipTour();
    } else if (e.key === 'ArrowRight' || e.key === 'Enter') {
      e.preventDefault();
      next();
    } else if (e.key === 'ArrowLeft') {
      e.preventDefault();
      prev();
    }
  }

  watch(
    () => store.isTourActive,
    (active) => {
      if (active) {
        window.addEventListener('keydown', handleKeydown);
        window.addEventListener('resize', updateTargetPosition);
        window.addEventListener('scroll', updateTargetPosition);
        goToStep(0);
      } else {
        window.removeEventListener('keydown', handleKeydown);
        window.removeEventListener('resize', updateTargetPosition);
        window.removeEventListener('scroll', updateTargetPosition);
      }
    }
  );

  return {
    steps,
    currentStep,
    currentStepIndex,
    totalSteps,
    isFirst,
    isLast,
    targetRect,
    startTour,
    goToStep,
    next,
    prev,
    skipTour,
    finishTour,
    updateTargetPosition,
  };
}
