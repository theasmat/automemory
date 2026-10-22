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
        <label class="block text-xs font-medium text-text-muted">Topic
          <input v-model="nr.topic" required class="mt-1 w-full rounded-xl bg-surface-2 px-3 py-2 text-sm text-text" placeholder="Cart storage" />
        </label>
        <label class="block text-xs font-medium text-text-muted">The question to answer (agents work from this)
          <input v-model="nr.question" required class="mt-1 w-full rounded-xl bg-surface-2 px-3 py-2 text-sm text-text" placeholder="Where do we keep the cart, and for how long?" />
        </label>
        <fieldset>
          <legend class="text-xs font-medium text-text-muted">Invite (connected here)</legend>
          <p v-if="!invitable.length" class="mt-1 text-sm text-text-muted">No agent is connected in this project. Connect one on the Agents page.</p>
          <label v-for="a in invitable" :key="a.id" class="mt-1.5 flex items-center gap-2 text-sm text-text cursor-pointer">
            <input v-model="nr.agents" type="checkbox" :value="a.id" />
            {{ agentName(a.id) }}
            <span v-if="!a.native_hooks" class="text-xs text-text-muted">(no hooks: give it the room id yourself)</span>
          </label>
        </fieldset>
        <div class="grid grid-cols-1 sm:grid-cols-2 gap-3">
          <label class="block text-xs font-medium text-text-muted">Critique &amp; revision rounds
            <select v-model.number="nr.rounds" class="mt-1 w-full rounded-xl bg-surface-2 px-3 py-2 text-sm text-text">
              <option :value="1">1 round (default)</option>
              <option :value="2">2 rounds</option>
              <option :value="3">3 rounds</option>
            </select>
          </label>
          <label class="block text-xs font-medium text-text-muted">Files in scope (comma separated, optional)
            <input v-model="nr.scope" class="mt-1 w-full rounded-xl bg-surface-2 px-3 py-2 text-sm text-text" placeholder="shop/cart.py, shop/checkout.py" />
          </label>
        </div>
        <div>
          <label class="block text-xs font-medium text-text-muted">Starting evidence (optional)
            <input v-model="evQuery" class="mt-1 w-full rounded-xl bg-surface-2 px-3 py-2 text-sm text-text" placeholder="Search notes…" @input="searchNotes" />
          </label>
          <label v-for="m in evResults" :key="m.id" class="mt-1.5 flex items-start gap-2 text-sm text-text cursor-pointer">
            <input v-model="nr.evidence" type="checkbox" :value="m.id" class="mt-1" /> {{ m.summary }}
          </label>
        </div>
        <label class="block text-xs font-medium text-text-muted">Constraints, one per line (optional)
          <textarea v-model="nr.constraints" rows="2" class="mt-1 w-full rounded-xl bg-surface-2 px-3 py-2 text-sm text-text" placeholder="No new services" />
        </label>
        <div class="flex justify-end gap-2 pt-1">
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

    <div v-else class="grid grid-cols-1 lg:grid-cols-[20rem_1fr] gap-4 items-start">
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
            <p class="mt-1 text-xs text-text-muted truncate">
              {{ r.posts }} post{{ r.posts === 1 ? '' : 's' }}
              <span v-if="r.room.rounds > 1"> · {{ r.room.rounds }} rounds</span>
              <span v-if="r.room.members.length"> · {{ r.room.members.map((m: string) => shortName(r.labels?.[m] || m)).join(', ') }}</span>
            </p>
          </button>
        </li>
      </ul>

      <!-- Detail -->
      <div v-if="detail" class="space-y-4">
        <div v-if="detail.superseded_by" class="flex items-center justify-between gap-3 rounded-card bg-warn-bg px-4 py-3 text-sm text-text">
          <span>This room's plan was superseded by a follow-up planning room.</span>
          <Button size="sm" variant="secondary" @click="select(detail.superseded_by)">Open follow-up room</Button>
        </div>

        <Card :title="detail.room.topic">
          <template #actions>
            <div class="flex items-center gap-2">
              <button
                v-if="detail.room.parent_room"
                type="button"
                class="text-xs text-accent hover:underline"
                @click="select(detail.room.parent_room)"
              >
                ← Prior room
              </button>
              <Badge :variant="stateBadge.variant">{{ stateBadge.label }}</Badge>
            </div>
          </template>

          <!-- Clean stage progress strip -->
          <div class="mb-3 flex flex-wrap items-center justify-between gap-2 rounded-xl bg-surface-2/50 px-3 py-2 text-xs">
            <div class="flex items-center gap-1.5 flex-wrap">
              <span
                v-for="(stage, sIdx) in stages"
                :key="stage.id"
                class="inline-flex items-center gap-1"
              >
                <span
                  class="px-2 py-0.5 rounded-pill font-medium"
                  :class="stage.active ? 'bg-accent/15 text-accent font-semibold' : stage.done ? 'text-text' : 'text-text-muted'"
                >
                  {{ stage.label }}
                </span>
                <span v-if="sIdx < stages.length - 1" class="text-text-muted/50" aria-hidden="true">→</span>
              </span>
            </div>
            <div v-if="detail.room.state !== 'decided' && !detail.outcome" class="flex items-center gap-1.5">
              <Button size="sm" variant="ghost" lock="/api/rooms/:id/skip" :loading="busy" @click="skipTurn">Skip turn</Button>
              <Button size="sm" variant="ghost" lock="/api/rooms/:id/end" :loading="busy" @click="endRoom">End room</Button>
            </div>
          </div>

          <div v-if="detail.brief" class="mb-3 space-y-1 text-sm">
            <p v-if="detail.brief.question" class="text-text"><span class="text-text-muted">Question:</span> {{ detail.brief.question }}</p>
            <p v-if="detail.brief.scope?.length" class="text-xs text-text-muted">Scope: <span class="font-mono">{{ detail.brief.scope.join(', ') }}</span></p>
            <p v-if="detail.brief.constraints?.length" class="text-xs text-text-muted">Constraints: {{ detail.brief.constraints.join(' · ') }}</p>
            <p v-if="detail.brief.invited?.length" class="text-xs text-text-muted">Invited: {{ detail.brief.invited.map(agentName).join(', ') }} (told on their next prompt)</p>
          </div>

          <p class="text-xs text-text-muted">
            Agents: {{ detail.room.members.length ? detail.room.members.map(label).join(', ') : 'none joined yet' }}
            <span v-if="detail.room.rounds > 1"> · {{ detail.room.rounds }} rounds</span>
            <span v-if="detail.room.turn_holder && detail.room.turn_holder !== lastAuthor"> · waiting for {{ label(detail.room.turn_holder) }}</span>
            <span v-else-if="detail.room.state !== 'decided'"> · waiting for another agent to join and reply</span>
          </p>

          <div v-if="detail.turn_stalled && !detail.outcome" class="mt-3 flex flex-wrap items-center justify-between gap-3 rounded-xl bg-warn-bg px-4 py-3 text-xs text-text">
            <span>This turn has waited over 30 minutes without a reply. Pass the turn to the next agent or end the discussion now.</span>
            <div class="flex items-center gap-2">
              <Button size="sm" variant="secondary" lock="/api/rooms/:id/skip" :loading="busy" @click="skipTurn">Skip turn</Button>
              <Button size="sm" variant="secondary" lock="/api/rooms/:id/end" :loading="busy" @click="endRoom">End room</Button>
            </div>
          </div>

          <ol class="mt-4 space-y-3">
            <li v-for="p in detail.posts" :key="p.id" class="rounded-xl px-4 py-3" :class="p.kind === 'note' ? 'bg-accent/10' : 'bg-surface-2/60'">
              <div class="flex items-center gap-2 text-xs text-text-muted">
                <Badge :variant="p.kind === 'note' ? 'accent' : 'info'">{{ kindLabel[p.kind] || p.kind }}</Badge>
                <span class="font-medium text-text" :title="p.author_session">{{ label(p.author_session) }}</span>
              </div>
              <p class="mt-2 text-sm text-text whitespace-pre-line leading-relaxed">{{ p.body }}</p>
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
          <template #actions>
            <Badge v-if="detail.draft" variant="accent">Draft saved</Badge>
          </template>
          <p class="text-sm text-text-muted">
            {{ detail.plan ? 'Edit anything below before approving. Approving stores the decision and one task per step.'
                           : 'You can end the discussion now with your own decision. It is stored like an approved plan.' }}
          </p>
          <p v-if="detail.plan?.dissent" class="mt-2 text-xs text-text-muted">Objection it answered: {{ detail.plan.dissent }}</p>
          <label class="mt-3 block text-xs font-medium text-text-muted">Decision
            <input v-model="form.decision" class="mt-1 w-full rounded-xl bg-surface-2 px-3 py-2 text-sm text-text" placeholder="Store the cart in SQLite" />
          </label>

          <div class="mt-4 space-y-2">
            <div class="flex items-center justify-between">
              <span class="text-xs font-medium text-text-muted">Steps (each becomes a tracked task)</span>
              <button type="button" class="text-xs font-medium text-accent hover:underline" @click="addStep">+ Add step</button>
            </div>
            <div v-if="form.steps.length === 0" class="rounded-xl bg-surface-2/50 px-3 py-3 text-xs text-text-muted">
              No steps yet. Click <button type="button" class="text-accent underline" @click="addStep">+ Add step</button> to add tasks to this plan.
            </div>
            <div
              v-for="(st, idx) in form.steps"
              :key="idx"
              class="rounded-xl bg-surface-2/60 p-2.5 space-y-2 border border-border/60"
            >
              <div class="flex flex-wrap sm:flex-nowrap items-center gap-2">
                <span class="text-xs font-mono text-text-muted w-6 text-center flex-shrink-0">{{ idx + 1 }}.</span>
                <input
                  v-model="st.title"
                  class="flex-1 min-w-[12rem] rounded-lg bg-surface px-2.5 py-1.5 text-sm text-text"
                  placeholder="Add the cart table"
                  :aria-label="`Step ${idx + 1} title`"
                />
                <select
                  v-model="st.assigned_agent"
                  class="rounded-lg bg-surface px-2 py-1.5 text-xs text-text"
                  :aria-label="`Step ${idx + 1} owner`"
                >
                  <option value="">Any agent</option>
                  <option v-for="ag in availableAgents" :key="ag" :value="ag">{{ ag }}</option>
                </select>
                <button
                  type="button"
                  class="px-2 py-1 rounded-lg text-xs transition-colors"
                  :class="hasStepDetails(st) || st.expanded ? 'bg-accent/15 text-accent font-medium' : 'bg-surface text-text-muted hover:text-text'"
                  :title="st.expanded ? 'Hide file locks and check command' : 'Set files to lock or check command'"
                  @click="st.expanded = !st.expanded"
                >
                  {{ hasStepDetails(st) ? 'Scoped' : 'Options' }}
                </button>
                <div class="flex items-center gap-0.5">
                  <button
                    type="button"
                    class="p-1.5 rounded text-text-muted hover:text-text hover:bg-surface disabled:opacity-30"
                    :disabled="idx === 0"
                    aria-label="Move step up"
                    title="Move step up"
                    @click="moveStep(idx, -1)"
                  >
                    <svg class="w-3.5 h-3.5" viewBox="0 0 20 20" fill="currentColor" aria-hidden="true">
                      <path fill-rule="evenodd" d="M10 17a.75.75 0 01-.75-.75V5.612L5.29 9.77a.75.75 0 01-1.08-1.04l5.25-5.5a.75.75 0 011.08 0l5.25 5.5a.75.75 0 11-1.08 1.04l-3.96-4.158V16.25A.75.75 0 0110 17z" clip-rule="evenodd" />
                    </svg>
                  </button>
                  <button
                    type="button"
                    class="p-1.5 rounded text-text-muted hover:text-text hover:bg-surface disabled:opacity-30"
                    :disabled="idx === form.steps.length - 1"
                    aria-label="Move step down"
                    title="Move step down"
                    @click="moveStep(idx, 1)"
                  >
                    <svg class="w-3.5 h-3.5" viewBox="0 0 20 20" fill="currentColor" aria-hidden="true">
                      <path fill-rule="evenodd" d="M10 3a.75.75 0 01.75.75v10.638l3.96-4.158a.75.75 0 111.08 1.04l-5.25 5.5a.75.75 0 01-1.08 0l-5.25-5.5a.75.75 0 111.08-1.04l3.96 4.158V3.75A.75.75 0 0110 3z" clip-rule="evenodd" />
                    </svg>
                  </button>
                  <button
                    type="button"
                    class="p-1.5 rounded text-text-muted hover:text-bad hover:bg-surface"
                    aria-label="Remove step"
                    title="Remove step"
                    @click="removeStep(idx)"
                  >
                    <svg class="w-3.5 h-3.5" viewBox="0 0 20 20" fill="currentColor" aria-hidden="true">
                      <path d="M6.28 5.22a.75.75 0 00-1.06 1.06L8.94 10l-3.72 3.72a.75.75 0 101.06 1.06L10 11.06l3.72 3.72a.75.75 0 101.06-1.06L11.06 10l3.72-3.72a.75.75 0 00-1.06-1.06L10 8.94 6.28 5.22z" />
                    </svg>
                  </button>
                </div>
              </div>
              <div v-if="st.expanded || hasStepDetails(st)" class="grid grid-cols-1 sm:grid-cols-2 gap-2 pl-8 pt-1">
                <label class="block text-xs text-text-muted">Files to lock (comma-separated)
                  <input
                    v-model="st.files_text"
                    class="mt-1 w-full rounded-lg bg-surface px-2.5 py-1 text-xs font-mono text-text"
                    placeholder="src/cart.rs, src/db.rs"
                  />
                </label>
                <label class="block text-xs text-text-muted">Check command
                  <input
                    v-model="st.check_cmd"
                    class="mt-1 w-full rounded-lg bg-surface px-2.5 py-1 text-xs font-mono text-text"
                    placeholder="cargo test"
                  />
                </label>
              </div>
            </div>
          </div>

          <div class="mt-4 flex flex-wrap gap-2">
            <Button size="sm" variant="primary" lock="/api/rooms/:id/decide" :loading="busy" :disabled="!form.decision.trim()" @click="decide">
              {{ detail.plan ? (edited ? 'Approve with my changes' : 'Approve') : 'Decide now' }}
            </Button>
            <Button size="sm" variant="secondary" lock="/api/rooms/:id/draft" :loading="busy" :disabled="!form.decision.trim()" @click="saveDraft">
              Save draft
            </Button>
            <Button v-if="detail.plan" size="sm" variant="secondary" lock="/api/rooms/:id/another-round" :loading="busy" @click="act('another-round', 'send this plan back for another round')">Another round</Button>
            <Button v-if="detail.plan" size="sm" variant="ghost" lock="/api/rooms/:id/reject" :loading="busy" @click="act('reject', 'reject this plan')">Reject</Button>
          </div>
        </Card>

        <Card v-if="detail.outcome" title="Your verdict">
          <template #actions>
            <div class="flex items-center gap-2">
              <Badge v-if="detail.outcome.edited_by_human" variant="accent">Edited by you</Badge>
              <NuxtLink v-if="detail.outcome.decision_id" to="/brain" class="text-xs text-accent hover:underline">View in Project brain →</NuxtLink>
            </div>
          </template>
          <div class="flex flex-wrap items-center justify-between gap-3">
            <p class="text-sm text-text">
              {{ detail.outcome.outcome === 'approved'
                ? `Approved. The decision is recorded and ${detail.outcome.task_ids.length} task${detail.outcome.task_ids.length === 1 ? '' : 's'} wait for an agent to claim.`
                : detail.outcome.outcome === 'ended'
                ? 'Ended early without storing a decision.'
                : 'Rejected. Nothing was stored.' }}
            </p>
            <Button
              v-if="detail.outcome.outcome === 'approved' && !detail.superseded_by"
              size="sm"
              variant="secondary"
              lock="/api/rooms/:id/revise"
              :loading="busy"
              @click="revisePlan"
            >
              Revise this plan
            </Button>
          </div>

          <!-- Tracked Tasks (R3) -->
          <div v-if="detail.tasks?.length" class="mt-4 space-y-2 border-t border-border/60 pt-4">
            <div class="flex items-center justify-between">
              <p class="text-xs font-semibold text-text-muted">Plan tasks</p>
              <span class="text-xs text-text-muted">{{ completedTasksCount }} of {{ detail.tasks.length }} done</span>
            </div>
            <ul class="space-y-2">
              <li
                v-for="t in detail.tasks"
                :key="t.id"
                class="rounded-xl bg-surface-2/60 px-3.5 py-2.5 text-sm border border-border/40"
              >
                <div v-if="editingTaskId !== t.id" class="flex flex-wrap items-center justify-between gap-2">
                  <div class="space-y-1 min-w-0">
                    <div class="flex flex-wrap items-center gap-2">
                      <Badge :variant="taskBadge(t).variant">{{ taskBadge(t).label }}</Badge>
                      <span class="font-medium text-text" :class="t.cancelled ? 'line-through text-text-muted' : ''">{{ t.title }}</span>
                      <span v-if="t.assigned_agent" class="text-xs text-text-muted">· owner: {{ t.assigned_agent }}</span>
                    </div>
                    <div v-if="t.files?.length || t.check_cmd || t.check_status" class="flex flex-wrap items-center gap-2 text-xs text-text-muted">
                      <span v-for="f in t.files" :key="f" class="inline-flex items-center gap-1 rounded bg-surface px-1.5 py-0.5 font-mono">
                        <svg
                          v-if="(t.status === 'claimed' || t.status === 'in_progress') && !t.cancelled"
                          class="w-3 h-3 text-warn flex-shrink-0"
                          viewBox="0 0 20 20"
                          fill="currentColor"
                          aria-hidden="true"
                        >
                          <path fill-rule="evenodd" d="M10 2a4 4 0 00-4 4v2H5a1 1 0 00-1 1v7a1 1 0 001 1h10a1 1 0 001-1V9a1 1 0 00-1-1h-1V6a4 4 0 00-4-4zm2 6V6a2 2 0 10-4 0v2h4z" clip-rule="evenodd" />
                        </svg>
                        <span>{{ f }}</span>
                      </span>
                      <span v-if="t.check_cmd" class="font-mono">check: {{ t.check_cmd }}</span>
                      <Badge v-if="t.check_status === 'passed'" variant="ok">Check passed</Badge>
                      <Badge v-else-if="t.check_status === 'failed'" variant="bad">Check failed</Badge>
                    </div>
                  </div>
                  <div v-if="!t.cancelled && t.status !== 'submitted' && t.status !== 'completed'" class="flex items-center gap-1.5">
                    <Button size="sm" variant="ghost" @click="startEditTask(t)">Edit</Button>
                    <Button size="sm" variant="ghost" lock="/api/tasks/:id" :loading="busy" @click="cancelTaskItem(t.id)">Cancel</Button>
                  </div>
                </div>

                <!-- Inline task editor -->
                <div v-else class="space-y-2">
                  <div class="flex flex-wrap items-center gap-2">
                    <input v-model="taskEdit.title" class="flex-1 rounded-lg bg-surface px-2.5 py-1 text-sm text-text" aria-label="Task title" />
                    <select v-model="taskEdit.assigned_agent" class="rounded-lg bg-surface px-2 py-1 text-xs text-text" aria-label="Task owner">
                      <option value="">Any agent</option>
                      <option v-for="ag in availableAgents" :key="ag" :value="ag">{{ ag }}</option>
                    </select>
                  </div>
                  <div class="grid grid-cols-1 sm:grid-cols-2 gap-2">
                    <input v-model="taskEdit.files_text" class="rounded-lg bg-surface px-2.5 py-1 text-xs font-mono text-text" placeholder="Files (comma-separated)" />
                    <input v-model="taskEdit.check_cmd" class="rounded-lg bg-surface px-2.5 py-1 text-xs font-mono text-text" placeholder="Check command" />
                  </div>
                  <div class="flex justify-end gap-2">
                    <Button size="sm" variant="ghost" @click="editingTaskId = ''">Close</Button>
                    <Button size="sm" variant="primary" lock="/api/tasks/:id" :loading="busy" @click="saveTaskEdit(t.id)">Save task</Button>
                  </div>
                </div>
              </li>
            </ul>
          </div>
        </Card>
      </div>
    </div>
  </div>
