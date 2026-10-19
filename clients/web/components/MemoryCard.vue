<template>
  <Card
    hover
    :custom-class="memory.status === 'archived' ? 'opacity-60 bg-surface-2/30' : ''"
  >
    <!-- View Mode -->
    <div v-if="!isEditing" class="space-y-3">
      <!-- Card Header: Type Badge, Source, Relative Time, Actions -->
      <div class="flex items-center justify-between gap-3 flex-wrap">
        <div class="flex items-center gap-2 flex-wrap">
          <Badge :variant="typeVariant">
            {{ formatType(memory.type) }}
          </Badge>
          <Badge v-if="memory.metadata?.demo" variant="accent">
            Sample
          </Badge>
          <SourceBadge :source="memory.source_kind" :agent="memory.metadata?.agent" />
          <span v-if="memory.status === 'pinned'" class="text-xs font-semibold text-accent flex items-center gap-1">
            <svg class="w-3 h-3" fill="currentColor" viewBox="0 0 20 20">
              <path d="M5 4a2 2 0 012-2h6a2 2 0 012 2v14l-5-2.5L5 18V4z" />
            </svg>
            Pinned
          </span>
          <span v-if="memory.status === 'pending'" class="text-xs text-text-muted italic">
            Waiting for review
          </span>
        </div>

        <div class="flex items-center gap-2">
          <span class="text-xs text-text-muted" :title="memory.created_at">
            {{ formatRelativeTime(memory.created_at) }}
          </span>

          <div class="flex items-center gap-1">
            <!-- Pin Button -->
            <IconButton
              :aria-label="memory.status === 'pinned' ? 'Unpin' : 'Pin to top'"
              :title="memory.status === 'pinned' ? 'Unpin' : 'Pin to top'"
              size="sm"
              @click="$emit('toggle-pin', memory)"
            >
              <svg class="w-3.5 h-3.5" :class="memory.status === 'pinned' ? 'text-accent fill-current' : 'text-text-muted'" fill="none" viewBox="0 0 24 24" stroke="currentColor">
                <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M5 5a2 2 0 012-2h10a2 2 0 012 2v16l-7-3.5L5 21V5z" />
              </svg>
            </IconButton>

            <!-- Edit Button -->
            <IconButton
              aria-label="Edit memory"
              title="Edit inline"
              size="sm"
              @click="startEdit"
            >
              <svg class="w-3.5 h-3.5" fill="none" viewBox="0 0 24 24" stroke="currentColor">
                <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M15.232 5.232l3.536 3.536m-2.036-5.036a2.5 2.5 0 113.536 3.536L6.5 21.036H3v-3.572L16.732 3.732z" />
              </svg>
            </IconButton>

            <button
              v-if="memory.status === 'archived'"
              type="button"
              class="text-sm text-accent hover:underline px-2"
              @click="$emit('restore', memory)"
            >
              Restore
            </button>

            <!-- Archive Button -->
            <IconButton
              v-if="memory.status !== 'archived'"
              aria-label="Archive memory"
              title="Archive"
              size="sm"
              @click="$emit('archive', memory)"
            >
              <svg class="w-3.5 h-3.5" fill="none" viewBox="0 0 24 24" stroke="currentColor">
                <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M19 7l-.867 12.142A2 2 0 0116.138 21H7.862a2 2 0 01-1.995-1.858L5 7m5 4v6m4-6v6m1-10V4a1 1 0 00-1-1h-4a1 1 0 00-1 1v3M4 7h16" />
              </svg>
            </IconButton>
          </div>
        </div>
      </div>

      <!-- Main Summary & Content -->
      <div>
        <h4 class="text-[15px] sm:text-base font-semibold text-text leading-snug" :class="expanded ? '' : 'line-clamp-2'">
          {{ memory.summary }}
        </h4>
        <p
          v-if="preview"
          class="text-sm text-text-muted mt-1.5 leading-relaxed whitespace-pre-line"
          :class="expanded ? '' : 'line-clamp-3'"
        >
          {{ expanded ? plainContent : preview }}
        </p>
        <button
          v-if="isLong"
          type="button"
          class="mt-1.5 text-sm text-accent hover:underline"
          @click="expanded = !expanded"
        >
          {{ expanded ? 'Show less' : 'Show more' }}
        </button>
      </div>

      <!-- Tags & Scope Footer -->
      <div v-if="memory.tags?.length" class="flex items-center gap-1.5 flex-wrap pt-1">
        <span
          v-for="tag in memory.tags.slice(0, 4)"
          :key="tag"
          class="text-xs px-2 py-0.5 rounded-pill bg-surface-2 text-text-muted"
        >
          {{ tag }}
        </span>
      </div>
    </div>

    <!-- Inline Edit Mode -->
    <div v-else class="space-y-3">
      <div>
        <label class="block text-xs font-medium text-text-muted mb-1">Summary</label>
        <input
          v-model="editSummary"
          type="text"
          class="w-full bg-surface-2 border border-transparent rounded-xl px-3.5 py-2 text-sm text-text focus:border-accent focus:outline-none"
        />
      </div>

      <div>
        <label class="block text-xs font-medium text-text-muted mb-1">Content / Rationale</label>
        <textarea
          v-model="editContent"
          rows="3"
          class="w-full bg-surface-2 border border-transparent rounded-xl px-3.5 py-2 text-sm text-text focus:border-accent focus:outline-none"
        />
      </div>

      <div class="flex items-center justify-end gap-2 pt-2">
        <Button size="sm" variant="ghost" class="mr-auto" @click="$emit('veto', memory); isEditing = false">Mark as wrong</Button>
        <Button size="sm" variant="ghost" @click="$emit('revoke', memory); isEditing = false">Revoke</Button>
        <Button size="sm" variant="ghost" @click="isEditing = false">
          Cancel
        </Button>
        <Button size="sm" variant="primary" :loading="isSaving" @click="saveEdit">
          Save changes
        </Button>
      </div>
    </div>
  </Card>
