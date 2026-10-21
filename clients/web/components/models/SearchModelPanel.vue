<template>
  <div class="space-y-8">
    <ErrorState v-if="loadError" title="Could not load settings" :message="loadError" command="automemory serve" @retry="load" />

    <template v-else-if="state">
      <!-- 1. Mode -->
      <section class="space-y-3" aria-labelledby="mode-h">
        <h2 id="mode-h" class="text-lg font-semibold text-text">Search</h2>
        <div class="grid grid-cols-1 sm:grid-cols-2 gap-4">
          <button
            v-for="m in modes"
            :key="m.id"
            type="button"
            class="p-5 rounded-card text-left shadow-card transition-all"
            :class="state.mode === m.id ? 'bg-accent/10 ring-2 ring-accent' : 'bg-surface border border-border/70 hover:bg-surface-2'"
            @click="setMode(m.id)"
          >
            <span class="text-[17px] font-semibold text-text">{{ m.title }}</span>
            <p class="text-sm text-text-muted mt-1">{{ m.text }}</p>
          </button>
        </div>
      </section>

      <!-- 2. Models (hybrid only) -->
      <section v-if="state.mode === 'hybrid'" class="space-y-3" aria-labelledby="model-h">
        <div class="flex items-center justify-between gap-3">
          <h2 id="model-h" class="text-lg font-semibold text-text">Where meaning comes from</h2>
          <Button variant="primary" size="sm" @click="openAdd">Add a model</Button>
        </div>
        <Card>
          <ul class="divide-y divide-border/60">
            <li v-for="p in state.profiles" :key="p.name" class="py-4 first:pt-0 last:pb-0 flex flex-wrap items-center justify-between gap-3">
              <div class="min-w-0">
                <p class="text-[15px] font-medium text-text flex items-center gap-2">
                  {{ p.name === 'default' ? 'Built-in' : p.name }}
                  <Badge v-if="state.active === p.name" variant="ok">In use</Badge>
                </p>
                <p class="text-sm text-text-muted">
                  {{ describe(p) }}
                  <template v-if="p.needs_key"> · {{ p.key.set ? 'Key: ' + p.key.where : 'No key yet' }}</template>
                </p>
              </div>
              <div class="flex items-center gap-2">
                <Button v-if="p.needs_key" size="sm" variant="secondary" @click="openKey(p.name)">{{ p.key.set ? 'Change key' : 'Add key' }}</Button>
                <Button v-if="state.active !== p.name" size="sm" variant="secondary" @click="useProfile(p.name)">Use</Button>
                <Button v-if="p.name !== 'default'" size="sm" variant="ghost" @click="removeProfile(p.name)">Remove</Button>
              </div>
            </li>
          </ul>
        </Card>
        <p v-if="state.problem" class="text-sm text-warn rounded-xl bg-warn-bg px-4 py-3">{{ state.problem }}</p>
      </section>

      <!-- 3. Test and index -->
      <section v-if="state.mode === 'hybrid'" class="space-y-3" aria-labelledby="index-h">
        <h2 id="index-h" class="text-lg font-semibold text-text">Index your memories</h2>
        <Card>
          <div class="space-y-5">
            <div class="flex flex-wrap items-center justify-between gap-3">
              <div>
                <p class="text-[15px] font-medium text-text">Check the connection</p>
                <p class="text-sm text-text-muted">Embeds one sentence with the model in use.</p>
              </div>
              <Button size="sm" variant="secondary" :loading="busy === 'test'" @click="runTest">Test</Button>
            </div>
            <p v-if="testResult" class="text-sm rounded-xl px-4 py-3" :class="testResult.ok ? 'bg-ok-bg text-ok' : 'bg-bad-bg text-bad'">
              <template v-if="testResult.ok">Works: {{ testResult.model }}, {{ testResult.dimensions }} dimensions, {{ testResult.ms }} ms. {{ testResult.local ? 'Nothing left this computer.' : 'The test sentence was sent to the provider.' }}</template>
              <template v-else>{{ testResult.error }}</template>
            </p>

            <div v-if="state.coverage" class="space-y-2">
              <div class="flex items-center justify-between text-sm">
                <span class="text-text">{{ state.coverage.embedded }} of {{ state.coverage.active }} memories indexed</span>
                <span class="text-text-muted">{{ state.coverage.model }}</span>
              </div>
              <div class="h-1.5 rounded-pill bg-surface-2 overflow-hidden" role="progressbar" :aria-valuenow="percent" aria-valuemin="0" aria-valuemax="100">
                <div class="h-full rounded-pill bg-ok transition-all duration-500" :style="{ width: percent + '%' }" />
              </div>
              <p v-if="state.indexing.running" class="text-sm text-text-muted">Indexing {{ state.indexing.done }} of {{ state.indexing.total }}…</p>
              <p v-else-if="state.indexing.error" class="text-sm text-bad">{{ state.indexing.error }}</p>
              <p v-else-if="state.coverage.pending > 0" class="text-sm text-text-muted">
                {{ state.coverage.pending }} to go, about {{ state.coverage.estimated_tokens.toLocaleString() }} tokens<template v-if="state.coverage.estimated_cost_usd != null && !state.coverage.local"> (about ${{ state.coverage.estimated_cost_usd.toFixed(4) }}, an estimate)</template>.
              </p>
              <p v-if="!state.coverage.local" class="text-xs text-text-muted">Cloud use today: {{ state.tokens_today.toLocaleString() }} of {{ state.max_tokens_per_day.toLocaleString() }} tokens allowed per day.</p>
            </div>

            <div class="flex flex-wrap gap-2">
              <Button v-if="!state.indexing.running" size="sm" variant="primary" :loading="busy === 'index'" @click="startIndex(false)">Index now</Button>
              <Button v-else size="sm" variant="secondary" @click="cancelIndex">Stop</Button>
              <Button v-if="!state.indexing.running" size="sm" variant="secondary" @click="startIndex(true)">Re-index everything</Button>
              <Button v-if="unusedCount > 0" size="sm" variant="ghost" @click="cleanUnused">Remove {{ unusedCount }} unused vectors</Button>
            </div>
          </div>
        </Card>
        <p class="text-sm text-text-muted">
          With a cloud model, the title and the first part of each note are sent to the provider, with secrets removed first. Keys are kept in your computer's keychain, never in a file.
        </p>
      </section>

      <!-- Add a model -->
      <Modal :is-open="addOpen" title="Add a model" max-width-class="max-w-xl" @close="addOpen = false">
        <div class="space-y-4">
          <div>
            <label class="block text-sm text-text-muted mb-1" for="preset">Start from</label>
            <select id="preset" v-model="form.preset" class="w-full bg-surface-2 border border-transparent rounded-xl px-3.5 py-2 text-sm text-text" @change="applyPreset">
              <optgroup label="On this computer (free)">
                <option v-for="p in state.presets.filter((x: any) => x.local)" :key="p.id" :value="p.id">{{ p.label }}</option>
              </optgroup>
              <optgroup label="Cloud (needs a key)">
                <option v-for="p in state.presets.filter((x: any) => !x.local)" :key="p.id" :value="p.id">{{ p.label }}</option>
              </optgroup>
            </select>
            <p class="text-xs text-text-muted mt-1">{{ presetNote }}</p>
          </div>
          <div class="grid grid-cols-1 sm:grid-cols-2 gap-4">
            <div>
              <label class="block text-sm text-text-muted mb-1" for="pname">Name</label>
              <input id="pname" v-model="form.name" placeholder="cloud" class="w-full bg-surface-2 border border-transparent rounded-xl px-3.5 py-2 text-sm text-text" />
            </div>
            <div>
              <label class="block text-sm text-text-muted mb-1" for="pmodel">Model</label>
              <input id="pmodel" v-model="form.model" class="w-full bg-surface-2 border border-transparent rounded-xl px-3.5 py-2 text-sm text-text" />
            </div>
          </div>
          <div v-if="form.provider !== 'exec'">
            <label class="block text-sm text-text-muted mb-1" for="pend">Endpoint</label>
            <input id="pend" v-model="form.endpoint" class="w-full bg-surface-2 border border-transparent rounded-xl px-3.5 py-2 text-sm text-text" />
          </div>
          <div v-else>
            <label class="block text-sm text-text-muted mb-1" for="pcmd">Command</label>
            <input id="pcmd" v-model="form.endpoint" class="w-full bg-surface-2 border border-transparent rounded-xl px-3.5 py-2 text-sm text-text" />
          </div>
          <label v-if="!form.local" class="flex items-start gap-3 text-sm text-text">
            <input v-model="form.allow_network" type="checkbox" class="mt-1" />
            <span>Allow sending note text to this provider. <span class="text-text-muted">Secrets are removed first.</span></span>
          </label>
          <div v-if="form.needs_key">
            <label class="block text-sm text-text-muted mb-1" for="pkey">API key (kept in the keychain)</label>
            <input id="pkey" v-model="form.key" type="password" autocomplete="off" data-1p-ignore data-lpignore="true" spellcheck="false" placeholder="paste it here" class="w-full bg-surface-2 border border-transparent rounded-xl px-3.5 py-2 text-sm text-text" />
          </div>
          <p v-if="formError" class="text-sm text-bad">{{ formError }}</p>
        </div>
        <template #footer>
          <Button variant="ghost" size="sm" @click="addOpen = false">Cancel</Button>
          <Button variant="primary" size="sm" :loading="busy === 'save'" :disabled="!form.name" @click="saveProfile">Save</Button>
        </template>
      </Modal>

      <!-- Change a key -->
      <Modal :is-open="keyOpen" title="API key" max-width-class="max-w-md" @close="closeKey">
        <div class="space-y-3">
          <p class="text-sm text-text-muted">For {{ keyFor }}. It goes straight to your keychain and is never shown again.</p>
          <input v-model="keyValue" type="password" autocomplete="off" data-1p-ignore data-lpignore="true" spellcheck="false" placeholder="paste it here" class="w-full bg-surface-2 border border-transparent rounded-xl px-3.5 py-2 text-sm text-text" @keydown.enter="saveKey" />
          <p v-if="formError" class="text-sm text-bad">{{ formError }}</p>
        </div>
        <template #footer>
          <Button variant="ghost" size="sm" @click="removeKey">Remove stored key</Button>
          <Button variant="ghost" size="sm" @click="closeKey">Cancel</Button>
          <Button variant="primary" size="sm" :disabled="!keyValue" @click="saveKey">Save</Button>
        </template>
      </Modal>
    </template>

    <Skeleton v-else height="h-40" />
  </div>
