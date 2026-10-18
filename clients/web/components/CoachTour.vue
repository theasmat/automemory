<template>
  <Teleport to="body">
    <div
      v-if="store.isTourActive && current"
      class="tour-root"
      role="dialog"
      aria-modal="true"
      :aria-labelledby="`tour-title-${current.id}`"
      :aria-describedby="`tour-body-${current.id}`"
    >
      <!-- Dimmed page with a soft, animated cut-out around the target -->
      <div v-if="rect" class="tour-spot" :style="spotStyle" />
      <div v-else class="tour-dim" />

      <div
        ref="card"
        class="tour-card"
        :class="{ 'tour-card--center': !rect }"
        :style="cardStyle"
        tabindex="-1"
      >
        <span v-if="rect" class="tour-arrow" :class="`tour-arrow--${placement}`" :style="arrowStyle" aria-hidden="true" />

        <button type="button" class="tour-close" aria-label="Close tour" @click="skip">
          <svg viewBox="0 0 24 24" class="w-4 h-4" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round">
            <path d="M6 6l12 12M18 6L6 18" />
          </svg>
        </button>

        <h3 :id="`tour-title-${current.id}`" class="tour-title">{{ current.title }}</h3>
        <p :id="`tour-body-${current.id}`" class="tour-body">{{ current.body }}</p>

        <div class="tour-foot">
          <div class="tour-dots" role="tablist" aria-label="Tour progress">
            <button
              v-for="(s, i) in steps"
              :key="s.id"
              type="button"
              role="tab"
              class="tour-dot"
              :class="{ 'tour-dot--on': i === index, 'tour-dot--done': i < index }"
              :aria-selected="i === index"
              :aria-label="`Step ${i + 1} of ${steps.length}: ${s.title}`"
              @click="show(i)"
            />
          </div>

          <div class="tour-actions">
            <button v-if="!isLast" type="button" class="tour-link" @click="skip">Skip</button>
            <button v-if="!isFirst" type="button" class="tour-btn tour-btn--ghost" @click="prev">Back</button>
            <button
              v-if="current.action"
              type="button"
              class="tour-btn tour-btn--ghost"
              @click="runAction"
            >
              {{ current.action.label }}
            </button>
            <button ref="primary" type="button" class="tour-btn tour-btn--primary" @click="next">
              {{ isLast ? 'Done' : isFirst ? 'Start tour' : 'Next' }}
            </button>
          </div>
        </div>

        <p class="sr-only" aria-live="polite">Step {{ index + 1 }} of {{ steps.length }}: {{ current.title }}</p>
      </div>
    </div>
  </Teleport>
</template>

<script setup lang="ts">
import { computed, nextTick, onBeforeUnmount, ref, watch } from 'vue';
import { useRouter } from 'vue-router';
import { useAutoMemoryStore } from '~/stores/automemory';
import { useCoachTour, type Placement } from '~/composables/useCoachTour';

const store = useAutoMemoryStore();
const router = useRouter();
const { steps, current, index, isFirst, isLast, rect, show, next, prev, skip, finish, measure } = useCoachTour();

const card = ref<HTMLElement | null>(null);
const primary = ref<HTMLElement | null>(null);
const size = ref({ w: 360, h: 180 });
const view = ref({ w: 1024, h: 768 });
const PAD = 8; // breathing room around the target
const GAP = 14; // distance from target to card
const EDGE = 12; // minimum distance to the viewport edge

let previouslyFocused: HTMLElement | null = null;

function readSizes() {
  if (typeof window === 'undefined') return;
  view.value = { w: window.innerWidth, h: window.innerHeight };
  if (card.value) size.value = { w: card.value.offsetWidth, h: card.value.offsetHeight };
}

const spotStyle = computed(() => {
  const r = rect.value;
  if (!r) return {};
  return {
    top: `${r.top - PAD}px`,
    left: `${r.left - PAD}px`,
    width: `${r.width + PAD * 2}px`,
    height: `${r.height + PAD * 2}px`,
  };
});