</template>

<script setup lang="ts">
import { ref, reactive, computed, watch, onMounted, onBeforeUnmount } from 'vue';
import { useRoute, useRouter } from 'vue-router';
import { request, protectedCall } from '~/utils/api';
import { useToast } from '~/composables/useToast';
import { useAutoMemoryStore } from '~/stores/automemory';

interface EditableStep {
  title: string;
  assigned_agent: string;
  files_text: string;
  check_cmd: string;
  expanded?: boolean;
}

const route = useRoute();
const router = useRouter();
const store = useAutoMemoryStore();
const toast = useToast();
const rooms = ref<any[]>([]);
const selectedId = ref((route.query.id as string) || '');
const detail = ref<any>(null);
const busy = ref(false);
let timer: ReturnType<typeof setInterval> | null = null;

const newOpen = ref(false);
const nr = reactive({
  topic: '',
  question: '',
  agents: [] as string[],
  evidence: [] as string[],
  scope: '',
  constraints: '',
  rounds: 1,
});
const invitable = ref<any[]>([]);
const evQuery = ref('');
const evResults = ref<any[]>([]);
const agentName = (id: string) => ({ claude: 'Claude Code', agy: 'Antigravity', codex: 'Codex' } as Record<string, string>)[id] || id;

async function openNew() {
  Object.assign(nr, { topic: '', question: '', agents: [], evidence: [], scope: '', constraints: '', rounds: 1 });
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
    topic: nr.topic,
    question: nr.question,
    agents: nr.agents,
    evidence: nr.evidence,
    scope: nr.scope.split(',').map((x) => x.trim()).filter(Boolean),
    constraints: nr.constraints.split('\n').map((x) => x.trim()).filter(Boolean),
    rounds: nr.rounds,
  };
  const r = await protectedCall('POST', '/api/rooms', 'start a planning room', body);
  if (!r.ok) return toast.error(r.error || 'Not created');
  newOpen.value = false;
  toast.success('Room started. Invited agents are told on their next prompt.');
  select(r.data.room.id);
  await load();
}

