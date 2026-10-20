<template>
  <div class="space-y-6">
    <PageHeader
      title="Continuity"
      description="What the next agent knows when you switch: the goal, what was ruled out and why, open questions and next steps, plus the searchable history of earlier chats."
    >
      <template #actions>
        <Button size="sm" variant="primary" :loading="busy === 'handoff'" @click="handOff">Hand off now</Button>
      </template>
    </PageHeader>

    <EmptyState v-if="data && !data.project" title="No project yet" description="Connect an agent in a project folder first (Agents page)." />

    <template v-else-if="data">
      <!-- Hand-off result / preview -->
      <p v-for="st in data.stale" :key="st.agent" class="rounded-xl bg-warn-bg px-4 py-2 text-sm text-warn">
        {{ st.label }} chatted {{ st.minutes ? `${st.minutes} min` : 'a while' }} after the brief was last updated. Click Hand off now to bring it up to date.
      </p>
      <Card :title="`What the next agent receives (about ${data.preview_tokens} tokens)`">
        <p v-if="handoffNote" class="mb-3 text-sm text-ok">{{ handoffNote }}</p>
        <pre v-if="data.preview" class="max-h-72 overflow-auto whitespace-pre-wrap rounded-xl bg-surface-2/60 px-4 py-3 text-xs text-text">{{ data.preview }}</pre>
        <p v-else class="text-sm text-text-muted">The brief is empty. Agents fill it with <code>continuity.brief</code> before they stop, or add something below.</p>
        <p class="mt-2 text-xs text-text-muted">Shown at the start of a new session, up to {{ data.inject_tokens }} tokens; the rest is one tool call away.</p>
      </Card>

      <!-- Brief -->
      <section aria-labelledby="brief-h" class="space-y-3">
        <h2 id="brief-h" class="text-lg font-semibold text-text">The brief</h2>
        <Card>
          <div v-for="k in data.kinds" :key="k.id">
            <div v-if="itemsOf(k.id).length" class="mb-4 last:mb-0">
              <h3 class="mb-1 text-sm font-semibold text-text">{{ k.heading }}</h3>
              <ul class="divide-y divide-border/60">
                <li v-for="i in itemsOf(k.id)" :key="i.id" class="py-3 first:pt-1 last:pb-0">
                  <div v-if="editing === i.id" class="space-y-2">
                    <textarea v-model="editText" rows="2" class="w-full rounded-xl bg-surface-2 px-3 py-2 text-sm text-text" />
                    <input v-model="editReason" class="w-full rounded-xl bg-surface-2 px-3 py-2 text-sm text-text" placeholder="Why (optional)" />
                    <div class="flex gap-2">
                      <Button size="sm" variant="primary" lock="/api/continuity/items/:id" @click="saveEdit(i.id)">Save</Button>
                      <Button size="sm" variant="ghost" @click="editing = ''">Cancel</Button>
                    </div>
                  </div>
                  <div v-else class="flex flex-wrap items-start justify-between gap-3">
                    <div class="min-w-0">
                      <p class="text-sm text-text">{{ i.text }}<span v-if="i.reason" class="text-text-muted"> because {{ i.reason }}</span></p>
                      <p class="mt-0.5 text-xs text-text-muted">{{ i.label }}</p>
                    </div>
                    <div class="flex flex-shrink-0 flex-wrap gap-1">
                      <Button v-if="i.state === 'draft'" size="sm" variant="secondary" lock="/api/continuity/items/:id/:action" @click="act(i.id, 'confirm')">Confirm</Button>
                      <Button size="sm" variant="ghost" lock="/api/continuity/items/:id" @click="startEdit(i)">Edit</Button>
                      <Button v-if="['open', 'next'].includes(i.kind)" size="sm" variant="ghost" lock="/api/continuity/items/:id/:action" @click="act(i.id, 'resolve')">Done</Button>
                      <Button size="sm" variant="ghost" lock="/api/continuity/items/:id/:action" @click="act(i.id, 'remove')">Remove</Button>
                    </div>
                  </div>
                </li>
              </ul>
            </div>
          </div>
          <p v-if="!data.items.length" class="text-sm text-text-muted">Nothing yet.</p>

          <form class="mt-4 grid gap-2 border-t border-border/60 pt-4 sm:grid-cols-[10rem_1fr_1fr_auto]" @submit.prevent="addItem">
            <select v-model="nw.kind" class="rounded-xl bg-surface-2 px-3 py-2 text-sm text-text" aria-label="Kind">
              <option v-for="k in data.kinds" :key="k.id" :value="k.id">{{ k.heading }}</option>
            </select>
            <input v-model="nw.text" class="rounded-xl bg-surface-2 px-3 py-2 text-sm text-text" placeholder="Add to the brief" aria-label="Text" />
            <input v-model="nw.reason" class="rounded-xl bg-surface-2 px-3 py-2 text-sm text-text" placeholder="Why (optional)" aria-label="Reason" />
            <Button size="sm" variant="secondary" type="submit" lock="/api/continuity/items" :disabled="!nw.text.trim()">Add</Button>
          </form>
        </Card>
      </section>

      <!-- Sources -->
      <section aria-labelledby="src-h" class="space-y-3">
        <h2 id="src-h" class="text-lg font-semibold text-text">Chat history</h2>
        <Card>
          <p class="text-sm text-text-muted">Off for every agent until you turn it on. Reading is local and read-only; secrets and your home folder are removed before anything is stored, and nothing leaves this computer unless you sync it.</p>
          <ul class="mt-3 divide-y divide-border/60">
            <li v-for="s in data.sources" :key="s.id" class="py-3 first:pt-0 last:pb-0">
              <div class="flex flex-wrap items-center justify-between gap-3">
                <div class="min-w-0">
                  <p class="text-[15px] font-medium text-text">{{ s.label }} <Badge :variant="s.enabled ? 'ok' : 'info'">{{ s.enabled ? 'Reading' : 'Off' }}</Badge></p>
                  <p class="text-xs text-text-muted">{{ s.found }} chat(s) of this project found · {{ s.sessions }} imported ({{ s.messages }} messages)</p>
                  <p class="text-xs text-text-muted truncate" :title="s.locations.join(', ')">Reads: {{ s.locations.join(', ') }}</p>
                </div>
                <div class="flex flex-wrap gap-2">
                  <Button size="sm" :variant="s.enabled ? 'secondary' : 'primary'" lock="/api/continuity/source/:agent" @click="toggle(s)">{{ s.enabled ? 'Turn off' : 'Turn on' }}</Button>
                  <Button v-if="s.enabled" size="sm" variant="secondary" :loading="latest('chat-import')?.state === 'running'" @click="importNow(s.id)">Import now</Button>
                  <Button v-if="s.sessions" size="sm" variant="ghost" lock="/api/continuity/history" @click="forget(s)">Delete imported</Button>
                </div>
              </div>
            </li>
          </ul>
          <p v-if="latest('chat-import')" class="mt-3 text-xs text-text-muted">Last import: {{ latest('chat-import')!.state }} {{ latest('chat-import')!.message }}</p>
        </Card>

        <Card v-if="data.others?.length" title="Other agents">
          <p class="text-sm text-text-muted">No chat reader yet for {{ data.others.map((o: any) => o.label).join(', ') }}. They still read and write the brief through <code>continuity.brief</code>; transcript reading is added one agent at a time.</p>
        </Card>

        <Card title="Chat history on your other computers">
          <p v-if="!hs || !hs.remote" class="text-sm text-text-muted">Needs a sync remote first (Sync page). The brief itself already travels with it; chat history only moves when you click here.</p>
          <template v-else>
            <ul v-if="hs.sessions.length" class="mb-3 text-sm text-text-muted">
              <li v-for="x in hs.sessions" :key="x.id">{{ x.id }}: {{ x.messages }} messages, about {{ x.kb }} KB encrypted</li>
            </ul>
            <p v-else class="mb-3 text-sm text-text-muted">{{ hs.note || 'No chat imported for this project yet.' }}</p>
            <div class="flex flex-wrap gap-2">
              <Button size="sm" variant="secondary" lock="/api/continuity/history-sync/push" :disabled="!hs.sessions.length" @click="pushHistory">Send to my other computers</Button>
              <Button size="sm" variant="secondary" :loading="busy === 'pull'" @click="pullHistory">Fetch from my other computers</Button>
            </div>
            <p v-if="hsNote" class="mt-2 text-xs text-text-muted">{{ hsNote }}</p>
          </template>
        </Card>

        <Card v-if="llm?.on" title="Suggest with the AI helper">
          <p class="text-sm text-text-muted">Optional. {{ llm.local ? 'Your local model reads the latest imported chat and suggests brief items.' : `Sends the last 40 messages of the latest imported chat (secrets already removed) to ${llm.model} and asks for brief items.` }} Everything it suggests is a draft until you or an agent confirms it.</p>
          <div class="mt-3"><Button size="sm" variant="secondary" :lock="llm.local ? undefined : '/api/llm/run'" @click="suggest">Suggest items</Button></div>
        </Card>

        <Card title="Search earlier chats">
          <form class="flex gap-2" @submit.prevent="search">
            <input v-model="q" class="flex-1 rounded-xl bg-surface-2 px-3 py-2 text-sm text-text" placeholder="e.g. sticky routing" aria-label="Search" />
            <Button size="sm" variant="secondary" type="submit" :disabled="!q.trim()">Search</Button>
          </form>
          <p v-if="searched && !hits.length" class="mt-3 text-sm text-text-muted">Nothing found. Turn an agent on and import first.</p>
          <ul class="mt-3 space-y-2">
            <li v-for="h in hits" :key="h.session + h.seq">
              <button type="button" class="w-full rounded-xl bg-surface-2/60 px-4 py-3 text-left hover:bg-surface-2" @click="openChat(h)">
                <p class="text-xs text-text-muted">{{ h.agent }} · {{ h.title || h.session }} · {{ (h.ts || '').slice(0, 16).replace('T', ' ') }}</p>
                <p class="mt-1 text-sm text-text">{{ h.snippet }}</p>
              </button>
            </li>
          </ul>
          <div v-if="chat" class="mt-4 rounded-xl border border-border/70 p-4">
            <p class="mb-2 text-xs text-text-muted">{{ chat.session.title || chat.session.id }}</p>
            <ol class="space-y-2">
              <li v-for="m in chat.messages" :key="m.seq" class="text-sm">
                <span class="mr-2 text-xs font-semibold text-text-muted">{{ m.role }}</span><span class="whitespace-pre-line text-text">{{ m.text }}</span>
              </li>
            </ol>
          </div>
        </Card>
      </section>
    </template>
  </div>
