<template>
  <Card>
    <!-- Header: one calm line + progress -->
    <div class="flex items-center justify-between gap-4">
      <div class="min-w-0">
        <h3 class="text-[17px] font-semibold text-text">{{ allDone ? 'Setup complete' : title }}</h3>
        <p class="text-sm text-text-muted mt-0.5">
          {{ allDone ? 'Everything is connected and protected.' : `${doneCount} of ${visible.length} done` }}
        </p>
      </div>
      <div class="w-24 h-1.5 rounded-pill bg-surface-2 overflow-hidden flex-shrink-0" role="progressbar" :aria-valuenow="percent" aria-valuemin="0" aria-valuemax="100" :aria-label="`${percent}% of setup done`">
        <div class="h-full rounded-pill bg-ok transition-all duration-500" :style="{ width: percent + '%' }" />
      </div>
    </div>

    <!-- Steps -->
    <ol v-if="!allDone || showAll" class="mt-5 space-y-2">
      <li v-for="(g, gi) in groups" :key="g.id" class="rounded-xl" :class="openId === g.id ? 'bg-surface-2/60' : ''">
        <button
          type="button"
          class="w-full flex items-center gap-3 p-3 text-left rounded-xl hover:bg-surface-2/60 transition-colors"
          :aria-expanded="openId === g.id"
          @click="openId = openId === g.id ? '' : g.id"
        >
          <span
            class="w-7 h-7 rounded-full grid place-items-center text-xs font-semibold flex-shrink-0 transition-colors"
            :class="g.done ? 'bg-ok text-white' : openId === g.id || g.active ? 'bg-accent-solid text-accent-fg' : 'bg-surface-2 text-text-muted'"
          >
            <svg v-if="g.done" class="w-3.5 h-3.5" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="3" stroke-linecap="round" stroke-linejoin="round"><path d="M5 12l5 5L20 7" /></svg>
            <span v-else>{{ gi + 1 }}</span>
          </span>
          <span class="min-w-0 flex-1">
            <span class="block text-[15px] font-medium text-text">{{ g.title }}</span>
            <span class="block text-sm text-text-muted truncate">{{ g.blurb }}</span>
          </span>
          <span class="text-xs text-text-muted flex-shrink-0">{{ g.items.filter((i) => i.done).length }}/{{ g.items.length }}</span>
          <svg class="w-4 h-4 text-text-muted transition-transform flex-shrink-0" :class="openId === g.id ? 'rotate-90' : ''" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round"><path d="M9 6l6 6-6 6" /></svg>
        </button>

        <div v-if="openId === g.id" class="px-3 pb-3 pl-13 space-y-1">
          <div v-for="item in g.items" :key="item.id" class="flex items-start gap-3 py-2 sm:ml-10">
            <span
              class="mt-1 w-4 h-4 rounded-full grid place-items-center flex-shrink-0"
              :class="item.done ? 'bg-ok text-white' : 'border border-border-strong'"
            >
              <svg v-if="item.done" class="w-2.5 h-2.5" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="4" stroke-linecap="round" stroke-linejoin="round"><path d="M5 12l5 5L20 7" /></svg>
            </span>
            <div class="min-w-0 flex-1">
              <div class="text-sm font-medium" :class="item.done ? 'text-text-muted' : 'text-text'">{{ item.title }}</div>
              <p v-if="item.why && !item.done" class="text-sm text-text-muted mt-0.5">{{ item.why }}</p>

              <div v-if="!item.done" class="mt-2 flex flex-wrap items-center gap-2">
                <NuxtLink v-if="item.link" :to="item.link">
                  <Button size="sm" variant="primary">Open</Button>
                </NuxtLink>
                <button
                  v-if="item.command"
                  type="button"
                  class="text-sm text-accent hover:underline"
                  @click="cmdOpen = cmdOpen === item.id ? '' : item.id"
                >
                  {{ cmdOpen === item.id ? 'Hide command' : 'Use the terminal' }}
                </button>
              </div>
              <div v-if="item.command && cmdOpen === item.id" class="mt-2">
                <CopyCommand :command="item.command" />
              </div>
            </div>
          </div>
        </div>
      </li>
    </ol>

    <button v-if="allDone" type="button" class="mt-3 text-sm text-accent hover:underline" @click="showAll = !showAll">
      {{ showAll ? 'Hide steps' : 'Review steps' }}
    </button>
  </Card>
</template>

<script setup lang="ts">
import { computed, ref, watch } from 'vue';

export interface ChecklistItem {
  id: string;
  title: string;
  done: boolean;
  why?: string;
  command?: string;
  link?: string;
}

const props = defineProps<{ title: string; items: ChecklistItem[] }>();

/** The setup steps, in the order a person does them. Unknown ids fall into "More". */
const GROUPS = [
  { id: 'connect', title: 'Connect your agent', blurb: 'Link a coding agent and share instructions', ids: ['agent', 'instruction_files'] },
  { id: 'teach', title: 'Teach it something', blurb: 'Give every agent the same knowledge', ids: ['remembered', 'decided'] },
  { id: 'protect', title: 'Stay in control', blurb: 'Locks, review, password and sync', ids: ['saw_lock_block', 'reviewed_suggestion', 'password', 'sync'] },
];
// "connected" repeats "agent" in plain words, so it is not listed twice.
const HIDDEN = new Set(['connected']);

const visible = computed(() => props.items.filter((i) => !HIDDEN.has(i.id)));
const doneCount = computed(() => visible.value.filter((i) => i.done).length);
const percent = computed(() => (visible.value.length ? Math.round((doneCount.value / visible.value.length) * 100) : 0));
const allDone = computed(() => visible.value.length > 0 && doneCount.value === visible.value.length);

const groups = computed(() => {
  const known = new Set(GROUPS.flatMap((g) => g.ids));
  const list = GROUPS.map((g) => ({ ...g, items: visible.value.filter((i) => g.ids.includes(i.id)) }));
  const rest = visible.value.filter((i) => !known.has(i.id));
  if (rest.length) list.push({ id: 'more', title: 'More', blurb: 'Other steps', ids: [], items: rest });
  const filled = list.filter((g) => g.items.length);
  const firstOpen = filled.find((g) => g.items.some((i) => !i.done))?.id;
  return filled.map((g) => ({ ...g, done: g.items.every((i) => i.done), active: g.id === firstOpen }));
});

const openId = ref('');
const cmdOpen = ref('');
const showAll = ref(false);

// Open the first unfinished step by default; follow along as steps complete.
watch(
  () => groups.value.find((g) => g.active)?.id,
  (id) => {
    if (id) openId.value = id;
  },
  { immediate: true }
);
</script>