</template>

<script setup lang="ts">
import { computed, onBeforeUnmount, onMounted, reactive, ref } from 'vue';
import { humanFetch, request } from '~/utils/api';
import { useToast } from '~/composables/useToast';

const toast = useToast();
const state = ref<any>(null);
const loadError = ref('');
const busy = ref('');
const testResult = ref<any>(null);
const addOpen = ref(false);
const keyOpen = ref(false);
const keyFor = ref('');
const keyValue = ref('');
const formError = ref('');
let poll: ReturnType<typeof setInterval> | null = null;

const modes = [
  { id: 'keyword', title: 'Keyword', text: 'Fast and private. Finds notes by the words in them. No setup.' },
  { id: 'hybrid', title: 'Hybrid', text: 'Also finds notes by meaning, using an embedding model you choose.' },
];

const form = reactive({ preset: '', name: '', provider: 'ollama', model: '', endpoint: '', dimensions: null as number | null, allow_network: false, local: true, needs_key: false, key: '' });
const presetNote = computed(() => state.value?.presets.find((p: any) => p.id === form.preset)?.note || '');
const percent = computed(() => {
  const c = state.value?.coverage;
  return c && c.active ? Math.round((c.embedded / c.active) * 100) : 0;
});
const unusedCount = computed(() => (state.value?.coverage?.unused || []).reduce((n: number, u: any) => n + u.vectors, 0));

