<template>
  <div>
    <p v-if="loading && !changes.length" class="text-sm text-text-muted py-3">Loading file changes…</p>

    <div v-else-if="source === 'none'" class="rounded-card border border-dashed border-border p-4 text-sm text-text-muted">
      <p class="text-text">File history comes from Tracewise.</p>
      <p class="mt-1">{{ note || 'It is not installed here.' }}</p>
      <p class="mt-2">Install it with <code class="font-mono text-xs bg-surface-2 rounded px-1.5 py-0.5">automemory companion install</code> (macOS).</p>
    </div>

    <p v-else-if="!changes.length" class="text-sm text-text-muted py-3">{{ emptyText || 'No file changes recorded yet.' }}</p>

    <ul v-else class="divide-y divide-border/60" aria-label="File changes">
      <li v-for="c in changes" :key="c.id" class="py-2.5 first:pt-0 last:pb-0 min-w-0">
        <div class="flex items-start justify-between gap-3">
          <div class="min-w-0 flex-1">
            <div class="flex items-center gap-2 flex-wrap">
              <span v-if="c.project && !projectId" class="text-xs font-medium text-accent" :title="`Project: ${c.project}`">{{ c.project }}</span>
              <span class="text-xs font-semibold text-text">{{ c.agent }}</span>
              <Badge :variant="opVariant(c.op)">{{ opLabel(c.op) }}</Badge>
              <Badge v-if="c.lock_conflict" variant="warn" :title="c.lock_conflict">Edited while held</Badge>
            </div>
            <p class="font-mono text-xs text-text mt-1 break-all" :title="c.path">{{ c.path }}</p>
            <p v-if="c.why" class="text-xs text-text-muted mt-0.5 line-clamp-2 break-words" :title="`Reason given by ${c.agent}: ${c.why}`">{{ c.why }}</p>
          </div>
          <span class="text-xs text-text-muted whitespace-nowrap flex-shrink-0" :title="c.time">{{ ago(c.time) }}</span>
        </div>
      </li>
    </ul>

    <p v-if="showCommandHint && source === 'tracewise'" class="mt-3 text-xs text-text-muted">
      Full history, diffs and undo: the Tracewise app, or <code class="font-mono bg-surface-2 rounded px-1.5 py-0.5">tracewise log</code> in the project.
    </p>
  </div>
</template>

<script setup lang="ts">
import { ref, watch, onMounted, onBeforeUnmount } from 'vue';
import { request } from '~/utils/api';
import { formatRelativeTime } from '~/utils/memoryText';

interface FileChange {
  id: string;
  time: string;
  agent: string;
  kind: string;
  session: string;
  path: string;
  op: string;
  why?: string | null;
  lock_conflict?: string | null;
  project?: string | null;
}

const props = defineProps<{
  projectId?: string | null;
  /** Only the changes of this session (its id, at least 8 characters). */
  session?: string;
  /** Only these files (at most five are asked for). */
  paths?: string[];
  limit?: number;
  /** Ask again every 10 seconds while the page is open. */
  poll?: boolean;
  emptyText?: string;
  showCommandHint?: boolean;
}>();

const emit = defineEmits<{ (e: 'loaded', info: { source: 'tracewise' | 'none'; count: number }): void }>();

const changes = ref<FileChange[]>([]);
const source = ref<'tracewise' | 'none'>('tracewise');
const note = ref('');
const loading = ref(false);
let timer: ReturnType<typeof setInterval> | null = null;
let seq = 0;

async function load() {
  const mine = ++seq;
  const q = new URLSearchParams();
  if (props.projectId) q.set('project_id', props.projectId);
  if (props.session) q.set('session', props.session);
  if (props.paths?.length) q.set('paths', props.paths.join(','));
  q.set('limit', String(props.limit ?? 20));
  loading.value = true;
  const r = await request<{ source: 'tracewise' | 'none'; changes: FileChange[]; note?: string }>(`/api/changes?${q.toString()}`);
  if (mine !== seq) return; // a newer question is already on its way
  loading.value = false;
  if (r.ok && r.data) {
    source.value = r.data.source;
    changes.value = r.data.changes || [];
    note.value = r.data.note || '';
    emit('loaded', { source: source.value, count: changes.value.length });
  }
}

function ago(t: string) {
  return formatRelativeTime(t);
}
function opLabel(op: string) {
  return ({ create: 'Created', delete: 'Deleted', rename: 'Renamed', modify: 'Changed' } as Record<string, string>)[op] || op;
}
function opVariant(op: string) {
  return op === 'delete' ? 'bad' : op === 'create' ? 'ok' : 'neutral';
}

onMounted(() => {
  load();
  if (props.poll) timer = setInterval(load, 10000);
});
onBeforeUnmount(() => {
  if (timer) clearInterval(timer);
});
watch(() => [props.projectId, props.session, (props.paths || []).join(','), props.limit], load);
</script>
