<template>
  <Card title="Git backup">
    <template #actions>
      <Badge v-if="status" :variant="badge.variant" dot>{{ badge.text }}</Badge>
    </template>

    <p v-if="loading && !status" class="text-sm text-text-muted">Loading…</p>

    <!-- The recovery phrase, shown once right after setup -->
    <div v-else-if="phrase" class="space-y-4" aria-live="polite">
      <div class="rounded-xl bg-ok-bg text-ok px-4 py-3 text-sm font-medium">
        Backed up to {{ done?.provider }}.
        <span v-if="done?.verified">Checked: the copy restores ({{ done.verified.remote_notes }} notes, {{ done.verified.remote_decisions }} decisions).</span>
        <span v-else-if="done?.note">{{ done.note }}</span>
      </div>
      <div>
        <h4 class="text-sm font-semibold text-text">Your recovery phrase</h4>
        <p class="text-sm text-text-muted mt-1">It is the only way to restore this backup on another computer. Nobody can recover it for you. Store it in a password manager now.</p>
        <ol class="mt-3 grid grid-cols-2 sm:grid-cols-3 gap-x-4 gap-y-1.5 rounded-xl bg-surface-2 p-4 font-mono text-sm text-text">
          <li v-for="(w, i) in phraseWords" :key="i"><span class="text-text-muted select-none">{{ i + 1 }}.</span> {{ w }}</li>
        </ol>
        <div class="mt-3 flex items-center gap-3 flex-wrap">
          <Button size="sm" variant="secondary" @click="copyPhrase">{{ copied ? 'Copied' : 'Copy the phrase' }}</Button>
          <label class="flex items-center gap-2 text-sm text-text min-h-11 sm:min-h-0">
            <input v-model="saved" type="checkbox" class="w-4 h-4" />
            I saved the recovery phrase
          </label>
        </div>
      </div>
      <Button size="sm" variant="primary" :disabled="!saved" @click="finish">Done</Button>
    </div>

    <!-- Not set up -->
    <div v-else-if="status && status.state === 'off'" class="space-y-4">
      <p class="text-sm text-text-muted leading-relaxed">
        Keep an encrypted copy of your memory in a private git repository: GitHub, GitLab, Bitbucket or any git server. It is encrypted on this computer before it leaves, and AutoMemory keeps it up to date by itself.
      </p>

      <div>
        <p class="text-xs font-medium text-text-muted mb-1.5">1. Create an empty, private repository</p>
        <div class="flex gap-1.5 flex-wrap" role="group" aria-label="Git provider">
          <button
            v-for="p in providers"
            :key="p.name"
            type="button"
            class="px-3 min-h-10 sm:min-h-0 sm:py-1 rounded-pill text-xs font-medium border transition-colors"
            :class="provider === p.name ? 'bg-accent-solid text-accent-fg border-transparent' : 'bg-surface-2 text-text-muted border-transparent hover:text-text'"
            :aria-pressed="provider === p.name"
            @click="provider = p.name"
          >{{ p.name === 'your git server' ? 'Other' : p.name }}</button>
        </div>
        <p class="mt-2 text-sm text-text-muted">{{ hint }}</p>
      </div>

      <form class="space-y-2" @submit.prevent="setup(false, false, false)">
        <label for="git-backup-url" class="block text-xs font-medium text-text-muted">2. Paste its address</label>
        <div class="flex gap-2 flex-wrap sm:flex-nowrap">
          <input
            id="git-backup-url"
            v-model="url"
            type="text"
            inputmode="url"
            autocomplete="off"
            spellcheck="false"
            placeholder="git@github.com:you/memory-vault.git"
            class="flex-1 min-w-0 bg-surface-2 border border-transparent rounded-xl px-3.5 min-h-11 sm:min-h-0 sm:py-2 text-sm text-text font-mono focus:border-accent focus:outline-none"
          />
          <Button type="submit" variant="primary" lock="/api/backup/git/setup" :loading="busy" :disabled="!url.trim()">Set up backup</Button>
        </div>
      </form>

      <div v-if="issue" class="rounded-xl bg-warn-bg text-warn px-4 py-3 text-sm space-y-2" role="alert">
        <p>{{ issue.error }}</p>
        <div v-if="issue.code === 'public' || issue.code === 'not_empty' || issue.code === 'secret'" class="flex gap-2 flex-wrap">
          <Button size="sm" variant="secondary" :loading="busy" lock="/api/backup/git/setup" @click="setup(issue.code === 'public', issue.code === 'not_empty', issue.code === 'secret')">Use it anyway</Button>
        </div>
      </div>
      <p class="text-xs text-text-muted">Other computer? <code class="font-mono bg-surface-2 rounded px-1.5 py-0.5">automemory backup git restore &lt;address&gt;</code> restores from the repository with your recovery phrase.</p>
    </div>

    <!-- Set up: how it is doing -->
    <div v-else-if="status" class="space-y-4">
      <dl class="grid grid-cols-1 sm:grid-cols-2 gap-x-8 gap-y-2.5 text-sm">
        <div class="flex justify-between gap-4 min-w-0"><dt class="text-text-muted flex-shrink-0">Kept on</dt><dd class="text-text text-right min-w-0 break-all font-mono text-xs" :title="status.remote || ''">{{ status.provider_name }}: {{ status.remote }}</dd></div>
        <div class="flex justify-between gap-4"><dt class="text-text-muted">Last backed up</dt><dd class="text-text">{{ ago(status.last_ok) || 'not yet' }}</dd></div>
        <div class="flex justify-between gap-4"><dt class="text-text-muted">Restore checked</dt><dd class="text-text">{{ ago(status.last_verified) || 'never' }}</dd></div>
        <div class="flex justify-between gap-4"><dt class="text-text-muted">Key on this computer</dt><dd class="text-text">{{ status.key_present ? 'yes' : 'no' }}</dd></div>
      </dl>

      <div v-if="status.last_error && (status.state === 'failing' || status.state === 'never')" class="rounded-xl bg-warn-bg text-warn px-4 py-3 text-sm" role="alert">
        The last attempt{{ status.last_error_at ? ' (' + ago(status.last_error_at) + ')' : '' }} did not work: {{ status.last_error }}
      </div>
      <div v-else-if="status.state === 'stale'" class="rounded-xl bg-warn-bg text-warn px-4 py-3 text-sm" role="alert">
        No successful backup for over a week. AutoMemory backs up after your agents work; if you have, check the repository and your sign-in.
      </div>

      <div class="flex gap-2 flex-wrap items-center">
        <Button size="sm" variant="primary" :loading="nowBusy" @click="backupNow">Back up now</Button>
        <Button size="sm" variant="secondary" :loading="verifyBusy" @click="verify">Check that it restores</Button>
        <Button size="sm" variant="ghost" lock="/api/backup/git" :loading="offBusy" @click="turnOff">Turn off</Button>
      </div>
      <p v-if="result" class="text-sm" :class="result.ok ? 'text-ok' : 'text-warn'" role="status">{{ result.text }}</p>

      <details class="text-sm">
        <summary class="cursor-pointer text-accent font-medium">Restore on another computer</summary>
        <div class="mt-2 space-y-2 text-text-muted">
          <p>Install AutoMemory there, then run this with your recovery phrase (it asks for the phrase if you leave it out):</p>
          <CopyCommand :command="`automemory backup git restore ${status.remote || '<address>'}`" />
          <p>Lost the phrase? It is stored on this computer: <code class="font-mono bg-surface-2 rounded px-1.5 py-0.5">automemory sync pair</code> shows it again.</p>
        </div>
      </details>
    </div>
  </Card>
