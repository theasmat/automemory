<template>
  <div class="space-y-6">
    <PageHeader
      data-tour="brain-header"
      title="Project brain"
      :description="`Shared persistent context for ${store.activeProject || 'all agents'}. Every session starts from these notes.`"
    >
      <template #actions>
        <Button variant="primary" size="sm" lock="/api/memories" @click="isAddModalOpen = true">
          <template #icon>
            <svg class="w-4 h-4" fill="none" viewBox="0 0 24 24" stroke="currentColor">
              <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M12 4v16m8-8H4" />
            </svg>
          </template>
          + Add note
        </Button>

        <Button variant="secondary" size="sm" lock="/api/decisions" @click="openDecision">Record a decision</Button>

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

    <!-- The standing rules every agent is shown at the start of a session -->
    <Card v-if="store.constitution" title="Rules every agent follows">
      <details>
        <summary class="cursor-pointer text-sm text-accent">Show the rules</summary>
        <div class="mt-3"><Prose :text="store.constitution" /></div>
      </details>
    </Card>

    <!-- Filter & Search Toolbar -->
    <div class="flex flex-col gap-4">
      <Tabs
        v-model="activeTab"
        :tabs="tabs"
      />

      <SearchBox
        v-model="searchQuery"
        placeholder="Search notes, decisions, tags..."
      />
    </div>

    <!-- Tidy up: memories nobody uses -->
    <Card title="Tidy up">
      <template #actions>
        <Button size="sm" variant="secondary" :loading="tidyLoading" @click="loadTidy">
          {{ tidy === null ? 'Find unused notes' : 'Refresh' }}
        </Button>
      </template>
      <p v-if="tidy === null" class="text-sm text-text-muted">Finds notes no agent has been given in a month, notes never used, and low-confidence ones. Archived notes are kept, not deleted.</p>
      <p v-else-if="tidy.length === 0" class="text-sm text-text-muted">Nothing to tidy. Every note has been used recently.</p>
      <div v-else class="space-y-2">
        <label v-for="t in tidy" :key="t.id" class="flex items-start gap-3 text-sm cursor-pointer">
          <input v-model="tidyPick" type="checkbox" :value="t.id" class="mt-1" />
          <span class="flex-1 min-w-0">
            <span class="text-text">{{ t.summary }}</span>
            <span class="block text-xs text-text-muted">
              {{ t.type }} &middot; {{ t.age_days }} days old &middot;
              {{ t.flags.map((f: string) => tidyLabels[f] || f).join(', ') }}
            </span>
          </span>
        </label>
        <div class="flex gap-2 pt-2">
          <Button size="sm" variant="secondary" @click="tidyPick = tidy.map((t: any) => t.id)">Select all</Button>
          <Button size="sm" variant="primary" lock="/api/memories/archive_many" :disabled="!tidyPick.length" @click="archiveTidy">Archive {{ tidyPick.length }} selected</Button>
        </div>
      </div>
    </Card>

    <!-- Notes List -->
    <div v-if="store.isLoading && store.memories.length === 0" class="space-y-3" aria-busy="true">
      <Skeleton v-for="n in 3" :key="n" height="h-28" />
    </div>

    <div v-else-if="filteredNotes.length === 0" class="py-4">
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
        @restore="handleRestore"
        @veto="handleVeto"
        @revoke="handleRevoke"
      />
    </div>

    <!-- Record a decision -->
    <Modal :is-open="decisionOpen" title="Record a decision" max-width-class="max-w-lg" @close="decisionOpen = false">
      <div class="space-y-4">
        <p class="text-sm text-text-muted">A decision on a topic replaces the earlier one (the history is kept). Every agent sees it first.</p>
        <div>
          <label class="block text-sm text-text-muted mb-1" for="dtopic">Topic</label>
          <input id="dtopic" v-model="decision.topic" placeholder="payments" class="w-full bg-surface-2 border border-transparent rounded-xl px-3.5 py-2 text-sm text-text" />
        </div>
        <div>
          <label class="block text-sm text-text-muted mb-1" for="ddec">Decision</label>
          <input id="ddec" v-model="decision.decision" placeholder="Use Stripe webhooks, not polling" class="w-full bg-surface-2 border border-transparent rounded-xl px-3.5 py-2 text-sm text-text" />
        </div>
        <div>
          <label class="block text-sm text-text-muted mb-1" for="dwhy">Why</label>
          <input id="dwhy" v-model="decision.reason" placeholder="Polling missed events during outages" class="w-full bg-surface-2 border border-transparent rounded-xl px-3.5 py-2 text-sm text-text" />
        </div>
      </div>
      <template #footer>
        <Button variant="ghost" size="sm" @click="decisionOpen = false">Cancel</Button>
        <Button variant="primary" size="sm" :disabled="!decision.topic || !decision.decision" @click="saveDecision">Save</Button>
      </template>
    </Modal>

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
              <div class="text-xs text-text-muted mt-0.5">{{ t.description }}</div>
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
            class="w-full bg-surface-2 border border-border rounded-card px-3 py-2 text-sm text-text focus:border-accent focus:outline-none"
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
import { ref, reactive, computed } from 'vue';
import { api, apiGet, protectedCall, safeCall } from '~/utils/api';
import { useAutoMemoryStore } from '~/stores/automemory';
import { useToast } from '~/composables/useToast';
import type { Memory } from '~/types';

