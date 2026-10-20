<template>
  <div class="space-y-6">
    <PageHeader
      title="Planning rooms"
      description="Agents propose, critique and revise a plan together. You give the verdict: approving records the decision and creates a task for each step."
    >
      <template #actions>
        <Button size="sm" variant="primary" lock="/api/rooms" @click="openNew">New room</Button>
      </template>
    </PageHeader>

    <Modal :is-open="newOpen" title="New planning room" max-width-class="max-w-xl" @close="newOpen = false">
      <form class="space-y-4" @submit.prevent="createRoom">
        <label class="block text-xs text-text-muted">Topic
          <input v-model="nr.topic" required class="mt-1 w-full rounded-xl bg-surface-2 px-3 py-2 text-sm text-text" placeholder="Cart storage" />
        </label>
        <label class="block text-xs text-text-muted">The question to answer (agents work from this)
          <input v-model="nr.question" required class="mt-1 w-full rounded-xl bg-surface-2 px-3 py-2 text-sm text-text" placeholder="Where do we keep the cart, and for how long?" />
        </label>
        <fieldset>
          <legend class="text-xs text-text-muted">Invite (connected here)</legend>
          <p v-if="!invitable.length" class="mt-1 text-sm text-text-muted">No agent is connected in this project. Connect one on the Agents page.</p>
          <label v-for="a in invitable" :key="a.id" class="mt-1 flex items-center gap-2 text-sm text-text">
            <input v-model="nr.agents" type="checkbox" :value="a.id" />
            {{ agentName(a.id) }}
            <span v-if="!a.native_hooks" class="text-xs text-text-muted">(no hooks: give it the room id yourself)</span>
          </label>
        </fieldset>
        <div>
          <label class="block text-xs text-text-muted">Starting evidence (optional)
            <input v-model="evQuery" class="mt-1 w-full rounded-xl bg-surface-2 px-3 py-2 text-sm text-text" placeholder="Search notes…" @input="searchNotes" />
          </label>
          <label v-for="m in evResults" :key="m.id" class="mt-1 flex items-start gap-2 text-sm text-text">
            <input v-model="nr.evidence" type="checkbox" :value="m.id" class="mt-1" /> {{ m.summary }}
          </label>
        </div>
        <label class="block text-xs text-text-muted">Files in scope (comma separated, optional)
          <input v-model="nr.scope" class="mt-1 w-full rounded-xl bg-surface-2 px-3 py-2 text-sm text-text" placeholder="shop/cart.py, shop/checkout.py" />
        </label>
        <label class="block text-xs text-text-muted">Constraints, one per line (optional)
          <textarea v-model="nr.constraints" rows="2" class="mt-1 w-full rounded-xl bg-surface-2 px-3 py-2 text-sm text-text" placeholder="No new services" />
        </label>
        <div class="flex justify-end gap-2">
          <Button size="sm" variant="ghost" @click="newOpen = false">Cancel</Button>
          <Button size="sm" variant="primary" type="submit" lock="/api/rooms" :disabled="!nr.topic.trim() || !nr.question.trim() || !nr.agents.length">Create</Button>
        </div>
      </form>
    </Modal>

    <EmptyState
      v-if="rooms.length === 0"
      title="No rooms yet"
      description="Start one with New room, or an agent opens one with room.join. Agents take turns: propose, critique, revise; you decide."
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
            <p class="mt-1 text-xs text-text-muted">{{ r.posts }} post{{ r.posts === 1 ? '' : 's' }} · {{ r.room.members.map((m: string) => shortName(r.labels?.[m] || m)).join(', ') }}</p>
          </button>
        </li>
      </ul>

      <!-- Detail -->
      <div v-if="detail" class="space-y-4">
        <Card :title="detail.room.topic">
          <template #actions>
            <Badge :variant="stateBadge.variant">{{ stateBadge.label }}</Badge>
          </template>
          <div v-if="detail.brief" class="mb-3 space-y-1 text-sm">
            <p v-if="detail.brief.question" class="text-text"><span class="text-text-muted">Question:</span> {{ detail.brief.question }}</p>
            <p v-if="detail.brief.scope?.length" class="text-xs text-text-muted">Scope: {{ detail.brief.scope.join(', ') }}</p>
            <p v-if="detail.brief.constraints?.length" class="text-xs text-text-muted">Constraints: {{ detail.brief.constraints.join(' · ') }}</p>
            <p v-if="detail.brief.invited?.length" class="text-xs text-text-muted">Invited: {{ detail.brief.invited.map(agentName).join(', ') }} (told on their next prompt)</p>
          </div>
          <p class="text-xs text-text-muted">
            Agents: {{ detail.room.members.length ? detail.room.members.map(label).join(', ') : 'none joined yet' }}
            <span v-if="detail.room.turn_holder && detail.room.turn_holder !== lastAuthor"> · waiting for {{ label(detail.room.turn_holder) }}</span>
            <span v-else-if="detail.room.state !== 'decided'"> · waiting for another agent to join and reply</span>
          </p>
          <ol class="mt-4 space-y-4">
            <li v-for="p in detail.posts" :key="p.id" class="rounded-xl px-4 py-3" :class="p.kind === 'note' ? 'bg-accent/10' : 'bg-surface-2/60'">
              <div class="flex items-center gap-2 text-xs text-text-muted">
                <Badge :variant="p.kind === 'note' ? 'accent' : 'info'">{{ kindLabel[p.kind] || p.kind }}</Badge>
                <span class="font-medium text-text" :title="p.author_session">{{ label(p.author_session) }}</span>
              </div>
              <p class="mt-2 text-sm text-text whitespace-pre-line">{{ p.body }}</p>
              <p v-if="p.evidence?.length" class="mt-2 text-xs text-text-muted">Based on {{ p.evidence.length }} stored note{{ p.evidence.length === 1 ? '' : 's' }}</p>
            </li>
          </ol>
          <p v-if="!detail.posts.length" class="mt-3 text-sm text-text-muted">Nobody has posted yet.</p>
          <form v-if="!detail.outcome" class="mt-4 flex gap-2" @submit.prevent="sendNote">
            <input v-model="note" class="flex-1 rounded-xl bg-surface-2 px-3 py-2 text-sm text-text" placeholder="Add a note for the agents (a constraint, an answer, a preference)" />
            <Button size="sm" variant="secondary" type="submit" lock="/api/rooms/:id/note" :disabled="!note.trim()">Send</Button>
          </form>
        </Card>

        <Card v-if="!detail.outcome" :title="detail.plan ? 'The agents\' plan: approve or change it' : 'Decide it yourself'">
          <p class="text-sm text-text-muted">
            {{ detail.plan ? 'Edit anything below before approving. Approving stores the decision and one task per step.'
                           : 'You can end the discussion now with your own decision. It is stored like an approved plan.' }}
          </p>
          <p v-if="detail.plan?.dissent" class="mt-2 text-xs text-text-muted">Objection it answered: {{ detail.plan.dissent }}</p>
          <label class="mt-3 block text-xs text-text-muted">Decision
            <input v-model="form.decision" class="mt-1 w-full rounded-xl bg-surface-2 px-3 py-2 text-sm text-text" placeholder="Store the cart in SQLite" />
          </label>
          <label class="mt-3 block text-xs text-text-muted">Steps, one per line (each becomes a task)
            <textarea v-model="form.steps" rows="4" class="mt-1 w-full rounded-xl bg-surface-2 px-3 py-2 text-sm text-text" placeholder="Add the cart table&#10;Migrate cookie carts" />
          </label>
          <div class="mt-4 flex flex-wrap gap-2">
            <Button size="sm" variant="primary" lock="/api/rooms/:id/decide" :loading="busy" :disabled="!form.decision.trim()" @click="decide">
              {{ detail.plan ? (edited ? 'Approve with my changes' : 'Approve') : 'Decide now' }}
            </Button>
            <Button v-if="detail.plan" size="sm" variant="secondary" lock="/api/rooms/:id/another-round" :loading="busy" @click="act('another-round', 'send this plan back for another round')">Another round</Button>
            <Button v-if="detail.plan" size="sm" variant="ghost" lock="/api/rooms/:id/reject" :loading="busy" @click="act('reject', 'reject this plan')">Reject</Button>
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
import { ref, reactive, computed, onMounted, onBeforeUnmount } from 'vue';
import { request, protectedCall } from '~/utils/api';
import { useToast } from '~/composables/useToast';