</template>

<script setup lang="ts">
import { ref, reactive, onMounted } from 'vue';
import { request, safeCall, protectedCall } from '~/utils/api';
import { useJobs } from '~/composables/useJobs';
import { useToast } from '~/composables/useToast';

const toast = useToast();
const { latest, refresh } = useJobs();
const data = ref<any>(null);
const busy = ref('');
const handoffNote = ref('');
const editing = ref('');
const editText = ref('');
const editReason = ref('');
const nw = reactive({ kind: 'next', text: '', reason: '' });
const q = ref('');
const hits = ref<any[]>([]);
const searched = ref(false);
const chat = ref<any>(null);
const hs = ref<any>(null);
const hsNote = ref('');
const llm = ref<any>(null);

const itemsOf = (kind: string) => (data.value?.items || []).filter((i: any) => i.kind === kind);

async function load() {
  const r = await request<any>('/api/continuity');
  if (r.ok) data.value = r.data;
  const h = await request<any>('/api/continuity/history-sync');
  if (h.ok) hs.value = h.data;
}
onMounted(async () => {
  await load();
  const l = await request<any>('/api/llm');
  if (l.ok) llm.value = l.data;
});

async function pushHistory() {
  const r = await protectedCall('POST', '/api/continuity/history-sync/push', 'send chat history to the sync remote', {});
  hsNote.value = r.ok ? `Sent ${r.data.sessions} session(s), encrypted. ${r.data.summary}` : r.error || 'Not sent';
}