function describe(p: any): string {
  if (p.provider === 'ngram') return 'Simple built-in matching, works offline';
  const where = p.provider === 'ollama' ? 'on this computer' : p.provider === 'exec' ? 'your command' : p.endpoint || 'cloud';
  return `${p.model || 'model not set'} · ${where}`;
}

async function load() {
  const res = await request<any>('/api/recall');
  if (res.ok && res.data) {
    state.value = res.data;
    loadError.value = '';
    // While a job runs, refresh every second so the bar moves.
    if (res.data.indexing?.running && !poll) poll = setInterval(load, 1000);
    if (!res.data.indexing?.running && poll) {
      clearInterval(poll);
      poll = null;
    }
  } else {
    loadError.value = res.error || 'Unknown error';
  }
}

async function send(method: string, url: string, action: string, body?: unknown): Promise<{ ok: boolean; data: any }> {
  const res = await humanFetch(url, action, {
    method,
    headers: { 'Content-Type': 'application/json' },
    body: body === undefined ? undefined : JSON.stringify(body),
  });
  const data = await res.json().catch(() => ({}));
  return { ok: res.ok, data };
}

async function setMode(mode: string) {
  const r = await send('PUT', '/api/recall', 'change the search mode', { mode });
  if (r.ok) await load();
  else toast.error(r.data.error || 'Not changed');
}