const toast = useToast();
const rooms = ref<any[]>([]);
const selectedId = ref('');
const detail = ref<any>(null);
const busy = ref(false);
let timer: ReturnType<typeof setInterval> | null = null;

const newOpen = ref(false);
const nr = reactive({ topic: '', question: '', agents: [] as string[], evidence: [] as string[], scope: '', constraints: '' });
const invitable = ref<any[]>([]);
const evQuery = ref('');
const evResults = ref<any[]>([]);
const agentName = (id: string) => ({ claude: 'Claude Code', agy: 'Antigravity', codex: 'Codex' } as Record<string, string>)[id] || id;

async function openNew() {
  Object.assign(nr, { topic: '', question: '', agents: [], evidence: [], scope: '', constraints: '' });
  evQuery.value = '';
  evResults.value = [];
  newOpen.value = true;
  const r = await request<any>('/api/agents/setup');
  invitable.value = r.ok ? (r.data.agents || []).filter((a: any) => a.registered_here && a.id !== 'claude-desktop') : [];
}

let evTimer: ReturnType<typeof setTimeout> | null = null;
function searchNotes() {
  if (evTimer) clearTimeout(evTimer);
  evTimer = setTimeout(async () => {
    if (!evQuery.value.trim()) { evResults.value = []; return; }
    const r = await request<any[]>(`/api/memories?q=${encodeURIComponent(evQuery.value)}`);
    const picked = evResults.value.filter((m) => nr.evidence.includes(m.id));
    evResults.value = [...picked, ...(r.ok && Array.isArray(r.data) ? r.data.filter((m: any) => !nr.evidence.includes(m.id)).slice(0, 6) : [])];
  }, 250);
}