</template>

<script setup lang="ts">
import { ref, computed, onMounted, onBeforeUnmount } from 'vue';
import { request, protectedCall, safeCall } from '~/utils/api';
import { formatRelativeTime } from '~/utils/memoryText';

interface Status {
  state: 'off' | 'never' | 'ok' | 'stale' | 'failing';
  remote?: string | null;
  provider_name?: string | null;
  last_ok?: string | null;
  last_error?: string | null;
  last_error_at?: string | null;
  last_verified?: string | null;
  key_present: boolean;
}
interface Provider { id: string; name: string; hint: string }

const status = ref<Status | null>(null);
const providers = ref<Provider[]>([]);
const loading = ref(true);
const provider = ref('GitHub');
const url = ref('');
const busy = ref(false);
const nowBusy = ref(false);
const verifyBusy = ref(false);
const offBusy = ref(false);
const issue = ref<{ code: string; error: string } | null>(null);
const result = ref<{ ok: boolean; text: string } | null>(null);
const phrase = ref('');
const done = ref<any>(null);
const saved = ref(false);
const copied = ref(false);
let timer: ReturnType<typeof setInterval> | null = null;

const phraseWords = computed(() => phrase.value.split(/\s+/).filter(Boolean));
const hint = computed(() => providers.value.find((p) => p.name === provider.value)?.hint || '');
const badge = computed(() => {
  switch (status.value?.state) {
    case 'ok': return { variant: 'ok', text: 'Working' };
    case 'failing': return { variant: 'bad', text: 'Failing' };
    case 'stale': return { variant: 'warn', text: 'Out of date' };
    case 'never': return { variant: 'warn', text: 'Waiting for first backup' };
    default: return { variant: 'neutral', text: 'Off' };
  }
});