async function useProfile(name: string) {
  const r = await send('PUT', '/api/recall', 'switch the embedding model', { active: name });
  if (r.ok) {
    toast.success('Switched. Index your memories to use it.');
    await load();
  } else toast.error(r.data.error || 'Not changed');
}

async function removeProfile(name: string) {
  const r = await send('DELETE', `/api/recall/profile/${encodeURIComponent(name)}`, 'remove an embedding model');
  if (r.ok) await load();
  else toast.error(r.data.error || 'Not removed');
}

function openAdd() {
  const first = state.value.presets[0];
  Object.assign(form, { preset: first.id, name: '', key: '', allow_network: false });
  applyPreset();
  formError.value = '';
  addOpen.value = true;
}

function applyPreset() {
  const p = state.value.presets.find((x: any) => x.id === form.preset);
  if (!p) return;
  Object.assign(form, { provider: p.provider, model: p.model, endpoint: p.endpoint, dimensions: p.dimensions, local: p.local, needs_key: !['ngram', 'ollama', 'exec'].includes(p.provider) && !p.local });
}

async function saveProfile() {
  formError.value = '';
  busy.value = 'save';
  try {
    const r = await send('POST', '/api/recall/profile', 'save an embedding model', {
      name: form.name, preset: form.preset, provider: form.provider, model: form.model, endpoint: form.endpoint,
      dimensions: form.dimensions, allow_network: form.allow_network,
    });
    if (!r.ok) {
      formError.value = r.data.error || 'Not saved';
      return;
    }
    if (form.needs_key && form.key) {
      const k = await send('PUT', '/api/recall/key', 'store an API key', { profile: form.name, key: form.key });
      if (!k.ok) formError.value = k.data.error || 'The key was not stored';
    }
    form.key = '';
    if (!formError.value) {
      addOpen.value = false;
      toast.success(`Saved ${form.name}.`);
    }
    await load();
  } finally {
    busy.value = '';
  }
}

function openKey(name: string) {
  keyFor.value = name;
  keyValue.value = '';
  formError.value = '';
  keyOpen.value = true;
}

function closeKey() {
  keyValue.value = '';
  keyOpen.value = false;
}

async function saveKey() {
  if (!keyValue.value) return;
  const r = await send('PUT', '/api/recall/key', 'store an API key', { profile: keyFor.value, key: keyValue.value });
  keyValue.value = '';
  if (r.ok) {
    keyOpen.value = false;
    toast.success('Key saved in the keychain.');
    await load();
  } else formError.value = r.data.error || 'Not stored';
}

async function removeKey() {
  const r = await send('DELETE', `/api/recall/key?profile=${encodeURIComponent(keyFor.value)}`, 'remove an API key');
  if (r.ok) {
    closeKey();
    await load();
  }
}

async function runTest() {
  busy.value = 'test';
  try {
    const r = await send('POST', '/api/recall/test', 'test the embedding model');
    testResult.value = r.ok ? r.data : { ok: false, error: r.data.error || 'Not allowed' };
  } finally {
    busy.value = '';
  }
}

async function startIndex(all: boolean) {
  busy.value = 'index';
  try {
    const r = await send('POST', '/api/recall/index', 'index memories', { all });
    if (!r.ok) toast.error(r.data.error || 'Not started');
    await load();
  } finally {
    busy.value = '';
  }
}

async function cancelIndex() {
  await send('POST', '/api/recall/index/cancel', 'stop indexing');
}

async function cleanUnused() {
  const r = await send('POST', '/api/recall/clean', 'remove unused vectors');
  if (r.ok) {
    toast.success(`Removed ${r.data.removed} vectors.`);
    await load();
  }
}

onMounted(load);
onBeforeUnmount(() => {
  if (poll) clearInterval(poll);
});
</script>