const kindLabel: Record<string, string> = { propose: 'Proposal', critique: 'Critique', revise: 'Revision', note: 'Your note' };
const note = ref('');
const form = reactive({ decision: '', steps: [] as EditableStep[] });
let formFor = '';
const label = (s: string) => detail.value?.labels?.[s] || s;
// Rooms from before the fix keep the turn on whoever posted last; that really means "waiting for another agent".
const lastAuthor = computed(() => [...(detail.value?.posts || [])].reverse().find((p: any) => p.kind !== 'note')?.author_session);
const shortName = (l: string) => l.replace(/\s*\([^)]*\)$/, '');

const stages = computed(() => {
  const st = detail.value?.room?.state || 'open';
  const hasOut = !!detail.value?.outcome;
  const order = ['open', 'critique', 'revise', 'decided'];
  const curIdx = hasOut ? 4 : order.indexOf(st);
  return [
    { id: 'open', label: '1. Propose', active: curIdx === 0, done: curIdx > 0 },
    { id: 'critique', label: '2. Critique', active: curIdx === 1, done: curIdx > 1 },
    { id: 'revise', label: '3. Revise', active: curIdx === 2, done: curIdx > 2 },
    { id: 'decided', label: '4. Your verdict', active: curIdx === 3, done: curIdx > 3 },
  ];
});