/** Choose the side with room (preferred side first), then clamp inside the viewport. */
const layout = computed(() => {
  const r = rect.value;
  const { w, h } = size.value;
  const vw = view.value.w;
  const vh = view.value.h;
  if (!r) return { placement: 'bottom' as Placement, top: 0, left: 0 };

  const t = r.top - PAD;
  const l = r.left - PAD;
  const tw = r.width + PAD * 2;
  const th = r.height + PAD * 2;
  const room: Record<Placement, number> = {
    bottom: vh - (t + th) - GAP - EDGE - h,
    top: t - GAP - EDGE - h,
    right: vw - (l + tw) - GAP - EDGE - w,
    left: l - GAP - EDGE - w,
  };
  const pref = current.value?.placement ?? 'bottom';
  const order: Placement[] = [pref, 'bottom', 'top', 'right', 'left'];
  const placement = order.find((p) => room[p] >= 0) ?? order.sort((a, b) => room[b] - room[a])[0];

  let top = 0;
  let left = 0;
  if (placement === 'bottom') {
    top = t + th + GAP;
    left = l + tw / 2 - w / 2;
  } else if (placement === 'top') {
    top = t - GAP - h;
    left = l + tw / 2 - w / 2;
  } else if (placement === 'right') {
    left = l + tw + GAP;
    top = t + th / 2 - h / 2;
  } else {
    left = l - GAP - w;
    top = t + th / 2 - h / 2;
  }
  left = Math.max(EDGE, Math.min(vw - w - EDGE, left));
  top = Math.max(EDGE, Math.min(vh - h - EDGE, top));
  return { placement, top, left };
});

const placement = computed(() => layout.value.placement);
const cardStyle = computed(() =>
  rect.value ? { top: `${layout.value.top}px`, left: `${layout.value.left}px` } : {}
);

/** The arrow points at the middle of the target, kept inside the card's rounded corners. */
const arrowStyle = computed(() => {
  const r = rect.value;
  if (!r) return {};
  const { top, left, placement: p } = layout.value;
  if (p === 'top' || p === 'bottom') {
    const x = r.left + r.width / 2 - left;
    return { left: `${Math.max(20, Math.min(size.value.w - 20, x))}px` };
  }
  const y = r.top + r.height / 2 - top;
  return { top: `${Math.max(20, Math.min(size.value.h - 20, y))}px` };
});

function runAction() {
  const a = current.value?.action;
  if (!a) return;
  finish();
  if (a.to) router.push(a.to);
  a.run?.();
}

function onKeydown(e: KeyboardEvent) {
  if (e.key === 'Escape') {
    e.preventDefault();
    skip();
  } else if (e.key === 'ArrowRight') {
    e.preventDefault();
    next();
  } else if (e.key === 'ArrowLeft') {
    e.preventDefault();
    prev();
  } else if (e.key === 'Tab' && card.value) {
    // Keep keyboard focus inside the card while the tour is open.
    const items = Array.from(card.value.querySelectorAll<HTMLElement>('button')).filter((b) => b.offsetParent !== null);
    if (!items.length) return;
    const first = items[0];
    const last = items[items.length - 1];
    if (e.shiftKey && document.activeElement === first) {
      e.preventDefault();
      last.focus();
    } else if (!e.shiftKey && document.activeElement === last) {
      e.preventDefault();
      first.focus();
    }
  }
}

function onViewportChange() {
  readSizes();
  measure();
}

watch(
  () => store.isTourActive,
  async (active) => {
    if (active) {
      previouslyFocused = document.activeElement as HTMLElement | null;
      window.addEventListener('keydown', onKeydown);
      window.addEventListener('resize', onViewportChange);
      window.addEventListener('scroll', onViewportChange, { passive: true, capture: true });
      await nextTick();
      readSizes();
    } else {
      window.removeEventListener('keydown', onKeydown);
      window.removeEventListener('resize', onViewportChange);
      window.removeEventListener('scroll', onViewportChange, true);
      previouslyFocused?.focus?.();
    }
  },
  { immediate: true }
);

// Re-read the card size and move focus to the main button after every step.
watch([index, rect], async () => {
  await nextTick();
  readSizes();
  primary.value?.focus({ preventScroll: true });
});

onBeforeUnmount(() => {
  window.removeEventListener('keydown', onKeydown);
  window.removeEventListener('resize', onViewportChange);
  window.removeEventListener('scroll', onViewportChange, true);
});
</script>

