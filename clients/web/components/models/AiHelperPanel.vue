<template>
  <div class="space-y-8">
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
          <div class="grid gap-3 sm:grid-cols-3">
            <label class="text-xs text-text-muted">Setup
              <select v-model="llmForm.name" class="mt-1 w-full rounded-xl bg-surface-2 px-3 py-2 text-sm text-text" @change="pickProfile">
                <option v-for="p in llm.profiles" :key="p.name" :value="p.name">{{ p.name }}{{ p.name === llm.active ? ' (in use)' : '' }}</option>
                <option value="__new">New setup…</option>
              </select>
            </label>
            <label class="text-xs text-text-muted">Model
              <input v-model="llmForm.model" class="mt-1 w-full rounded-xl bg-surface-2 px-3 py-2 text-sm text-text" placeholder="gpt-4o-mini or llama3.1" />
            </label>
            <label class="text-xs text-text-muted">Address (blank = OpenAI)
              <input v-model="llmForm.endpoint" class="mt-1 w-full rounded-xl bg-surface-2 px-3 py-2 text-sm text-text" placeholder="http://127.0.0.1:11434/v1" />
            </label>
          </div>
          <label v-if="llmForm.name === '__new'" class="block text-xs text-text-muted">Name for the new setup
            <input v-model="llmForm.newName" class="mt-1 w-full rounded-xl bg-surface-2 px-3 py-2 text-sm text-text" placeholder="local or cloud" />
          </label>
          <label class="flex items-center gap-2 text-sm text-text">
            <input v-model="llmForm.allow_network" type="checkbox" /> Allow sending note text to a service outside this computer
          </label>
          <div class="flex flex-wrap gap-2">
            <Button size="sm" variant="primary" lock="/api/llm" @click="saveLlm">{{ llm.on ? 'Save and use' : 'Turn on' }}</Button>
            <Button v-if="llm.on" size="sm" variant="secondary" @click="saveLlm(true)">Turn off</Button>
            <Button size="sm" variant="secondary" lock="/api/llm/key" @click="setLlmKey">{{ currentKey?.set ? 'Replace API key' : 'Set API key' }}</Button>
            <Button v-if="llmForm.name !== 'default' && llmForm.name !== '__new'" size="sm" variant="danger" lock="/api/llm/profile/:name" @click="removeProfile">Remove this setup</Button>
          </div>
          <p v-if="currentKey?.set" class="text-xs text-text-muted">API key stored in: {{ currentKey.where }}. It is never shown again.</p>
          <div v-if="llm.on" class="rounded-xl bg-surface-2/60 px-4 py-3 space-y-2">
            <div class="flex flex-wrap gap-2">
              <Button size="sm" variant="secondary" :loading="latest('llm')?.state === 'running'" @click="runLlm('pairs')">Find conflicts and duplicates</Button>
              <Button size="sm" variant="secondary" :loading="latest('llm')?.state === 'running'" @click="runLlm('distill')">Suggest notes from sessions</Button>
              <Button size="sm" variant="secondary" :loading="latest('llm')?.state === 'running'" @click="runLlm('brief')">Draft the hand-off brief from the latest chat</Button>
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
import { computed, onMounted, reactive, ref } from 'vue';
import { request, safeCall, protectedCall } from '~/utils/api';
import { useJobs } from '~/composables/useJobs';
import { useToast } from '~/composables/useToast';

const { latest, refresh } = useJobs();
const toast = useToast();
const stateLabel = (s: string) => ({ running: 'Running…', done: 'Finished', failed: 'Failed', cancelled: 'Stopped' } as Record<string, string>)[s] || s;

const llm = ref<any>(null);
const llmForm = reactive({ name: 'default', newName: '', model: '', endpoint: '', allow_network: false });
const currentKey = computed(() => llm.value?.profiles?.find((p: any) => p.name === llmForm.name)?.key);
const formName = () => (llmForm.name === '__new' ? llmForm.newName.trim() : llmForm.name);

function pickProfile() {
  const p = llm.value?.profiles?.find((x: any) => x.name === llmForm.name);
  Object.assign(llmForm, p ? { model: p.model, endpoint: p.endpoint, allow_network: p.allow_network } : { model: '', endpoint: '', allow_network: false });
}

async function loadLlm() {
  const r = await request<any>('/api/llm');
  if (r.ok) {
    llm.value = r.data;
    llmForm.name = r.data.active;
    pickProfile();
  }
}
onMounted(loadLlm);

async function saveLlm(off = false) {
  const name = formName();
  if (off !== true && !name) return toast.error('Give the new setup a name');
  const body = off === true ? { off: true } : { name, model: llmForm.model, endpoint: llmForm.endpoint, allow_network: llmForm.allow_network, activate: true };
  const r = await protectedCall('PUT', '/api/llm', 'change the AI helper settings', body);
  if (r.ok) await loadLlm();
}

async function setLlmKey() {
  const key = window.prompt('Paste the API key. It goes to your keychain and is never shown again.');
  if (!key) return;
  const r = await protectedCall('PUT', '/api/llm/key', 'store the AI helper API key', { key, profile: formName() });
  if (r.ok) await loadLlm();
}

async function removeProfile() {
  const r = await protectedCall('DELETE', `/api/llm/profile/${encodeURIComponent(llmForm.name)}`, 'remove an AI helper setup');
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

</script>