async function pullHistory() {
  busy.value = 'pull';
  try {
    const r = await safeCall('POST', '/api/continuity/history-sync/pull');
    hsNote.value = r.ok ? `Added ${r.data.report.sessions} session(s) (${r.data.report.messages} messages); ${r.data.report.already_here} already here.` : r.error || 'Could not fetch';
    await load();
  } finally {
    busy.value = '';
  }
}

async function suggest() {
  const r = llm.value?.local ? await safeCall('POST', '/api/llm/run', { what: 'brief' }) : await protectedCall('POST', '/api/llm/run', 'send chat text to the AI helper', { what: 'brief' });
  if (!r.ok) return toast.error(r.error || 'Could not start');
  await refresh();
  setTimeout(load, 4000);
}

async function handOff() {
  busy.value = 'handoff';
  try {
    const r = await safeCall('POST', '/api/continuity/handoff');
    if (!r.ok) return toast.error(r.error || 'Could not hand off');
    const n = (r.data.imported || []).reduce((a: number, x: any) => a + x.new_messages, 0);
    handoffNote.value = `Hand-off written${n ? `, ${n} new chat message(s) imported` : ''}. Open any agent in this project: it starts with the text below.`;
    await load();
  } finally {
    busy.value = '';
  }
}

async function toggle(s: any) {
  const r = await protectedCall('PUT', `/api/continuity/source/${encodeURIComponent(s.id)}`, `${s.enabled ? 'stop' : 'allow'} reading ${s.label} chats`, { enabled: !s.enabled });
  if (r.ok) await load();
}

