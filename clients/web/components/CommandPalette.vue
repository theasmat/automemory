<template>
  <Teleport to="body">
    <Transition
      enter-active-class="transition duration-150 ease-out"
      enter-from-class="opacity-0 scale-95"
      enter-to-class="opacity-100 scale-100"
      leave-active-class="transition duration-100 ease-in"
      leave-from-class="opacity-100 scale-100"
      leave-to-class="opacity-0 scale-95"
    >
      <div
        v-if="isOpen"
        class="fixed inset-0 z-50 flex items-start justify-center pt-20 p-4 bg-overlay backdrop-blur-sm"
        role="dialog"
        aria-modal="true"
        aria-label="Command palette"
        @keydown.esc="close"
      >
        <div class="fixed inset-0" @click="close" />

        <div class="relative w-full max-w-xl bg-surface border border-border rounded-card shadow-2xl overflow-hidden z-10 flex flex-col max-h-[70vh]">
          <div class="flex items-center gap-3 px-4 py-3 border-b border-border bg-surface-2/30">
            <svg class="w-4 h-4 text-text-muted flex-shrink-0" fill="none" viewBox="0 0 24 24" stroke="currentColor">
              <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M21 21l-6-6m2-5a7 7 0 11-14 0 7 7 0 0114 0z" />
            </svg>
            <input
              ref="inputRef"
              v-model="query"
              type="text"
              class="w-full bg-transparent border-none text-sm text-text placeholder-text-muted focus:outline-none"
              placeholder="Type a command or jump to page... (Esc to close)"
              @keydown.down.prevent="navigate(1)"
              @keydown.up.prevent="navigate(-1)"
              @keydown.enter="executeActive"
            />
            <kbd class="hidden sm:inline-block text-xs font-mono px-1.5 py-0.5 rounded bg-surface border border-border text-text-muted">
              Esc
            </kbd>
          </div>

          <div class="flex-1 overflow-y-auto p-2 space-y-1">
            <div v-if="filteredCommands.length === 0" class="p-6 text-center text-xs text-text-muted">
              No matching commands or pages found.
            </div>

            <button
              v-for="(cmd, idx) in filteredCommands"
              :key="cmd.id"
              type="button"
              class="w-full flex items-center justify-between gap-3 px-3 py-2 rounded-card text-sm text-left transition-colors"
              :class="idx === selectedIndex ? 'bg-accent/15 text-accent font-medium' : 'text-text hover:bg-surface-2'"
              @mouseenter="selectedIndex = idx"
              @click="execute(cmd)"
            >
              <div class="flex items-center gap-2.5">
                <span class="text-xs text-text-muted">{{ cmd.category }}</span>
                <span>{{ cmd.title }}</span>
              </div>
              <span v-if="cmd.shortcut" class="text-xs font-mono text-text-muted">
                {{ cmd.shortcut }}
              </span>
            </button>
          </div>
        </div>
      </div>
    </Transition>
  </Teleport>
</template>

<script setup lang="ts">
import { ref, computed, watch, nextTick, onMounted, onUnmounted } from 'vue';
import { useRouter } from 'vue-router';
import { useTheme } from '~/composables/useTheme';

const router = useRouter();
const { setTheme } = useTheme();
const isOpen = ref(false);
const query = ref('');
const selectedIndex = ref(0);
const inputRef = ref<HTMLInputElement | null>(null);

const emit = defineEmits<{
  (e: 'add-note'): void;
}>();

interface CommandItem {
  id: string;
  category: string;
  title: string;
  shortcut?: string;
  action: () => void;
}

const commands: CommandItem[] = [
  { id: 'home', category: 'Navigation', title: 'Go to Home', shortcut: 'g h', action: () => router.push('/') },
  { id: 'brain', category: 'Navigation', title: 'Go to Project Brain', shortcut: 'g b', action: () => router.push('/brain') },
  { id: 'agents', category: 'Navigation', title: 'Go to Agents & Locks', shortcut: 'g a', action: () => router.push('/agents') },
  { id: 'inbox', category: 'Navigation', title: 'Go to Suggestions Inbox', shortcut: 'g i', action: () => router.push('/inbox') },
  { id: 'sync', category: 'Navigation', title: 'Go to Sync & Devices', shortcut: 'g s', action: () => router.push('/sync') },
  { id: 'safety', category: 'Navigation', title: 'Go to Safety Guardrails', action: () => router.push('/safety') },
  { id: 'skills', category: 'Navigation', title: 'Go to Learned Skills', action: () => router.push('/skills') },
  { id: 'docs', category: 'Navigation', title: 'Go to Project Documents', action: () => router.push('/docs') },
  { id: 'traces', category: 'Navigation', title: 'Go to What Worked (Traces)', action: () => router.push('/traces') },
  { id: 'failures', category: 'Navigation', title: 'Go to Problems & Fixes', action: () => router.push('/failures') },
  { id: 'add-note', category: 'Action', title: 'Add a new note to Project Brain', shortcut: 'n', action: () => emit('add-note') },
  { id: 'theme-system', category: 'Theme', title: 'Switch to System Theme (Auto)', action: () => setTheme('system') },
  { id: 'theme-light', category: 'Theme', title: 'Switch to Light Theme', action: () => setTheme('light') },
  { id: 'theme-dark', category: 'Theme', title: 'Switch to Dark Theme', action: () => setTheme('dark') },
];

const filteredCommands = computed(() => {
  if (!query.value.trim()) return commands;
  const q = query.value.toLowerCase();
  return commands.filter(
    (c) => c.title.toLowerCase().includes(q) || c.category.toLowerCase().includes(q)
  );
});

function navigate(dir: number) {
  const len = filteredCommands.value.length;
  if (len === 0) return;
  selectedIndex.value = (selectedIndex.value + dir + len) % len;
}

function executeActive() {
  const cmd = filteredCommands.value[selectedIndex.value];
  if (cmd) execute(cmd);
}

function execute(cmd: CommandItem) {
  close();
  cmd.action();
}

function open() {
  isOpen.value = true;
  query.value = '';
  selectedIndex.value = 0;
  nextTick(() => inputRef.value?.focus());
}

function close() {
  isOpen.value = false;
}

function handleKeydown(e: KeyboardEvent) {
  if ((e.metaKey || e.ctrlKey) && e.key?.toLowerCase() === 'k') {
    e.preventDefault();
    if (isOpen.value) close();
    else open();
  }
}

onMounted(() => {
  window.addEventListener('keydown', handleKeydown);
});

onUnmounted(() => {
  window.removeEventListener('keydown', handleKeydown);
});

defineExpose({ open, close });
</script>
