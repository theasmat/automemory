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
/** The step whose target has already been scrolled into view (so polling does not keep scrolling). */
let scrolledFor = '';

/** Tour steps, in the order a new user should meet things. Keep copy to one or two short lines. */
const INTRO_STEPS: CoachStep[] = [
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

/** Walks every page once, using the sample project so each page has something to look at. */
const SHOWCASE_STEPS: CoachStep[] = [
  {
    id: 's-welcome',
    title: 'Your sample project is ready',
    body: 'We added a pretend online shop with notes, decisions, agents and locks. Take a 2-minute walk through every page.',
    route: '/',
  },
  {
    id: 's-home',
    target: '[data-tour="agents-card"]',
    title: 'Home: the big picture',
    body: 'Agents at work, what they know, and what needs you. Click any tile to open it.',
    route: '/',
    placement: 'bottom',
  },
  {
    id: 's-brain',
    target: 'main h1',
    title: 'Project brain',
    body: 'Everything every agent starts from: 3 decisions and 8 notes here. Add your own with “Add note”.',
    route: '/brain',
    placement: 'bottom',
  },
  {
    id: 's-agents',
    target: 'main h1',
    title: 'Agents & locks',
    body: 'Claude and Cursor each hold a file right now, so they cannot overwrite each other. A hand-off note is waiting below.',
    route: '/agents',
    placement: 'bottom',
  },
  {
    id: 's-inbox',
    target: 'main h1',
    title: 'Inbox',
    body: 'Three suggestions from agents wait for your OK. Nothing becomes shared knowledge until you approve it.',
    route: '/inbox',
    placement: 'bottom',
  },
  {
    id: 's-sync',
    target: 'main h1',
    title: 'Sync & devices',
    body: 'Share this memory with your other computer through a private git repository, encrypted end to end.',
    route: '/sync',
    placement: 'bottom',
  },
  {
    id: 's-safety',
    target: '[data-tour="safety-modes"]',
    title: 'Safety',
    body: 'Pick how careful agents must be, set a password, and test what would be blocked.',
    route: '/safety',
    placement: 'top',
  },
  {
    id: 's-skills',
    target: 'main h1',
    title: 'Learned skills',
    body: 'Reusable how-tos agents picked up, like “Test Stripe webhooks locally”.',
    route: '/skills',
    placement: 'bottom',
  },
  {
    id: 's-traces',
    target: 'main h1',
    title: 'What worked',
    body: 'Step-by-step runs that succeeded, so the next agent can repeat them.',
    route: '/traces',
    placement: 'bottom',
  },
  {
    id: 's-failures',
    target: 'main h1',
    title: 'Problems & fixes',
    body: 'What went wrong and how it was solved. One problem here is still open.',
    route: '/failures',
    placement: 'bottom',
  },
  {
    id: 's-replay',
    title: 'Session history',
    body: 'Replay what each agent did, step by step. Open it any time from the sidebar under Advanced.',
    route: '/replay',
  },
  {
    id: 's-docs',
    target: 'main h1',
    title: 'Documents',
    body: 'Project docs your agents read, kept in sync with the project brain.',
    route: '/docs',
    placement: 'bottom',
  },
  {
    id: 's-rules',
    target: 'main h1',
    title: 'Rules of the project',
    body: 'The standing rules every agent must follow in this project.',
    route: '/constitution',
    placement: 'bottom',
  },
  {
    id: 's-graph',
    target: 'main h1',
    title: 'Knowledge graph',
    body: 'How files, ideas and decisions connect. It grows as agents work.',
    route: '/graph',
    placement: 'bottom',
  },
  {
    id: 's-recall',
    target: 'main h1',
    title: 'Recall settings',
    body: 'Choose how agents search memory: simple keyword search, or smarter hybrid search.',
    route: '/recall',
    placement: 'bottom',
  },
  {
    id: 's-done',
    title: 'That is the whole app',
    body: 'Explore freely. Remove the sample data any time from the banner at the top, or right here.',
    route: '/',
    action: { label: 'Remove sample data', run: () => useAutoMemoryStore().removeDemoProject() },
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

  const steps = computed(() => (store.tourKind === 'showcase' ? SHOWCASE_STEPS : INTRO_STEPS));
  const current = computed(() => steps.value[index.value]);
  const isFirst = computed(() => index.value === 0);
  const isLast = computed(() => index.value === steps.value.length - 1);

  /**
   * Measure the spotlight target; null means "show a centered card". Called on a timer while the
   * tour is open, so a page that renders late (data still loading) is picked up as soon as the
   * target appears, and the highlight follows layout shifts.
   */
  function measure() {
    const step = current.value;
    if (!step?.target || typeof document === 'undefined') {
      if (rect.value) rect.value = null;
      return;
    }
    const el = findVisible(step.target);
    if (!el) {
      if (rect.value) rect.value = null;
      return;
    }
    if (scrolledFor !== step.id) {
      scrolledFor = step.id;
      el.scrollIntoView({ block: 'center', inline: 'nearest', behavior: 'auto' });
    }
    const r = el.getBoundingClientRect();
    const next = { top: r.top, left: r.left, width: r.width, height: r.height };
    const prev = rect.value;
    if (!prev || prev.top !== next.top || prev.left !== next.left || prev.width !== next.width || prev.height !== next.height) {
      rect.value = next;
    }
  }

  async function show(i: number) {
    if (i < 0 || i >= steps.value.length) return;
    index.value = i;
    const step = steps.value[i];
    if (step.route && router.currentRoute.value.path !== step.route) {
      await router.push(step.route);
    }
    // Wait (up to 4 s) for the page to render its target, then measure; the timer keeps following it.
    await nextTick();
    scrolledFor = '';
    rect.value = null;
    for (let tries = 0; tries < 40; tries++) {
      if (!step.target || findVisible(step.target)) break;
      await new Promise((r) => setTimeout(r, 100));
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
