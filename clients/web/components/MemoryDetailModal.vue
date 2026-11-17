<template>
  <Modal :is-open="isOpen" :title="editing ? 'Edit note' : 'Note details'" max-width-class="max-w-2xl" @close="onClose">
    <!-- View -->
    <div v-if="!editing" class="space-y-5">
      <div class="flex items-center gap-2 flex-wrap">
        <Badge :variant="typeVariant">{{ formatType(memory.type) }}</Badge>
        <Badge v-if="memory.metadata?.demo" variant="accent">Sample</Badge>
        <SourceBadge :source="memory.source_kind" :agent="memory.metadata?.agent" />
        <span v-if="memory.status !== 'active'" class="text-xs font-semibold text-text-muted capitalize">{{ memory.status }}</span>
      </div>

      <h3 class="text-lg font-semibold text-text leading-snug [text-wrap:balance] break-words">{{ memory.summary }}</h3>

      <div v-if="body" class="text-sm text-text leading-relaxed whitespace-pre-line break-words">{{ body }}</div>
      <p v-else class="text-sm text-text-muted">This note has no text beyond its title.</p>

      <details v-if="rawDiffers" class="text-sm">
        <summary class="cursor-pointer text-accent font-medium">Original text as written</summary>
        <pre class="mt-2 bg-surface-2 rounded-xl p-3 text-xs font-mono whitespace-pre-wrap break-words max-h-64 overflow-y-auto">{{ memory.content }}</pre>
      </details>

      <div v-if="memory.tags?.length || memory.entities?.length" class="space-y-2">
        <div v-if="memory.tags?.length" class="flex items-center gap-1.5 flex-wrap">
          <span class="text-xs text-text-muted w-12">Tags</span>
          <span v-for="t in memory.tags" :key="'t' + t" class="text-xs px-2 py-0.5 rounded-pill bg-surface-2 text-text-muted">{{ t }}</span>
        </div>
        <div v-if="memory.entities?.length" class="flex items-center gap-1.5 flex-wrap">
          <span class="text-xs text-text-muted w-12">Mentions</span>
          <span v-for="t in memory.entities" :key="'e' + t" class="text-xs px-2 py-0.5 rounded-pill bg-surface-2 text-text-muted font-mono">{{ t }}</span>
        </div>
      </div>

      <section v-if="linkedFiles.length" aria-label="Files this note is about" class="border-t border-border pt-4">
        <h4 class="text-sm font-semibold text-text mb-2">Files this note is about</h4>
        <p class="text-xs text-text-muted mb-2 break-all">{{ linkedFiles.join(', ') }}</p>
        <FileChanges :project-id="memory.project_id" :paths="linkedFiles" :limit="6" empty-text="Tracewise has recorded no changes to these files." />
      </section>

      <dl class="grid grid-cols-1 sm:grid-cols-2 gap-x-6 gap-y-2.5 text-sm border-t border-border pt-4">
        <div v-for="row in facts" :key="row.label" class="flex items-baseline justify-between gap-3 min-w-0">
          <dt class="text-text-muted flex-shrink-0">{{ row.label }}</dt>
          <dd class="text-text text-right min-w-0 break-words" :title="row.title">{{ row.value }}</dd>
        </div>
        <div class="flex items-baseline justify-between gap-3 min-w-0 sm:col-span-2">
          <dt class="text-text-muted flex-shrink-0">ID</dt>
          <dd class="min-w-0 flex items-center gap-2">
            <code class="font-mono text-xs text-text-muted truncate">{{ memory.id }}</code>
            <button type="button" class="text-xs text-accent hover:underline flex-shrink-0 min-h-8 px-1" @click="copyId">{{ copied ? 'Copied' : 'Copy' }}</button>
          </dd>
        </div>
      </dl>

      <details v-if="extraMetadata" class="text-sm">
        <summary class="cursor-pointer text-text-muted hover:text-text">Technical details</summary>
        <pre class="mt-2 bg-surface-2 rounded-xl p-3 text-xs font-mono whitespace-pre-wrap break-words max-h-48 overflow-y-auto">{{ extraMetadata }}</pre>
      </details>

      <div class="flex items-center justify-between gap-2 flex-wrap pt-2 border-t border-border sticky bottom-0 -mx-4 px-4 py-3 bg-surface sm:static sm:mx-0 sm:px-0 sm:py-0 sm:pt-2">
        <div class="flex items-center gap-2 flex-wrap">
          <Button size="sm" variant="ghost" lock="/api/memories/:id/pin" @click="$emit('toggle-pin', memory)">
            {{ memory.status === 'pinned' ? 'Unpin' : 'Pin to top' }}
          </Button>
          <Button v-if="memory.status === 'archived'" size="sm" variant="ghost" lock="/api/memories/:id/restore" @click="$emit('restore', memory); onClose()">Restore</Button>
          <Button v-else size="sm" variant="ghost" lock="/api/memories/:id/archive" @click="$emit('archive', memory); onClose()">Archive</Button>
        </div>
        <Button size="sm" variant="primary" lock="/api/memories/:id/update" @click="startEdit">Edit</Button>
      </div>
    </div>

    <!-- Edit -->
    <form v-else class="space-y-4" @submit.prevent="save">
      <div>
        <label for="mem-edit-summary" class="block text-xs font-medium text-text-muted mb-1">Title</label>
        <input id="mem-edit-summary" v-model="editSummary" type="text" required
          class="w-full bg-surface-2 border border-transparent rounded-xl px-3.5 min-h-11 sm:min-h-0 sm:py-2 text-sm text-text focus:border-accent focus:outline-none" />
      </div>
      <div>
        <label for="mem-edit-content" class="block text-xs font-medium text-text-muted mb-1">Full text</label>
        <textarea id="mem-edit-content" v-model="editContent" rows="10"
          class="w-full bg-surface-2 border border-transparent rounded-xl px-3.5 py-2 text-sm text-text focus:border-accent focus:outline-none resize-y" />
        <p class="mt-1 text-xs text-text-muted">The earlier text is kept as a previous version of this note.</p>
      </div>
      <div class="flex items-center justify-end gap-2 flex-wrap pt-2 border-t border-border sticky bottom-0 -mx-4 px-4 py-3 bg-surface sm:static sm:mx-0 sm:px-0 sm:py-0 sm:pt-2">
        <Button size="sm" variant="ghost" class="mr-auto" lock="/api/memories/:id/veto" @click="$emit('veto', memory); onClose()">Mark as wrong</Button>
        <Button size="sm" variant="ghost" lock="/api/memories/:id/revoke" @click="$emit('revoke', memory); onClose()">Revoke</Button>
        <Button size="sm" variant="ghost" @click="cancelEdit">Cancel</Button>
        <Button size="sm" variant="primary" type="submit" lock="/api/memories/:id/update" :disabled="!editSummary.trim()">Save changes</Button>
      </div>
    </form>
  </Modal>