const completedTasksCount = computed(() =>
  (detail.value?.tasks || []).filter((t: any) => t.status === 'submitted' || t.status === 'completed').length
);

const availableAgents = computed(() => {
  const set = new Set<string>(['claude', 'agy', 'codex']);
  for (const m of detail.value?.room?.members || []) {
    set.add(shortName(label(m)).toLowerCase());
    set.add(m);
  }
  for (const inv of detail.value?.brief?.invited || []) {
    set.add(inv);
  }
  return Array.from(set);
});

function hasStepDetails(st: EditableStep): boolean {
  return Boolean(st.files_text.trim() || st.check_cmd.trim());
}

function cleanSteps() {
  return form.steps
    .map((s) => ({
      title: s.title.trim(),
      assigned_agent: s.assigned_agent.trim() || null,
      files: s.files_text.split(',').map((f) => f.trim()).filter(Boolean),
      check_cmd: s.check_cmd.trim() || null,
    }))
    .filter((s) => s.title);
}

const edited = computed(() => {
  if (!detail.value?.plan) return false;
  if (detail.value?.draft) return true;
  const current = cleanSteps();
  const hasMeta = current.some((s) => s.assigned_agent || s.files.length > 0 || s.check_cmd);
  const titles = current.map((s) => s.title);
  return (
    form.decision.trim() !== detail.value.plan.decision ||
    titles.join('\n') !== (detail.value.plan.tasks || []).join('\n') ||
    hasMeta
  );
});