const store = useAutoMemoryStore();
const { success } = useToast();

const activeTab = ref('curated');

/** Notes a tool wrote on its own (session summaries). Useful, but not what a person curates. */
const isAuto = (m: any) =>
  ['task_reflection', 'tool_event', 'document'].includes(m.source_kind) || m.tags?.includes('distilled-session');
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
  { id: 'curated', label: 'Curated', count: store.memories.filter((m) => !isAuto(m)).length },
  { id: 'decision', label: 'Decisions', count: store.memories.filter((m) => m.type?.toLowerCase() === 'decision').length },
  { id: 'convention', label: 'Conventions', count: store.memories.filter((m) => m.type?.toLowerCase() === 'preference' || m.type?.toLowerCase() === 'convention').length },
  { id: 'gotcha', label: 'Gotchas', count: store.memories.filter((m) => m.type?.toLowerCase() === 'gotcha' || m.type?.toLowerCase() === 'lesson').length },
  { id: 'auto', label: 'Auto-captured', count: store.memories.filter(isAuto).length },
  { id: 'all', label: 'All', count: store.memories.length },
]);

const filteredNotes = computed(() => {
  let list = store.memories;

  if (activeTab.value === 'curated') {
    list = list.filter((m) => !isAuto(m));
  } else if (activeTab.value === 'auto') {
    list = list.filter(isAuto);
  } else if (activeTab.value === 'decision') {
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
Shared by every agent on this project. Start from this instead of re-exploring; for more call \`memory.list\` with \`view: context\` (MCP). Record non-trivial choices with \`decision.record\`. File locks are automatic.

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
    const r = await safeCall('POST', '/api/tools/export-rules');
    if (r.ok) {
      const changed = (r.data.files || []).filter((f: any) => f.status !== 'unchanged').length;
      success(changed ? `${changed} instruction file(s) written` : 'Instruction files were already up to date');
      previewDrawerOpen.value = false;
    }
  } finally {
    isExporting.value = false;
  }
}


const tidy = ref<any[] | null>(null);
const tidyPick = ref<string[]>([]);
const tidyLoading = ref(false);
const tidyLabels: Record<string, string> = {
  never_recalled: 'never used',
  unused_30d: 'not used for 30 days',
  low_confidence: 'low confidence',
  missing_file: 'mentions a file that no longer exists',
};

async function loadTidy() {
  tidyLoading.value = true;
  try {
    const q = store.activeProject ? `?project_id=${encodeURIComponent(store.activeProject)}` : '';
    const r = await apiGet<any>(`/api/memories/hygiene${q}`);
    tidy.value = r.ok ? r.data.items : [];
    tidyPick.value = [];
  } finally {
    tidyLoading.value = false;
  }
}

async function archiveTidy() {
  const r = await protectedCall('POST', '/api/memories/archive_many', `archive ${tidyPick.value.length} notes`, { ids: tidyPick.value });
  if (r.ok) {
    success(`${r.data.archived.length} notes archived (kept, not deleted).`);
    await store.fetchMemories();
    await loadTidy();
  }
}

const decisionOpen = ref(false);
const decision = reactive({ topic: '', decision: '', reason: '' });

function openDecision() {
  Object.assign(decision, { topic: '', decision: '', reason: '' });
  decisionOpen.value = true;
}

async function saveDecision() {
  const r = await protectedCall('POST', '/api/decisions', 'record a decision', { ...decision, project_id: store.activeProject || undefined });
  if (r.ok) {
    decisionOpen.value = false;
    success('Decision recorded. Every agent will see it.');
    await store.fetchMemories();
  }
}

async function handleRestore(memory: Memory) {
  if ((await api.restoreMemory(memory.id)).ok) await store.fetchMemories();
}

async function handleVeto(memory: Memory) {
  if ((await api.vetoMemory(memory.id)).ok) {
    success('Marked as wrong. Agents will not use it.');
    await store.fetchMemories();
  }
}

async function handleRevoke(memory: Memory) {
  if ((await api.revokeMemory(memory.id)).ok) {
    success('Revoked and archived.');
    await store.fetchMemories();
  }
}
</script>