</template>

<script setup lang="ts">
import { ref, computed, watch, nextTick } from 'vue';
import type { Memory } from '~/types';
import { formatType, formatRelativeTime, plainMemoryText } from '~/utils/memoryText';

const props = defineProps<{
  isOpen: boolean;
  memory: Memory;
  /** Open straight into the edit form (the pencil on the card). */
  startEditing?: boolean;
}>();

const emit = defineEmits<{
  (e: 'close'): void;
  (e: 'update', payload: { id: string; summary: string; content: string }): void;
  (e: 'archive', memory: Memory): void;
  (e: 'restore', memory: Memory): void;
  (e: 'toggle-pin', memory: Memory): void;
  (e: 'veto', memory: Memory): void;
  (e: 'revoke', memory: Memory): void;
}>();

const editing = ref(false);
const editSummary = ref('');
const editContent = ref('');
const copied = ref(false);

watch(
  () => props.isOpen,
  (open) => {
    if (open) {
      editing.value = false;
      copied.value = false;
      if (props.startEditing) startEdit();
    }
  },
  { immediate: true }
);

const typeVariant = computed(() => {
  switch (props.memory.type?.toLowerCase()) {
    case 'decision': return 'accent';
    case 'lesson':
    case 'gotcha': return 'warn';
    case 'preference':
    case 'convention': return 'info';
    case 'failure': return 'bad';
    default: return 'neutral';
  }
});