function addStep() {
  form.steps.push({ title: '', assigned_agent: '', files_text: '', check_cmd: '', expanded: false });
}

function removeStep(idx: number) {
  form.steps.splice(idx, 1);
}

function moveStep(idx: number, delta: number) {
  const target = idx + delta;
  if (target < 0 || target >= form.steps.length) return;
  const [item] = form.steps.splice(idx, 1);
  if (item) form.steps.splice(target, 0, item);
}

/** Prefill the form from a saved draft or the agents' plan once per room state, never over your typing. */
function prefill() {
  const key = `${selectedId.value}:${!!detail.value?.plan}:${!!detail.value?.draft}`;
  if (formFor === key) return;
  formFor = key;
  if (detail.value?.draft) {
    form.decision = detail.value.draft.decision || '';
    form.steps = (detail.value.draft.steps || []).map((s: any) => ({
      title: s.title || '',
      assigned_agent: s.assigned_agent || s.owner || '',
      files_text: Array.isArray(s.files) ? s.files.join(', ') : '',
      check_cmd: s.check_cmd || s.check || '',
      expanded: Boolean((Array.isArray(s.files) && s.files.length) || s.check_cmd || s.check),
    }));
  } else {
    form.decision = detail.value?.plan?.decision || '';
    form.steps = (detail.value?.plan?.tasks || []).map((t: string) => ({
      title: t,
      assigned_agent: '',
      files_text: '',
      check_cmd: '',
      expanded: false,
    }));
  }
}

