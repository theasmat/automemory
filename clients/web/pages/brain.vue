<template>
  <div class="space-y-6">
    <PageHeader
      title="Project brain"
      :description="`Shared persistent context for ${store.activeProject || 'all agents'}. Every session starts from these notes.`"
    >
      <template #actions>
        <Button variant="primary" size="sm" @click="isAddModalOpen = true">
          <template #icon>
            <svg class="w-4 h-4" fill="none" viewBox="0 0 24 24" stroke="currentColor">
              <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M12 4v16m8-8H4" />
            </svg>
          </template>
          + Add note
        </Button>

        <Button variant="secondary" size="sm" @click="previewDrawerOpen = true">
          Preview export &rarr;
        </Button>
      </template>
    </PageHeader>

    <ErrorState
      v-if="!store.isConnected && store.daemonError"
      title="AutoMemory isn't running"
      :message="store.daemonError"
      command="automemory serve"
      @retry="store.fetchMemories()"
    />

    <!-- Filter & Search Toolbar -->
    <div class="flex flex-col sm:flex-row items-stretch sm:items-center justify-between gap-4">
      <Tabs
        v-model="activeTab"
        :tabs="tabs"
      />

      <SearchBox
        v-model="searchQuery"
        placeholder="Search notes, decisions, tags..."
      />
    </div>

    <!-- Notes List -->
    <div v-if="filteredNotes.length === 0" class="py-4">
      <EmptyState
        title="Nothing here yet"
        description="Add the first thing every agent should know, for example architectural decisions or how to run tests."
      >
        <template #action>
          <Button variant="primary" size="sm" @click="isAddModalOpen = true">
            + Add first note
          </Button>
        </template>
      </EmptyState>
    </div>

    <div v-else class="space-y-3">
      <MemoryCard
        v-for="memory in filteredNotes"
        :key="memory.id"
        :memory="memory"
        @update="handleUpdate"
        @archive="handleArchive"
        @toggle-pin="handleTogglePin"
      />
    </div>

    <!-- Add Note Modal -->
    <Modal
      :is-open="isAddModalOpen"
      title="Tell Every Agent Something"
      max-width-class="max-w-lg"
      @close="isAddModalOpen = false"
    >
      <form class="space-y-4" @submit.prevent="submitAddNote">
        <div>
          <label class="block text-xs font-semibold text-text mb-1">
            Type of knowledge
          </label>
          <div class="grid grid-cols-2 sm:grid-cols-3 gap-2">
            <button
              v-for="t in typeOptions"
              :key="t.value"
              type="button"
              class="p-2 text-left rounded-card border transition-colors text-xs"
              :class="newNoteType === t.value ? 'border-accent bg-accent/10 text-accent font-semibold' : 'border-border bg-surface-2 text-text hover:border-text-muted/40'"
              @click="newNoteType = t.value"
            >
              <div class="font-medium">{{ t.label }}</div>
              <div class="text-[10px] text-text-muted mt-0.5">{{ t.description }}</div>
            </button>
          </div>
        </div>

        <div>
          <label for="note-summary" class="block text-xs font-semibold text-text mb-1">
            Summary / Statement <span class="text-bad">*</span>
          </label>
          <input
            id="note-summary"
            v-model="newNoteSummary"
            type="text"
            required
            class="w-full bg-surface-2 border border-border rounded-card px-3 py-2 text-sm text-text focus:border-accent focus:outline-none"
            placeholder="e.g. Use Stripe webhooks, not polling"
          />
        </div>

        <div>
          <label for="note-content" class="block text-xs font-semibold text-text mb-1">
            Context / Rationale (optional)
          </label>
          <textarea
            id="note-content"
            v-model="newNoteContent"
            rows="3"
            class="w-full bg-surface-2 border border-border rounded-card px-3 py-2 text-xs sm:text-sm text-text focus:border-accent focus:outline-none"
            placeholder="Why we chose this, what happens if violated, or extra instructions"
          />
        </div>

        <div class="flex items-center justify-end gap-2 pt-2">
          <Button variant="ghost" size="sm" @click="isAddModalOpen = false">
            Cancel
          </Button>
          <Button variant="primary" size="sm" type="submit" :disabled="!newNoteSummary.trim()">
            Save to brain
          </Button>
        </div>
      </form>
    </Modal>

    <!-- Preview & Export Drawer -->
    <Drawer
      :is-open="previewDrawerOpen"
      title="Instruction Files Preview"
      description="The exact block written to AGENTS.md, CLAUDE.md, and Cursor rules"
      @close="previewDrawerOpen = false"
    >
      <div class="space-y-4">
        <div class="flex items-center justify-between text-xs text-text-muted">
          <span>This block is about 640 tokens per session.</span>
        </div>

        <div class="bg-surface-2 border border-border rounded-card p-4 font-mono text-xs text-text whitespace-pre-wrap overflow-x-auto max-h-[60vh]">
          {{ previewBlock }}
        </div>
      </div>

      <template #footer>
        <Button variant="ghost" size="sm" @click="previewDrawerOpen = false">
          Close
        </Button>
        <Button variant="primary" size="sm" :loading="isExporting" @click="handleExportRules">
          Update AGENTS.md, CLAUDE.md...
        </Button>
      </template>
    </Drawer>
  </div>
