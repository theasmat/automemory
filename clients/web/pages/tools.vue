<template>
  <div class="space-y-8 max-w-3xl">
    <PageHeader title="Tools" description="Run maintenance from here. Everything on this page is safe: it never asks for your password." />

    <!-- Instruction files -->
    <section class="space-y-3" aria-labelledby="rules-h">
      <h2 id="rules-h" class="text-lg font-semibold text-text">Instruction files</h2>
      <Card>
        <p class="text-sm text-text-muted">Puts what your project knows into AGENTS.md, CLAUDE.md and the other files agents read. Only a marked block changes.</p>
        <ul v-if="plan" class="mt-4 divide-y divide-border/60">
          <li v-for="f in plan.files" :key="f.path" class="py-2 flex items-center justify-between gap-3 text-sm">
            <span class="font-mono text-text truncate">{{ f.path }}</span>
            <Badge :variant="f.status === 'unchanged' ? 'neutral' : 'accent'">{{ plan.applied ? statusDone[f.status] : statusWill[f.status] }}</Badge>
          </li>
        </ul>
        <details v-if="plan?.block" class="mt-3">
          <summary class="text-sm text-accent cursor-pointer">Show the block that will be written</summary>
          <pre class="mt-2 text-xs bg-surface-2 rounded-xl p-3 whitespace-pre-wrap max-h-64 overflow-auto">{{ plan.block }}</pre>
        </details>
        <div class="mt-4 flex gap-2">
          <Button size="sm" variant="secondary" :loading="busy === 'preview'" @click="previewRules">Preview</Button>
          <Button size="sm" variant="primary" :loading="busy === 'apply'" @click="applyRules">Write the files</Button>
        </div>
      </Card>
    </section>

    <!-- Hand-off -->
    <section class="space-y-3" aria-labelledby="hand-h">
      <h2 id="hand-h" class="text-lg font-semibold text-text">Hand-off</h2>
      <Card>
        <p class="text-sm text-text-muted">Leaves a note for the next session, on this or another computer.</p>
        <textarea v-model="note" rows="2" placeholder="Next: add the SAVE20 coupon and a test" class="mt-3 w-full bg-surface-2 border border-transparent rounded-xl px-3.5 py-2 text-sm text-text" />
        <div class="mt-3 flex items-center gap-3">
          <Button size="sm" variant="primary" :loading="busy === 'handoff'" @click="writeHandoff">Write hand-off</Button>
          <span v-if="handoffDone" class="text-sm text-ok">Saved.</span>
        </div>
      </Card>
    </section>

    <!-- Jobs -->
    <section class="space-y-3" aria-labelledby="jobs-h">
      <h2 id="jobs-h" class="text-lg font-semibold text-text">Maintenance</h2>
      <Card>
        <ul class="divide-y divide-border/60">
          <li v-for="t in tools" :key="t.kind" class="py-4 first:pt-0 last:pb-0 space-y-2">
            <div class="flex flex-wrap items-center justify-between gap-3">
              <div class="min-w-0">
                <p class="text-[15px] font-medium text-text">{{ t.title }}</p>
                <p class="text-sm text-text-muted">{{ t.text }}</p>
              </div>
              <Button size="sm" variant="secondary" :loading="latest(t.kind)?.state === 'running'" @click="run(t)">{{ t.button }}</Button>
            </div>
            <div v-if="latest(t.kind)" class="rounded-xl bg-surface-2/60 px-4 py-3">
              <p class="text-xs" :class="latest(t.kind)!.state === 'failed' ? 'text-bad' : 'text-text-muted'">
                {{ stateLabel(latest(t.kind)!.state) }}
              </p>
              <pre v-if="latest(t.kind)!.message" class="mt-1 text-xs text-text whitespace-pre-wrap max-h-56 overflow-auto">{{ latest(t.kind)!.message }}</pre>
            </div>
          </li>
        </ul>
      </Card>
    </section>

    <!-- Optional AI helper -->
    <section class="space-y-3" aria-labelledby="llm-h">
      <h2 id="llm-h" class="text-lg font-semibold text-text">Suggestions (optional AI helper)</h2>
      <Card>
        <p class="text-sm text-text-muted">
          Off by default. When on, a model reads your notes and <em>suggests</em> conflicts, duplicates and new notes.
          Nothing changes until you approve it in the Inbox.
        </p>
        <div v-if="llm" class="mt-4 space-y-4">
          <p v-if="llm.on" class="text-sm text-text">
            {{ llm.model }} &middot; {{ llm.local ? 'runs on this computer' : 'sends note text off this computer (secrets removed first)' }}
            &middot; {{ llm.tokens_today }} of {{ llm.max_tokens_per_day }} tokens today
          </p>
          <p v-if="llm.on && llm.problem" class="text-sm text-bad">{{ llm.problem }}</p>
          <div class="grid gap-3 sm:grid-cols-2">
            <label class="text-xs text-text-muted">Model
              <input v-model="llmForm.model" class="mt-1 w-full rounded-xl bg-surface-2 px-3 py-2 text-sm text-text" placeholder="gpt-4o-mini or llama3.1" />
            </label>
            <label class="text-xs text-text-muted">Address (blank = OpenAI)
              <input v-model="llmForm.endpoint" class="mt-1 w-full rounded-xl bg-surface-2 px-3 py-2 text-sm text-text" placeholder="http://127.0.0.1:11434/v1" />
            </label>
          </div>
          <label class="flex items-center gap-2 text-sm text-text">
            <input v-model="llmForm.allow_network" type="checkbox" /> Allow sending note text to a service outside this computer
          </label>
          <div class="flex flex-wrap gap-2">
            <Button size="sm" variant="primary" @click="saveLlm">{{ llm.on ? 'Save' : 'Turn on' }}</Button>
            <Button v-if="llm.on" size="sm" variant="secondary" @click="saveLlm(true)">Turn off</Button>
            <Button size="sm" variant="secondary" @click="setLlmKey">{{ llm.key.set ? 'Replace API key' : 'Set API key' }}</Button>
          </div>
          <p v-if="llm.key.set" class="text-xs text-text-muted">API key stored in: {{ llm.key.where }}. It is never shown again.</p>
          <div v-if="llm.on" class="rounded-xl bg-surface-2/60 px-4 py-3 space-y-2">
            <div class="flex flex-wrap gap-2">
              <Button size="sm" variant="secondary" :loading="latest('llm')?.state === 'running'" @click="runLlm('pairs')">Find conflicts and duplicates</Button>
              <Button size="sm" variant="secondary" :loading="latest('llm')?.state === 'running'" @click="runLlm('distill')">Suggest notes from sessions</Button>
            </div>
            <p v-if="latest('llm')" class="text-xs" :class="latest('llm')!.state === 'failed' ? 'text-bad' : 'text-text-muted'">
              {{ stateLabel(latest('llm')!.state) }} {{ latest('llm')!.message }}
            </p>
          </div>
        </div>
      </Card>
    </section>
  </div>