async function sendNote() {
  const r = await protectedCall('POST', `/api/rooms/${encodeURIComponent(selectedId.value)}/note`, 'add a note to this room', { text: note.value });
  if (r.ok) { note.value = ''; await loadDetail(); } else toast.error(r.error || 'Not sent');
}

async function saveDraft() {
  busy.value = true;
  try {
    const r = await protectedCall('PUT', `/api/rooms/${encodeURIComponent(selectedId.value)}/draft`, 'save a draft of this plan', {
      decision: form.decision,
      steps: cleanSteps(),
    });
    if (r.ok) {
      toast.success('Draft saved');
      await loadDetail();
    } else {
      toast.error(r.error || 'Draft not saved');
    }
  } finally {
    busy.value = false;
  }
}

async function decide() {
  busy.value = true;
  try {
    const r = await protectedCall('POST', `/api/rooms/${encodeURIComponent(selectedId.value)}/decide`, 'decide this planning room', {
      decision: form.decision,
      steps: cleanSteps(),
    });
    if (r.ok) toast.success(`Decided: ${r.data.outcome.task_ids.length} task(s) created`);
    else toast.error(r.error || 'Not decided');
    await load();
  } finally {
    busy.value = false;
  }
}

async function skipTurn() {
  busy.value = true;
  try {
    const r = await protectedCall('POST', `/api/rooms/${encodeURIComponent(selectedId.value)}/skip`, 'skip the current turn in this room');
    if (r.ok) toast.success('Turn skipped');
    else toast.error(r.error || 'Could not skip turn');
    await load();
  } finally {
    busy.value = false;
  }
}

