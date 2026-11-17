<template>
  <Card
    hover
    :custom-class="['relative', memory.status === 'archived' ? 'opacity-60 bg-surface-2/30' : ''].join(' ')"
  >
    <div class="space-y-3">
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
          <span v-if="memory.metadata?.shared_by" class="text-xs text-text-muted">from {{ memory.metadata.shared_by }}</span>
          <span v-if="memory.status === 'pinned'" class="text-xs font-semibold text-accent flex items-center gap-1">
            <svg class="w-3 h-3" fill="currentColor" viewBox="0 0 20 20" aria-hidden="true">
              <path d="M5 4a2 2 0 012-2h6a2 2 0 012 2v14l-5-2.5L5 18V4z" />
            </svg>
            Pinned
          </span>
          <span v-if="memory.status === 'pending'" class="text-xs text-text-muted italic">
            Waiting for review
          </span>
        </div>

        <!-- Above the card's click area, so these buttons act on their own -->
        <div class="relative z-10 flex items-center gap-2">
          <span class="text-xs text-text-muted" :title="memory.created_at">
            {{ formatRelativeTime(memory.created_at) }}
          </span>

          <div class="flex items-center gap-1">
            <IconButton
              :aria-label="memory.status === 'pinned' ? 'Unpin' : 'Pin to top'"
              :title="memory.status === 'pinned' ? 'Unpin' : 'Pin to top'"
              size="sm"
              @click="$emit('toggle-pin', memory)"
            >
              <svg class="w-3.5 h-3.5" :class="memory.status === 'pinned' ? 'text-accent fill-current' : 'text-text-muted'" fill="none" viewBox="0 0 24 24" stroke="currentColor" aria-hidden="true">
                <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M5 5a2 2 0 012-2h10a2 2 0 012 2v16l-7-3.5L5 21V5z" />
              </svg>
            </IconButton>

            <IconButton
              aria-label="Edit memory"
              title="Edit"
              size="sm"
              @click="openDetails(true)"
            >
              <svg class="w-3.5 h-3.5" fill="none" viewBox="0 0 24 24" stroke="currentColor" aria-hidden="true">
                <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M15.232 5.232l3.536 3.536m-2.036-5.036a2.5 2.5 0 113.536 3.536L6.5 21.036H3v-3.572L16.732 3.732z" />
              </svg>
            </IconButton>

            <button
              v-if="memory.status === 'archived'"
              type="button"
              class="text-sm text-accent hover:underline px-2 min-h-8"
              @click="$emit('restore', memory)"
            >
              Restore
            </button>

            <IconButton
              v-if="memory.status !== 'archived'"
              aria-label="Archive memory"
              title="Archive"
              size="sm"
              @click="$emit('archive', memory)"
            >
              <svg class="w-3.5 h-3.5" fill="none" viewBox="0 0 24 24" stroke="currentColor" aria-hidden="true">
                <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M19 7l-.867 12.142A2 2 0 0116.138 21H7.862a2 2 0 01-1.995-1.858L5 7m5 4v6m4-6v6m1-10V4a1 1 0 00-1-1h-4a1 1 0 00-1 1v3M4 7h16" />
              </svg>
            </IconButton>
          </div>
        </div>
      </div>

      <!-- Title and preview: the title is a real button whose click area covers the whole card -->
      <div>
        <h4 class="text-[15px] sm:text-base font-semibold text-text leading-snug">
          <button
            type="button"
            class="text-left line-clamp-2 w-full rounded after:absolute after:inset-0 after:content-[''] after:rounded-card focus-visible:outline-none focus-visible:after:ring-2 focus-visible:after:ring-accent"
            aria-haspopup="dialog"
            :aria-label="`Open details: ${memory.summary}`"
            @click="openDetails(false)"
          >
            {{ memory.summary }}
          </button>
        </h4>
        <p
          v-if="preview"
          class="text-sm text-text-muted mt-1.5 leading-relaxed whitespace-pre-line line-clamp-3"
        >
          {{ preview }}
        </p>
        <span v-if="isLong" class="mt-1.5 inline-block text-sm text-accent" aria-hidden="true">Read more</span>
      </div>

      <!-- Tags -->
      <div v-if="memory.tags?.length" class="flex items-center gap-1.5 flex-wrap pt-1">
        <span
          v-for="tag in memory.tags.slice(0, 4)"
          :key="tag"
          class="text-xs px-2 py-0.5 rounded-pill bg-surface-2 text-text-muted"
        >
          {{ tag }}
        </span>
        <span v-if="memory.tags.length > 4" class="text-xs text-text-muted">+{{ memory.tags.length - 4 }}</span>
      </div>
    </div>

    <MemoryDetailModal
      :is-open="detailsOpen"
      :memory="memory"
      :start-editing="startEditing"
      @close="detailsOpen = false"
      @update="(p) => $emit('update', p)"
      @archive="(m) => $emit('archive', m)"
      @restore="(m) => $emit('restore', m)"
      @toggle-pin="(m) => $emit('toggle-pin', m)"
      @veto="(m) => $emit('veto', m)"
      @revoke="(m) => $emit('revoke', m)"
    />
  </Card>
</template>

<script setup lang="ts">
import { ref, computed } from 'vue';
import type { Memory } from '~/types';
import { formatType, formatRelativeTime, plainMemoryText } from '~/utils/memoryText';

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

const detailsOpen = ref(false);
const startEditing = ref(false);

function openDetails(edit: boolean) {
  startEditing.value = edit;
  detailsOpen.value = true;
}

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

const plainContent = computed(() => plainMemoryText(props.memory));
const preview = computed(() => (plainContent.value && plainContent.value !== props.memory.summary ? plainContent.value : ''));
const isLong = computed(() => preview.value.length > 220 || preview.value.split('\n').length > 3 || props.memory.summary.length > 110);
</script>