</template>

<script setup lang="ts">
import { ref } from 'vue';
import { onMounted, reactive } from 'vue';
import { request, safeCall, protectedCall } from '~/utils/api';
import { useJobs } from '~/composables/useJobs';
import { useToast } from '~/composables/useToast';

const { latest, refresh } = useJobs();
const toast = useToast();
const busy = ref('');
const plan = ref<any>(null);
const note = ref('');
const handoffDone = ref(false);

const statusWill: Record<string, string> = { created: 'Will be created', updated: 'Will be updated', unchanged: 'Up to date' };
const statusDone: Record<string, string> = { created: 'Created', updated: 'Updated', unchanged: 'Up to date' };
const stateLabel = (s: string) => ({ running: 'Running…', done: 'Finished', failed: 'Failed', cancelled: 'Stopped' } as Record<string, string>)[s] || s;

const tools = [
  { kind: 'doctor', title: 'Health check', text: 'Checks the database, hooks, search and the code engine.', button: 'Run', url: '/api/tools/doctor' },
  { kind: 'sync-pull', title: 'Pull from the remote', text: 'Brings in changes from your other computers. Nothing is sent.', button: 'Pull', url: '/api/sync/pull' },
  { kind: 'docs', title: 'Rescan documents', text: 'Finds specs and decision records in the project again.', button: 'Rescan', url: '/api/tools/docs-rescan' },
  { kind: 'graph', title: 'Index the code graph', text: 'Maps files and symbols so agents see what a change touches.', button: 'Index', url: '/api/tools/graph-index' },
  { kind: 'eval', title: 'Run the checks', text: 'Runs the built-in quality checks for search, locks and speed.', button: 'Run', url: '/api/tools/eval' },
  { kind: 'backup', title: 'Back up the database', text: 'Saves a copy in ~/.automemory/backups.', button: 'Back up', url: '/api/tools/backup' },
];

async function run(t: { url: string; title: string }) {
  const r = await safeCall('POST', t.url);
  if (!r.ok) toast.error(r.error || `${t.title} could not start`);
  await refresh();
}

const llm = ref<any>(null);
const llmForm = reactive({ model: '', endpoint: '', allow_network: false });

async function loadLlm() {
  const r = await request<any>('/api/llm');
  if (r.ok) {
    llm.value = r.data;
    Object.assign(llmForm, { model: r.data.model, endpoint: r.data.endpoint, allow_network: r.data.allow_network });
  }
}
onMounted(loadLlm);

async function saveLlm(off = false) {
  const body = off === true ? { model: '' } : { ...llmForm };
  const r = await protectedCall('PUT', '/api/llm', 'change the AI helper settings', body);
  if (r.ok) await loadLlm();
}

async function setLlmKey() {
  const key = window.prompt('Paste the API key. It goes to your keychain and is never shown again.');
  if (!key) return;
  const r = await protectedCall('PUT', '/api/llm/key', 'store the AI helper API key', { key });
  if (r.ok) await loadLlm();
}

async function runLlm(what: string) {
  const body = { what };
  const r = llm.value?.local
    ? await safeCall('POST', '/api/llm/run', body)
    : await protectedCall('POST', '/api/llm/run', 'send note text to the AI helper', body);
  if (!r.ok) toast.error(r.error || 'Could not start');
  await refresh();
}

async function previewRules() {
  busy.value = 'preview';
  try {
    const res = await request<any>('/api/tools/export-rules');
    if (res.ok) plan.value = res.data;
    else toast.error(res.error || 'No project yet');
  } finally {
    busy.value = '';
  }
}

async function applyRules() {
  busy.value = 'apply';
  try {
    const r = await safeCall('POST', '/api/tools/export-rules');
    if (r.ok) plan.value = r.data;
    else toast.error(r.error || 'Not written');
  } finally {
    busy.value = '';
  }
}

async function writeHandoff() {
  busy.value = 'handoff';
  handoffDone.value = false;
  try {
    const r = await safeCall('POST', '/api/tools/handoff', { note: note.value || undefined });
    handoffDone.value = r.ok;
    if (!r.ok) toast.error(r.error || 'Not saved');
    else note.value = '';
  } finally {
    busy.value = '';
  }
}
</script>