async function endRoom() {
  busy.value = true;
  try {
    const r = await protectedCall('POST', `/api/rooms/${encodeURIComponent(selectedId.value)}/end`, 'end this room so you can decide');
    if (r.ok) toast.success('Discussion ended');
    else toast.error(r.error || 'Could not end room');
    await load();
  } finally {
    busy.value = false;
  }
}

async function revisePlan() {
  busy.value = true;
  try {
    const r = await protectedCall('POST', `/api/rooms/${encodeURIComponent(selectedId.value)}/revise`, 'open a follow-up room to revise this plan');
    if (r.ok && r.data?.room?.id) {
      toast.success('Follow-up room opened');
      formFor = '';
      select(r.data.room.id);
      await load();
    } else {
      toast.error(r.error || 'Could not open follow-up room');
    }
  } finally {
    busy.value = false;
  }
}

const editingTaskId = ref('');
const taskEdit = reactive({ title: '', assigned_agent: '', files_text: '', check_cmd: '' });

function startEditTask(t: any) {
  editingTaskId.value = t.id;
  taskEdit.title = t.title || '';
  taskEdit.assigned_agent = t.assigned_agent || '';
  taskEdit.files_text = Array.isArray(t.files) ? t.files.join(', ') : '';
  taskEdit.check_cmd = t.check_cmd || '';
}