async function createRoom() {
  const body = {
    topic: nr.topic, question: nr.question, agents: nr.agents, evidence: nr.evidence,
    scope: nr.scope.split(',').map((x) => x.trim()).filter(Boolean),
    constraints: nr.constraints.split('\n').map((x) => x.trim()).filter(Boolean),
  };
  const r = await protectedCall('POST', '/api/rooms', 'start a planning room', body);
  if (!r.ok) return toast.error(r.error || 'Not created');
  newOpen.value = false;
  toast.success('Room started. Invited agents are told on their next prompt.');
  selectedId.value = r.data.room.id;
  await load();
}

const kindLabel: Record<string, string> = { propose: 'Proposal', critique: 'Critique', revise: 'Revision', note: 'Your note' };
const note = ref('');
const form = reactive({ decision: '', steps: '' });
let formFor = '';
const label = (s: string) => detail.value?.labels?.[s] || s;
// Rooms from before the fix keep the turn on whoever posted last; that really means "waiting for another agent".
const lastAuthor = computed(() => [...(detail.value?.posts || [])].reverse().find((p: any) => p.kind !== 'note')?.author_session);
const shortName = (l: string) => l.replace(/\s*\([^)]*\)$/, '');
const steps = () => form.steps.split('\n').map((x) => x.trim()).filter(Boolean);
const edited = computed(() => !!detail.value?.plan && (form.decision.trim() !== detail.value.plan.decision || steps().join('\n') !== detail.value.plan.tasks.join('\n')));

/** Prefill the form from the agents' plan once per room (and when a plan appears), never over your typing. */
function prefill() {
  const key = `${selectedId.value}:${!!detail.value?.plan}`;
  if (formFor === key) return;
  formFor = key;
  Object.assign(form, { decision: detail.value?.plan?.decision || '', steps: (detail.value?.plan?.tasks || []).join('\n') });
}

async function sendNote() {
  const r = await protectedCall('POST', `/api/rooms/${encodeURIComponent(selectedId.value)}/note`, 'add a note to this room', { text: note.value });
  if (r.ok) { note.value = ''; await loadDetail(); } else toast.error(r.error || 'Not sent');
}

async function decide() {
  busy.value = true;
  try {
    const r = await protectedCall('POST', `/api/rooms/${encodeURIComponent(selectedId.value)}/decide`, 'decide this planning room', { decision: form.decision, steps: steps() });
    if (r.ok) toast.success(`Decided: ${r.data.outcome.task_ids.length} task(s) created`);
    else toast.error(r.error || 'Not decided');
    await load();
  } finally {
    busy.value = false;
  }
}

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
  if (r.ok) { detail.value = r.data; prefill(); }
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