function ago(t?: string | null) {
  return t ? formatRelativeTime(t) : '';
}

async function load() {
  const r = await request<{ status: Status; providers: Provider[] }>('/api/backup/git');
  loading.value = false;
  if (r.ok && r.data) {
    status.value = r.data.status;
    providers.value = r.data.providers;
  }
}

async function setup(allowPublic: boolean, allowNonEmpty: boolean, allowSecret: boolean) {
  busy.value = true;
  issue.value = null;
  try {
    const r = await protectedCall('POST', '/api/backup/git/setup', 'set up the git backup', {
      url: url.value.trim(), allow_public: allowPublic, allow_non_empty: allowNonEmpty, allow_secret: allowSecret,
    });
    if (r.ok && r.data?.recovery_phrase) {
      phrase.value = r.data.recovery_phrase;
      done.value = r.data;
      saved.value = false;
      url.value = '';
      await load();
    } else if (r.data?.code) {
      issue.value = { code: r.data.code, error: r.data.error };
    } else {
      issue.value = { code: 'failed', error: r.error || 'The setup did not work.' };
    }
  } finally {
    busy.value = false;
  }
}

function finish() {
  phrase.value = '';
  done.value = null;
}

async function copyPhrase() {
  try {
    await navigator.clipboard.writeText(phrase.value);
    copied.value = true;
    setTimeout(() => (copied.value = false), 1500);
  } catch { /* the words are on screen */ }
}

async function backupNow() {
  nowBusy.value = true;
  result.value = null;
  try {
    await safeCall('POST', '/api/backup/git/now');
    // the pass runs in the background; look again shortly
    await new Promise((r) => setTimeout(r, 2500));
    await load();
    result.value = { ok: status.value?.state === 'ok', text: status.value?.state === 'ok' ? 'Backed up.' : 'Started. If it does not finish, the reason shows above.' };
  } finally {
    nowBusy.value = false;
  }
}

async function verify() {
  verifyBusy.value = true;
  result.value = null;
  try {
    const r = await safeCall('POST', '/api/backup/git/verify');
    if (r.ok && r.data?.verification) {
      const v = r.data.verification;
      result.value = { ok: true, text: `The copy on the remote restores: ${v.remote_notes} notes and ${v.remote_decisions} decisions.${v.not_yet_backed_up ? ` ${v.not_yet_backed_up} newer note(s) here are not on it yet.` : ''}` };
    } else {
      result.value = { ok: false, text: r.data?.error || r.error || 'The check failed.' };
    }
    await load();
  } finally {
    verifyBusy.value = false;
  }
}

async function turnOff() {
  offBusy.value = true;
  try {
    const r = await protectedCall('DELETE', '/api/backup/git', 'turn the git backup off');
    if (r.ok) {
      result.value = null;
      await load();
    } else {
      result.value = { ok: false, text: r.error || 'Could not turn it off.' };
    }
  } finally {
    offBusy.value = false;
  }
}

onMounted(() => {
  load();
  timer = setInterval(() => { if (!phrase.value) load(); }, 15000);
});
onBeforeUnmount(() => { if (timer) clearInterval(timer); });
</script>