async function saveTaskEdit(taskId: string) {
  busy.value = true;
  try {
    const r = await protectedCall('PATCH', `/api/tasks/${encodeURIComponent(taskId)}`, 'update this task', {
      title: taskEdit.title,
      assigned_agent: taskEdit.assigned_agent,
      files: taskEdit.files_text.split(',').map((x) => x.trim()).filter(Boolean),
      check_cmd: taskEdit.check_cmd,
    });
    if (r.ok) {
      editingTaskId.value = '';
      toast.success('Task updated');
      await loadDetail();
    } else {
      toast.error(r.error || 'Could not update task');
    }
  } finally {
    busy.value = false;
  }
}

async function cancelTaskItem(taskId: string) {
  busy.value = true;
  try {
    const r = await protectedCall('DELETE', `/api/tasks/${encodeURIComponent(taskId)}`, 'cancel this task');
    if (r.ok) {
      toast.success('Task cancelled');
      await loadDetail();
    } else {
      toast.error(r.error || 'Could not cancel task');
    }
  } finally {
    busy.value = false;
  }
}

function taskBadge(t: any): { label: string; variant: 'ok' | 'warn' | 'info' | 'accent' | 'bad' } {
  if (t.cancelled) return { label: 'Cancelled', variant: 'bad' };
  if (t.status === 'submitted' || t.status === 'completed') return { label: 'Done', variant: 'ok' };
  if (t.status === 'claimed' || t.status === 'in_progress') {
    const holder = t.owner_session || t.claimed_by || 'agent';
    return { label: `Claimed (${shortName(label(holder))})`, variant: 'warn' };
  }
  return { label: 'Waiting', variant: 'info' };
}

function badge(r: any): { label: string; variant: 'ok' | 'warn' | 'info' | 'accent' | 'bad' } {
  if (r.outcome) return r.outcome.outcome === 'approved' ? { label: 'Approved', variant: 'ok' } : { label: 'Rejected', variant: 'bad' };
  if (r.needs_you) return { label: 'Needs you', variant: 'warn' };
  if (r.turn_stalled) return { label: 'Turn stalled', variant: 'warn' };
  return { label: 'In discussion', variant: 'info' };
}
const stateBadge = computed(() => (detail.value ? badge({
  outcome: detail.value.outcome,
  needs_you: detail.value.room.state === 'decided' && !detail.value.outcome,
  turn_stalled: detail.value.turn_stalled,
}) : { label: '', variant: 'info' as const }));

async function load() {
  const r = await request<any[]>('/api/rooms');
  if (r.ok && Array.isArray(r.data)) {
    rooms.value = r.data;
    const qId = route.query.id as string | undefined;
    if (qId && r.data.some((x) => x.room.id === qId)) {
      selectedId.value = qId;
    } else if (!selectedId.value && r.data.length) {
      selectedId.value = (r.data.find((x) => x.needs_you) || r.data[0]).room.id;
    }
    if (selectedId.value) await loadDetail();
  }
}

async function loadDetail() {
  const r = await request<any>(`/api/rooms/${encodeURIComponent(selectedId.value)}`);
  if (r.ok) { detail.value = r.data; prefill(); }
}

function select(id: string) {
  selectedId.value = id;
  router.replace({ query: { ...route.query, id } });
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

// Live WebSocket updates replace aggressive polling; fallback timer only runs when WS is disconnected.
watch(() => store.roomVersion, () => {
  load();
});

onMounted(() => {
  store.initWebSocket();
  load();
  timer = setInterval(() => {
    if (!store.isConnected) load();
  }, 5000);
});
onBeforeUnmount(() => timer && clearInterval(timer));
</script>