<style scoped>
.tour-root {
  position: fixed;
  inset: 0;
  z-index: 60;
}
/* Whole-page dim for steps without a target */
.tour-dim {
  position: fixed;
  inset: 0;
  background: var(--overlay);
  -webkit-backdrop-filter: blur(2px);
  backdrop-filter: blur(2px);
}
/* The cut-out: a transparent box whose huge shadow dims everything else. It glides between steps. */
.tour-spot {
  position: fixed;
  border-radius: 14px;
  box-shadow: 0 0 0 9999px var(--overlay), 0 0 0 2px var(--accent);
  pointer-events: none;
  transition: top 0.35s cubic-bezier(0.22, 1, 0.36, 1), left 0.35s cubic-bezier(0.22, 1, 0.36, 1),
    width 0.35s cubic-bezier(0.22, 1, 0.36, 1), height 0.35s cubic-bezier(0.22, 1, 0.36, 1);
}
.tour-card {
  position: fixed;
  width: min(360px, calc(100vw - 24px));
  padding: 20px 20px 16px;
  background: var(--surface);
  color: var(--text);
  border: 1px solid var(--border);
  border-radius: 18px;
  box-shadow: 0 18px 50px rgba(0, 0, 0, 0.28), 0 2px 8px rgba(0, 0, 0, 0.12);
  outline: none;
  transition: top 0.35s cubic-bezier(0.22, 1, 0.36, 1), left 0.35s cubic-bezier(0.22, 1, 0.36, 1);
}
.tour-card--center {
  top: 50%;
  left: 50%;
  transform: translate(-50%, -50%);
  width: min(420px, calc(100vw - 24px));
  padding: 28px 28px 20px;
  text-align: center;
}
.tour-card--center .tour-foot {
  flex-direction: column;
  gap: 16px;
}
.tour-title {
  font-size: 17px;
  font-weight: 600;
  letter-spacing: -0.01em;
  padding-right: 28px;
}
.tour-card--center .tour-title {
  font-size: 22px;
  padding-right: 0;
}
.tour-body {
  margin-top: 6px;
  font-size: 14px;
  line-height: 1.5;
  color: var(--text-muted);
}
.tour-close {
  position: absolute;
  top: 12px;
  right: 12px;
  display: grid;
  place-items: center;
  width: 28px;
  height: 28px;
  border-radius: 999px;
  color: var(--text-muted);
}
.tour-close:hover {
  background: var(--surface-2);
  color: var(--text);
}
.tour-foot {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 12px;
  margin-top: 18px;
}
.tour-dots {
  display: flex;
  gap: 6px;
}
.tour-dot {
  width: 7px;
  height: 7px;
  padding: 0;
  border-radius: 999px;
  background: var(--border-strong);
  opacity: 0.55;
  transition: width 0.25s ease, background 0.25s ease, opacity 0.25s ease;
}
.tour-dot--done {
  background: var(--accent);
  opacity: 0.6;
}
.tour-dot--on {
  width: 20px;
  background: var(--accent);
  opacity: 1;
}
.tour-actions {
  display: flex;
  align-items: center;
  gap: 6px;
}
.tour-link {
  font-size: 13px;
  color: var(--text-muted);
  padding: 6px 8px;
}
.tour-link:hover {
  color: var(--text);
}
.tour-btn {
  font-size: 13px;
  font-weight: 600;
  padding: 7px 14px;
  border-radius: 999px;
  transition: background 0.15s ease, opacity 0.15s ease;
}
.tour-btn--ghost {
  background: var(--surface-2);
  color: var(--text);
}
.tour-btn--ghost:hover {
  background: var(--border);
}
.tour-btn--primary {
  background: var(--accent-solid);
  color: var(--accent-fg);
}
.tour-btn--primary:hover {
  opacity: 0.92;
}
/* Arrow: a rotated square that shares the card's background and border */
.tour-arrow {
  position: absolute;
  width: 14px;
  height: 14px;
  background: var(--surface);
  border: 1px solid var(--border);
  transform: translate(-50%, -50%) rotate(45deg);
}
.tour-arrow--bottom {
  top: 0;
  border-right: 0;
  border-bottom: 0;
}
.tour-arrow--top {
  top: 100%;
  border-left: 0;
  border-top: 0;
}
.tour-arrow--right {
  left: 0;
  border-right: 0;
  border-top: 0;
}
.tour-arrow--left {
  left: 100%;
  border-left: 0;
  border-bottom: 0;
}
@media (prefers-reduced-motion: reduce) {
  .tour-spot,
  .tour-card,
  .tour-dot {
    transition: none;
  }
}
</style>