const plain = computed(() => plainMemoryText(props.memory));
const body = computed(() => (plain.value && plain.value !== props.memory.summary ? plain.value : ''));
const rawDiffers = computed(() => !!props.memory.content && props.memory.content.trim() !== body.value.trim() && props.memory.content.trim() !== props.memory.summary.trim());

function pct(n?: number): string {
  return typeof n === 'number' ? `${Math.round(n * 100)}%` : 'n/a';
}
function when(s?: string | null): { value: string; title: string } {
  if (!s) return { value: 'never', title: '' };
  return { value: `${new Date(s).toLocaleString()} (${formatRelativeTime(s)})`, title: s };
}

const facts = computed(() => {
  const m = props.memory;
  const created = when(m.created_at);
  const updated = when(m.updated_at);
  const recalled = when(m.last_recalled_at);
  const rows: { label: string; value: string; title?: string }[] = [
    { label: 'Type', value: formatType(m.type) },
    { label: 'Applies to', value: m.scope === 'global' ? 'Every project' : 'This project' },
    { label: 'Status', value: m.status },
    { label: 'Confidence', value: pct(m.confidence) },
    { label: 'Importance', value: pct(m.importance) },
    { label: 'Freshness', value: pct(m.freshness) },
    { label: 'Created', value: created.value, title: created.title },
    { label: 'Last edited', value: updated.value, title: updated.title },
    { label: 'Last given to an agent', value: recalled.value, title: recalled.title },
  ];
  if (m.version && m.version > 1) rows.push({ label: 'Version', value: `v${m.version}` });
  if (m.metadata?.shared_by) rows.push({ label: 'Shared by', value: String(m.metadata.shared_by) });
  if (m.metadata?.saved_by) rows.push({ label: 'Saved by', value: String(m.metadata.saved_by) });
  return rows;
});

/** Files the note names (what a session edited, or what the note was written about); at most five are looked up. */
const linkedFiles = computed<string[]>(() => {
  const m = props.memory.metadata || {};
  const raw = m.files_modified ?? m.files ?? [];
  return Array.isArray(raw) ? raw.filter((f: unknown) => typeof f === 'string' && f).slice(0, 5) : [];
});

/** Everything in metadata the rows above do not already say. */
const extraMetadata = computed(() => {
  const skip = new Set(['demo', 'agent', 'shared_by', 'saved_by', 'files_modified', 'files']);
  const rest = Object.fromEntries(Object.entries(props.memory.metadata || {}).filter(([k]) => !skip.has(k)));
  return Object.keys(rest).length ? JSON.stringify(rest, null, 2) : '';
});

function startEdit() {
  editSummary.value = props.memory.summary;
  editContent.value = props.memory.content || '';
  editing.value = true;
  nextTick(() => document.getElementById('mem-edit-summary')?.focus());
}
function cancelEdit() {
  if (props.startEditing) onClose();
  else editing.value = false;
}
function save() {
  emit('update', { id: props.memory.id, summary: editSummary.value.trim(), content: editContent.value });
  editing.value = false;
  onClose();
}
function onClose() {
  emit('close');
}
async function copyId() {
  try {
    await navigator.clipboard.writeText(props.memory.id);
    copied.value = true;
    setTimeout(() => (copied.value = false), 1500);
  } catch {
    /* clipboard may be blocked; the id is selectable text */
  }
}
</script>
