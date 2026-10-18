import { ref, computed, nextTick, watch } from 'vue';
import { useRouter } from 'vue-router';
import { useAutoMemoryStore } from '~/stores/automemory';

export type Placement = 'top' | 'bottom' | 'left' | 'right';

export interface CoachStep {
  id: string;
  /** CSS selector of the element to spotlight. Omit for a centered card (welcome, finish). */
  target?: string;
  title: string;
  body: string;
  route?: string;
  placement?: Placement;
  /** Optional extra button on the step (secondary action). */
  action?: { label: string; to?: string; run?: () => void };
}

export interface TargetRect {
  top: number;
  left: number;
  width: number;
  height: number;
}

const index = ref(0);
const rect = ref<TargetRect | null>(null);

/** Tour steps, in the order a new user should meet things. Keep copy to one or two short lines. */
const STEPS: CoachStep[] = [
  {
    id: 'welcome',
    title: 'Welcome to AutoMemory',
    body: 'Your coding agents now share one memory. Take a one-minute look around, or skip and explore on your own.',
    route: '/',
  },
  {
    id: 'nav',
    target: '[data-tour="nav-rail"]',
    title: 'Six places, six questions',
    body: 'Home shows what needs you. Project brain is what agents know. The rest cover agents, suggestions, sync and safety.',
    route: '/',
    placement: 'right',
  },
  {
    id: 'setup',
    target: '[data-tour="setup-card"]',
    title: 'Start here',
    body: 'Three short steps: connect an agent, teach it something, protect your work. Each one ticks itself when done.',
    route: '/',
    placement: 'bottom',
  },
  {
    id: 'search',
    target: '[data-tour="search"]',
    title: 'Find or do anything',
    body: 'Press ⌘K to search notes or run a command, like adding a note or syncing.',
    route: '/',
    placement: 'bottom',
  },
  {
    id: 'brain',
    target: '[data-tour="brain-header"]',
    title: 'Teach every agent once',
    body: 'Add a note here and every agent on this project starts from it. Decisions are shown first.',
    route: '/brain',
    placement: 'bottom',
  },
  {
    id: 'agents',
    target: '[data-tour="agents-header"]',
    title: 'See who is working',
    body: 'When two agents want the same file, one waits. You will see who holds what, and why.',
    route: '/agents',
    placement: 'bottom',
  },
  {
    id: 'inbox',
    target: '[data-tour="inbox-header"]',
    title: 'You stay in charge',
    body: 'Agents suggest what to remember. Approve or reject here before it becomes shared knowledge.',
    route: '/inbox',
    placement: 'bottom',
  },
  {
    id: 'safety',
    target: '[data-tour="safety-modes"]',
    title: 'Choose how careful to be',
    body: 'Relaxed warns, Balanced blocks risky commands, Strict requires a file lock for every edit.',
    route: '/safety',
    placement: 'top',
  },
  {
    id: 'done',
    title: 'You are all set',
    body: 'Add your first note to see it work. You can replay this tour any time from the help menu.',
    route: '/',
    action: { label: 'Add a note', to: '/brain' },
  },
];

/** The first matching element that is actually on screen (desktop and mobile layouts share a selector). */
function findVisible(selector?: string): HTMLElement | null {
  if (!selector || typeof document === 'undefined') return null;
  return Array.from(document.querySelectorAll<HTMLElement>(selector)).find((e) => visible(e)) ?? null;
}

function visible(el: Element | null): el is HTMLElement {
  if (!el) return false;
  const r = (el as HTMLElement).getBoundingClientRect();
  return r.width > 0 && r.height > 0;
}

export function useCoachTour() {
  const router = useRouter();
  const store = useAutoMemoryStore();

  const steps = STEPS;
  const current = computed(() => steps[index.value]);
  const isFirst = computed(() => index.value === 0);
  const isLast = computed(() => index.value === steps.length - 1);

  /** Measure the spotlight target; null means "show a centered card". */
  function measure() {
    const step = current.value;
    if (!step?.target || typeof document === 'undefined') {
      rect.value = null;
      return;
    }
    const el = findVisible(step.target);
    if (!el) {
      rect.value = null;
      return;
    }
    const r = el.getBoundingClientRect();
    rect.value = { top: r.top, left: r.left, width: r.width, height: r.height };
  }

  async function show(i: number) {
    if (i < 0 || i >= steps.length) return;
    index.value = i;
    const step = steps[i];
    if (step.route && router.currentRoute.value.path !== step.route) {
      await router.push(step.route);
    }
    // Wait for the page to render, then bring the target into view and measure it.
    await nextTick();
    for (let tries = 0; tries < 10; tries++) {
      if (!step.target || findVisible(step.target)) break;
      await new Promise((r) => setTimeout(r, 60));
    }
    const el = findVisible(step.target);
    if (el) {
      el.scrollIntoView({ block: 'center', inline: 'nearest', behavior: 'auto' });
    }
    measure();
  }

  function start() {
    store.startTour();
    show(0);
  }

  function next() {
    if (isLast.value) finish();
    else show(index.value + 1);
  }

  function prev() {
    if (!isFirst.value) show(index.value - 1);
  }

  function skip() {
    store.stopTour();
    store.updateOnboardingProgress({ action: 'dismiss', flag: 'tour_skipped' });
  }

  function finish() {
    store.stopTour();
    store.updateOnboardingProgress({ action: 'mark_done', step_id: 'tour_completed' });
  }

  // The store starts the tour (welcome card, help drawer); begin at the first step.
  watch(
    () => store.isTourActive,
    (active) => {
      if (active) show(0);
    }
  );

  return { steps, current, index, isFirst, isLast, rect, start, show, next, prev, skip, finish, measure };
}