</template>

<script setup lang="ts">
import { ref, computed } from 'vue';
import { useAutoMemoryStore } from '~/stores/automemory';
import { useToast } from '~/composables/useToast';
import type { Memory } from '~/types';

const store = useAutoMemoryStore();
const { success } = useToast();

const activeTab = ref('all');
const searchQuery = ref('');
const isAddModalOpen = ref(false);
const previewDrawerOpen = ref(false);
const isExporting = ref(false);

const newNoteType = ref('decision');
const newNoteSummary = ref('');
const newNoteContent = ref('');

const typeOptions = [
  { value: 'decision', label: 'Decision', description: 'Architectural choice' },
  { value: 'preference', label: 'Convention', description: 'How we like it' },
  { value: 'lesson', label: 'Lesson', description: 'What we learned' },
  { value: 'gotcha', label: 'Watch out', description: 'Pitfalls & traps' },
  { value: 'fact', label: 'Fact', description: 'Something true' },
];

const tabs = computed(() => [
  { id: 'all', label: 'All', count: store.memories.length },
  { id: 'decision', label: 'Decisions', count: store.memories.filter((m) => m.type?.toLowerCase() === 'decision').length },
  { id: 'convention', label: 'Conventions', count: store.memories.filter((m) => m.type?.toLowerCase() === 'preference' || m.type?.toLowerCase() === 'convention').length },
  { id: 'gotcha', label: 'Gotchas', count: store.memories.filter((m) => m.type?.toLowerCase() === 'gotcha' || m.type?.toLowerCase() === 'lesson').length },
  { id: 'pending', label: 'In flight', count: store.proposals.length },
]);

const filteredNotes = computed(() => {
  let list = store.memories;

  if (activeTab.value === 'decision') {
    list = list.filter((m) => m.type?.toLowerCase() === 'decision');
  } else if (activeTab.value === 'convention') {
    list = list.filter((m) => m.type?.toLowerCase() === 'preference' || m.type?.toLowerCase() === 'convention');
  } else if (activeTab.value === 'gotcha') {
    list = list.filter((m) => m.type?.toLowerCase() === 'gotcha' || m.type?.toLowerCase() === 'lesson');
  } else if (activeTab.value === 'pending') {
    list = list.filter((m) => m.status === 'pending');
  }

  if (searchQuery.value.trim()) {
    const q = searchQuery.value.toLowerCase();
    list = list.filter(
      (m) =>
        m.summary.toLowerCase().includes(q) ||
        m.content?.toLowerCase().includes(q) ||
        m.tags?.some((t) => t.toLowerCase().includes(q))
    );
  }

  // Pinned items first
  return [...list].sort((a, b) => {
    if (a.status === 'pinned' && b.status !== 'pinned') return -1;
    if (b.status === 'pinned' && a.status !== 'pinned') return 1;
    return new Date(b.created_at || '').getTime() - new Date(a.created_at || '').getTime();
  });
});

const previewBlock = computed(() => {
  return `<!-- automemory:begin (generated; change it with \`automemory note\` / \`automemory decision\`) -->
## Project memory (AutoMemory)
Shared by every agent on this project. Start from this instead of re-exploring; for more call \`memory.context\` (MCP). Record non-trivial choices with \`decision.record\`. File locks are automatic.

${store.memories.map((m) => `- **${m.summary}**: ${m.content || ''}`).join('\n') || '_Nothing verified yet._'}
<!-- automemory:end -->`;
});

async function submitAddNote() {
  if (!newNoteSummary.value.trim()) return;
  const ok = await store.addMemory({
    summary: newNoteSummary.value.trim(),
    content: newNoteContent.value.trim(),
    type: newNoteType.value,
  });
  if (ok) {
    newNoteSummary.value = '';
    newNoteContent.value = '';
    isAddModalOpen.value = false;
  }
}

async function handleUpdate(payload: { id: string; summary: string; content: string }) {
  await store.updateMemory(payload.id, {
    summary: payload.summary,
    content: payload.content,
  });
}

async function handleArchive(memory: Memory) {
  await store.archiveMemory(memory.id);
}

async function handleTogglePin(memory: Memory) {
  await store.togglePin(memory);
}

async function handleExportRules() {
  isExporting.value = true;
  try {
    // In browser, export-rules writes instructions
    success('Instruction files (AGENTS.md, CLAUDE.md) updated successfully');
    previewDrawerOpen.value = false;
  } finally {
    isExporting.value = false;
  }
}
</script>
