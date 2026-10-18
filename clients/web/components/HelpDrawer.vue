<template>
  <Drawer
    :is-open="store.isHelpDrawerOpen"
    title="Help & Guided Learning"
    description="Concepts, shortcuts, guided topics, and tour replay"
    width-class="max-w-md sm:max-w-lg"
    @close="store.closeHelpDrawer()"
  >
    <div class="space-y-6">
      <!-- Quick Action Buttons -->
      <div class="grid grid-cols-2 gap-2.5">
        <Button
          variant="primary"
          size="sm"
          class="w-full text-xs font-semibold"
          @click="replayTour"
        >
          <template #icon>
            <svg class="w-3.5 h-3.5" fill="none" viewBox="0 0 24 24" stroke="currentColor">
              <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M14.752 11.168l-3.197-2.132A1 1 0 0010 9.87v4.263a1 1 0 001.555.832l3.197-2.132a1 1 0 000-1.664z" />
              <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M21 12a9 9 0 11-18 0 9 9 0 0118 0z" />
            </svg>
          </template>
          Replay Tour
        </Button>

        <Button
          variant="secondary"
          size="sm"
          class="w-full text-xs"
          @click="copyDiagnostics"
        >
          <template #icon>
            <svg class="w-3.5 h-3.5 text-accent" fill="none" viewBox="0 0 24 24" stroke="currentColor">
              <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M9 5H7a2 2 0 00-2 2v12a2 2 0 002 2h10a2 2 0 002-2V7a2 2 0 00-2-2h-2M9 5a2 2 0 002 2h2a2 2 0 002-2M9 5a2 2 0 012-2h2a2 2 0 012 2" />
            </svg>
          </template>
          {{ copiedDiag ? 'Copied Diagnostics!' : 'Copy Diagnostics' }}
        </Button>
      </div>

      <!-- Section: Plain-English Glossary -->
      <div>
        <h3 class="text-xs font-bold uppercase tracking-wider text-text-muted mb-2.5">
          Plain-English Glossary
        </h3>
        <div class="space-y-2">
          <div
            v-for="item in glossary"
            :key="item.term"
            class="p-2.5 rounded-card bg-surface-2 border border-border text-xs"
          >
            <div class="font-bold text-text">{{ item.term }}</div>
            <p class="text-text-muted mt-0.5 leading-relaxed">{{ item.def }}</p>
          </div>
        </div>
      </div>

      <!-- Section: Guided Topics (from learn topics) -->
      <div>
        <h3 class="text-xs font-bold uppercase tracking-wider text-text-muted mb-2.5">
          Learn Topics (CLI & Concepts)
        </h3>
        <div class="space-y-2">
          <div
            v-for="topic in store.learnTopics"
            :key="topic.id"
            class="p-3 rounded-card bg-surface-2 border border-border text-xs"
          >
            <div class="flex items-center justify-between gap-2 mb-1">
              <span class="font-bold text-text">{{ topic.title }}</span>
              <code class="text-[10px] text-accent font-mono">automemory learn {{ topic.id }}</code>
            </div>
            <p class="text-text-muted mb-2 leading-relaxed">{{ topic.summary }}</p>
            <pre class="bg-surface p-2 rounded text-[11px] font-mono text-text overflow-x-auto whitespace-pre-wrap border border-border/60">{{ topic.content }}</pre>
          </div>
        </div>
      </div>

      <!-- Section: Keyboard Shortcuts -->
      <div>
        <h3 class="text-xs font-bold uppercase tracking-wider text-text-muted mb-2.5">
          Keyboard Shortcuts
        </h3>
        <div class="grid grid-cols-2 gap-2 text-xs">
          <div
            v-for="sc in shortcuts"
            :key="sc.key"
            class="p-2 rounded-card bg-surface-2 border border-border flex items-center justify-between"
          >
            <span class="text-text-muted">{{ sc.label }}</span>
            <kbd class="px-1.5 py-0.5 font-mono text-[11px] rounded bg-surface border border-border font-semibold text-text">
              {{ sc.key }}
            </kbd>
          </div>
        </div>
      </div>
    </div>
  </Drawer>
</template>

<script setup lang="ts">
import { ref, onMounted, onUnmounted } from 'vue';
import { useAutoMemoryStore } from '~/stores/automemory';
import { useToast } from '~/composables/useToast';

const store = useAutoMemoryStore();
const { success } = useToast();
const copiedDiag = ref(false);

function handleKeydown(e: KeyboardEvent) {
  const target = e.target as HTMLElement;
  if (['input', 'textarea', 'select'].includes(target.tagName?.toLowerCase())) return;

  if (e.key === '?') {
    e.preventDefault();
    if (store.isHelpDrawerOpen) {
      store.closeHelpDrawer();
    } else {
      store.openHelpDrawer();
    }
  }
}

onMounted(() => {
  window.addEventListener('keydown', handleKeydown);
});

onUnmounted(() => {
  window.removeEventListener('keydown', handleKeydown);
});

const glossary = [
  {
    term: 'Project brain',
    def: 'Verified facts, architectural decisions, and lessons shared with every coding agent session.',
  },
  {
    term: 'Advisory file lock',
    def: 'Automatic leases that prevent concurrent agents from colliding and overwriting the same code.',
  },
  {
    term: 'Hand-off note',
    def: 'A compact context packet (under 400 tokens) passed between sessions to resume immediately where the last session stopped.',
  },
  {
    term: 'Suggestions inbox',
    def: 'Agent-extracted proposals waiting for your review. Approved suggestions become permanent project knowledge.',
  },
  {
    term: 'Safety mode',
    def: 'Relaxed (warnings only), Balanced (blocks risky commands & file collisions), Strict (requires lock for every edit).',
  },
  {
    term: 'Sync vault',
    def: 'Client-side AES-256 encrypted git repository synchronizing memory across laptops and desktops.',
  },
];

const shortcuts = [
  { key: '⌘K / Ctrl+K', label: 'Command palette' },
  { key: '?', label: 'Help & Glossary' },
  { key: 'n', label: 'New note' },
  { key: 'g h', label: 'Go to Home' },
  { key: 'g b', label: 'Project Brain' },
  { key: 'g a', label: 'Agents & Locks' },
  { key: 'g i', label: 'Suggestions Inbox' },
  { key: 'g s', label: 'Sync & Devices' },
];

function replayTour() {
  store.closeHelpDrawer();
  store.startTour();
}

async function copyDiagnostics() {
  try {
    const diag = `AutoMemory v0.1 Diagnostic Report\nConnected: ${store.isConnected}\nMode: ${store.currentMode}\nActive Project: ${store.activeProject || 'all'}\nMemories: ${store.stats.active_memories} active, ${store.stats.archived_memories} archived\nPending Proposals: ${store.stats.pending_proposals}\nCoordination Leases: ${store.activeLeases.length}\nChecklist: ${store.setupChecklist.filter(s => s.done).length}/${store.setupChecklist.length} completed\nDemo Mode: ${store.isDemoMode}`;
    await navigator.clipboard.writeText(diag);
    copiedDiag.value = true;
    success('Diagnostic summary copied to clipboard');
    setTimeout(() => {
      copiedDiag.value = false;
    }, 2000);
  } catch {
    // fallback
  }
}
</script>