async function importNow(agent: string) {
  const r = await safeCall('POST', '/api/continuity/import', { agent });
  if (!r.ok) toast.error(r.error || 'Could not start');
  await refresh();
  setTimeout(load, 1500);
}

async function forget(s: any) {
  const r = await protectedCall('DELETE', `/api/continuity/history?agent=${encodeURIComponent(s.id)}`, `delete imported ${s.label} chats`);
  if (r.ok) { toast.success(`${r.data.removed} message(s) deleted`); chat.value = null; hits.value = []; await load(); }
}

function startEdit(i: any) {
  editing.value = i.id;
  editText.value = i.text;
  editReason.value = i.reason || '';
}

async function saveEdit(id: string) {
  const r = await protectedCall('PUT', `/api/continuity/items/${encodeURIComponent(id)}`, 'edit the brief', { text: editText.value, reason: editReason.value || undefined });
  if (r.ok) { editing.value = ''; await load(); } else toast.error(r.error || 'Not saved');
}

async function act(id: string, action: string) {
  const r = await protectedCall('POST', `/api/continuity/items/${encodeURIComponent(id)}/${action}`, `${action} a brief item`);
  if (r.ok) await load(); else toast.error(r.error || 'Not done');
}

async function addItem() {
  const r = await protectedCall('POST', '/api/continuity/items', 'add to the brief', { kind: nw.kind, text: nw.text, reason: nw.reason || undefined });
  if (r.ok) { nw.text = ''; nw.reason = ''; await load(); } else toast.error(r.error || 'Not added');
}

async function search() {
  const r = await request<any>(`/api/continuity/search?q=${encodeURIComponent(q.value)}`);
  hits.value = r.ok ? r.data.hits : [];
  searched.value = true;
  chat.value = null;
}

async function openChat(h: any) {
  const r = await request<any>(`/api/continuity/chat?session=${encodeURIComponent(h.session)}&from=${Math.max(0, h.seq - 2)}&limit=8`);
  if (r.ok) chat.value = r.data;
}
</script>