</template>

<script setup lang="ts">
import { ref, computed } from 'vue';
import type { Memory } from '~/types';

const props = defineProps<{
  memory: Memory;
}>();

const emit = defineEmits<{
  (e: 'update', payload: { id: string; summary: string; content: string }): void;
  (e: 'archive', memory: Memory): void;
  (e: 'toggle-pin', memory: Memory): void;
  (e: 'restore', memory: Memory): void;
  (e: 'veto', memory: Memory): void;
  (e: 'revoke', memory: Memory): void;
}>();

const isEditing = ref(false);
const expanded = ref(false);
const isSaving = ref(false);
const editSummary = ref('');
const editContent = ref('');

const typeVariant = computed(() => {
  switch (props.memory.type?.toLowerCase()) {
    case 'decision':
      return 'accent';
    case 'lesson':
    case 'gotcha':
      return 'warn';
    case 'preference':
    case 'convention':
      return 'info';
    case 'failure':
      return 'bad';
    default:
      return 'neutral';
  }
});

function formatType(type?: string): string {
  if (!type) return 'Note';
  const t = type.toLowerCase();
  if (t === 'decision') return 'Decision';
  if (t === 'lesson') return 'Lesson';
  if (t === 'gotcha') return 'Watch out';
  if (t === 'preference') return 'Preference';
  if (t === 'convention') return 'Convention';
  return type.charAt(0).toUpperCase() + type.slice(1);
}

function formatRelativeTime(dateStr?: string): string {
  if (!dateStr) return '';
  const date = new Date(dateStr);
  const now = new Date();
  const diffSec = Math.floor((now.getTime() - date.getTime()) / 1000);

  if (diffSec < 60) return 'just now';
  if (diffSec < 3600) return `${Math.floor(diffSec / 60)}m ago`;
  if (diffSec < 86400) return `${Math.floor(diffSec / 3600)}h ago`;
  if (diffSec < 604800) return `${Math.floor(diffSec / 86400)}d ago`;
  return date.toLocaleDateString();
}

/** Notes written by tools are markdown; show them as calm plain text. */
const plainContent = computed(() =>
  (props.memory.content || '')
    .replace(/```[\s\S]*?```/g, ' ')
    .replace(/^#{1,6}\s*/gm, '')
    .replace(/\*\*|__|`/g, '')
    .replace(/^\s*[-*]\s+/gm, '• ')
    .replace(/\n{3,}/g, '\n\n')
    .trim()
);
const preview = computed(() => (plainContent.value && plainContent.value !== props.memory.summary ? plainContent.value : ''));
const isLong = computed(() => preview.value.length > 220 || preview.value.split('\n').length > 3);

function startEdit() {
  editSummary.value = props.memory.summary;
  editContent.value = props.memory.content || '';
  isEditing.value = true;
}

function saveEdit() {
  emit('update', {
    id: props.memory.id,
    summary: editSummary.value,
    content: editContent.value,
  });
  isEditing.value = false;
}
</script>
