<template>
  <span
    class="inline-flex items-center gap-1.5 px-2 py-0.5 rounded-pill text-xs font-medium"
    :class="[
      isHuman ? 'bg-accent/10 text-accent font-semibold' : 'bg-surface-2 text-text-muted'
    ]"
  >
    <svg v-if="isHuman" class="w-3 h-3 text-accent" fill="none" viewBox="0 0 24 24" stroke="currentColor">
      <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M16 7a4 4 0 11-8 0 4 4 0 018 0zM12 14a7 7 0 00-7 7h14a7 7 0 00-7-7z" />
    </svg>
    <svg v-else class="w-3 h-3 text-text-muted" fill="none" viewBox="0 0 24 24" stroke="currentColor">
      <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M9.75 17L9 20l-1 1h8l-1-1-.75-3M3 13h18M5 17h14a2 2 0 002-2V5a2 2 0 00-2-2H5a2 2 0 00-2 2v10a2 2 0 002 2z" />
    </svg>
    <span>{{ formattedSource }}</span>
  </span>
</template>

<script setup lang="ts">
import { computed } from 'vue';

const props = defineProps<{
  source?: string;
  agent?: string;
}>();

const isHuman = computed(() => {
  const s = (props.source || props.agent || '').toLowerCase();
  return s === 'human' || s === 'user' || s === 'manual' || s === 'you';
});

const formattedSource = computed(() => {
  if (isHuman.value) return 'You';
  if (/task_reflection|tool_event|distill/i.test(props.source || '')) return 'Auto-captured';
  const raw = props.agent || props.source || 'Agent';
  if (/claude/i.test(raw)) return 'Claude Code';
  if (/cursor/i.test(raw)) return 'Cursor';
  if (/codex/i.test(raw)) return 'Codex';
  if (/antigravity|agy/i.test(raw)) return 'Antigravity';
  if (/goose/i.test(raw)) return 'Goose';
  return raw;
});
</script>
