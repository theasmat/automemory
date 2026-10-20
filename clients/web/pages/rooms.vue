<template>
  <div class="space-y-6">
    <PageHeader
      title="Planning rooms"
      description="Agents propose, critique and revise a plan together. You give the verdict: approving records the decision and creates a task for each step."
    />

    <EmptyState
      v-if="rooms.length === 0"
      title="No rooms yet"
      description="An agent opens one with the room.join tool when a design choice is worth two opinions. They take turns: propose, critique, revise."
    />

    <div v-else class="grid grid-cols-1 lg:grid-cols-[20rem_1fr] gap-4">
      <!-- List -->
      <ul class="space-y-2" aria-label="Rooms">
        <li v-for="r in rooms" :key="r.room.id">
          <button
            type="button"
            class="w-full text-left rounded-card px-4 py-3 shadow-card transition-colors"
            :class="r.room.id === selectedId ? 'bg-accent/10 ring-2 ring-accent' : 'bg-surface border border-border/70 hover:bg-surface-2'"
            @click="select(r.room.id)"
          >
            <div class="flex items-center justify-between gap-2">
              <span class="text-sm font-semibold text-text truncate">{{ r.room.topic }}</span>
              <Badge :variant="badge(r).variant">{{ badge(r).label }}</Badge>
            </div>
            <p class="mt-1 text-xs text-text-muted">{{ r.posts }} post{{ r.posts === 1 ? '' : 's' }} · {{ r.room.members.length }} agent{{ r.room.members.length === 1 ? '' : 's' }}</p>
          </button>
        </li>
      </ul>

      <!-- Detail -->
      <div v-if="detail" class="space-y-4">
        <Card :title="detail.room.topic">
          <template #actions>
            <Badge :variant="stateBadge.variant">{{ stateBadge.label }}</Badge>
          </template>
          <p class="text-xs text-text-muted">
            Agents: {{ detail.room.members.join(', ') }}<span v-if="detail.room.turn_holder"> · waiting for {{ detail.room.turn_holder }}</span>
          </p>
          <ol class="mt-4 space-y-4">
            <li v-for="p in detail.posts" :key="p.id" class="rounded-xl bg-surface-2/60 px-4 py-3">
              <div class="flex items-center gap-2 text-xs text-text-muted">
                <Badge variant="info">{{ kindLabel[p.kind] || p.kind }}</Badge>
                <span class="font-medium text-text">{{ p.author_session }}</span>
              </div>
              <p class="mt-2 text-sm text-text whitespace-pre-line">{{ p.body }}</p>
              <p v-if="p.evidence?.length" class="mt-2 text-xs text-text-muted">Based on {{ p.evidence.length }} stored note{{ p.evidence.length === 1 ? '' : 's' }}</p>
            </li>
          </ol>
          <p v-if="!detail.posts.length" class="mt-3 text-sm text-text-muted">Nobody has posted yet.</p>
        </Card>

        <Card v-if="detail.plan && !detail.outcome" title="If you approve">
          <p class="text-sm text-text">{{ detail.plan.decision }}</p>
          <p v-if="detail.plan.dissent" class="mt-2 text-xs text-text-muted">Objection it answered: {{ detail.plan.dissent }}</p>
          <ul v-if="detail.plan.tasks.length" class="mt-3 space-y-1 text-sm text-text">
            <li v-for="t in detail.plan.tasks" :key="t" class="flex gap-2"><span aria-hidden="true">•</span>{{ t }}</li>
          </ul>
          <p v-else class="mt-3 text-sm text-text-muted">The plan lists no steps, so no tasks will be created. Only the decision is recorded.</p>
          <div class="mt-4 flex flex-wrap gap-2">
            <Button size="sm" variant="primary" lock="/api/rooms/:id/approve" :loading="busy" @click="act('approve', 'approve this plan')">Approve</Button>
            <Button size="sm" variant="secondary" lock="/api/rooms/:id/another-round" :loading="busy" @click="act('another-round', 'send this plan back for another round')">Another round</Button>
            <Button size="sm" variant="ghost" lock="/api/rooms/:id/reject" :loading="busy" @click="act('reject', 'reject this plan')">Reject</Button>
          </div>
        </Card>

        <Card v-if="detail.outcome" title="Your verdict">
          <p class="text-sm text-text">
            {{ detail.outcome.outcome === 'approved'
              ? `Approved. The decision is recorded and ${detail.outcome.task_ids.length} task${detail.outcome.task_ids.length === 1 ? '' : 's'} wait for an agent to claim.`
              : 'Rejected. Nothing was stored.' }}
          </p>
        </Card>
      </div>
    </div>
  </div>
</template>

<script setup lang="ts">
import { ref, computed, onMounted, onBeforeUnmount } from 'vue';
import { request, protectedCall } from '~/utils/api';
import { useToast } from '~/composables/useToast';

const toast = useToast();
const rooms = ref<any[]>([]);
const selectedId = ref('');
const detail = ref<any>(null);
const busy = ref(false);
let timer: ReturnType<typeof setInterval> | null = null;

const kindLabel: Record<string, string> = { propose: 'Proposal', critique: 'Critique', revise: 'Revision' };

function badge(r: any): { label: string; variant: 'ok' | 'warn' | 'info' | 'accent' | 'bad' } {
  if (r.outcome) return r.outcome.outcome === 'approved' ? { label: 'Approved', variant: 'ok' } : { label: 'Rejected', variant: 'bad' };
  if (r.needs_you) return { label: 'Needs you', variant: 'warn' };
  return { label: 'In discussion', variant: 'info' };
}
const stateBadge = computed(() => (detail.value ? badge({ outcome: detail.value.outcome, needs_you: detail.value.room.state === 'decided' && !detail.value.outcome }) : { label: '', variant: 'info' as const }));

async function load() {
  const r = await request<any[]>('/api/rooms');
  if (r.ok && Array.isArray(r.data)) {
    rooms.value = r.data;
    if (!selectedId.value && r.data.length) selectedId.value = (r.data.find((x) => x.needs_you) || r.data[0]).room.id;
    if (selectedId.value) await loadDetail();
  }
}

async function loadDetail() {
  const r = await request<any>(`/api/rooms/${encodeURIComponent(selectedId.value)}`);
  if (r.ok) detail.value = r.data;
}

function select(id: string) {
  selectedId.value = id;
  loadDetail();
}

async function act(path: string, text: string) {
  busy.value = true;
  try {
    const r = await protectedCall('POST', `/api/rooms/${encodeURIComponent(selectedId.value)}/${path}`, text);
    if (r.ok) toast.success(path === 'approve' ? 'Approved: decision recorded, tasks created' : path === 'reject' ? 'Rejected' : 'Sent back for another round');
    else toast.error(r.error || 'Not done');
    await load();
  } finally {
    busy.value = false;
  }
}

onMounted(() => {
  load();
  timer = setInterval(load, 5000);
});
onBeforeUnmount(() => timer && clearInterval(timer));
</script>
