<template>
  <div class="space-y-6">
    <PageHeader title="Library" description="Documents for your agents: your PRDs, plans and notes that stay out of git (synced between your computers), and the project documents found in the repository.">
      <template #actions>
        <Button size="sm" variant="secondary" :loading="busy === 'rescan'" @click="rescan">Rescan project documents</Button>
        <Button size="sm" variant="secondary" :loading="busy === 'sync'" @click="syncNow">Sync folder</Button>
        <Button size="sm" variant="primary" lock="/api/library/file" @click="openNew">New file</Button>
      </template>
    </PageHeader>

    <EmptyState v-if="data && !data.project" title="No project yet" description="Connect an agent in a project folder first (Agents page)." />

    <template v-else-if="data">
      <!-- Is the project folder clean? -->
      <p v-if="data.folder.exists && data.folder.exclude_from_git && !data.folder.out_of_git_status" class="rounded-xl bg-warn-bg px-4 py-2 text-sm text-warn">
        <code>.automemory/library/</code> would show in your git status. Click Sync folder to hide it (it adds a line to <code>.git/info/exclude</code>, never <code>.gitignore</code>).
      </p>
      <p v-else-if="data.folder.exists" class="text-xs text-text-muted">
        Agents without MCP can read <code>.automemory/library/INDEX.md</code>.
        {{ data.folder.out_of_git_status ? 'The folder does not show in git status.' : 'Git is left alone (your setting).' }}
      </p>
      <ul v-if="data.problems.length" class="space-y-1 rounded-xl bg-warn-bg px-4 py-2 text-sm text-warn">
        <li v-for="p in data.problems" :key="p">{{ p }}</li>
      </ul>
      <p v-if="syncNote" class="text-sm text-ok">{{ syncNote }}</p>

      <div class="grid gap-4 lg:grid-cols-[18rem_1fr]">
        <!-- Tree -->
        <Card>
          <p v-if="!data.files.length && !data.project_docs?.length" class="text-sm text-text-muted">The library is empty. Add a file, or list files to keep in place below.</p>
          <div v-for="g in groups" :key="g.folder" class="mb-3 last:mb-0">
            <p v-if="g.folder" class="mb-1 text-xs font-semibold uppercase tracking-wide text-text-muted">{{ g.folder }}</p>
            <ul>
              <li v-for="f in g.files" :key="f.path">
                <button type="button" class="w-full rounded-lg px-2 py-1.5 text-left text-sm hover:bg-surface-2" :class="sel === f.path ? 'bg-surface-2 font-semibold text-text' : 'text-text-muted'" @click="open(f.path)">
                  {{ f.path.split('/').pop() }}
                  <Badge v-if="f.linked" variant="info">in place</Badge>
                  <Badge v-if="f.personal" variant="neutral">this computer</Badge>
                  <Badge v-if="f.origin === 'agent'" variant="warn">agent</Badge>
                </button>
              </li>
            </ul>
          </div>
          <div v-if="data.project_docs?.length" class="mt-4 border-t border-border/60 pt-3">
            <p class="mb-1 text-xs font-semibold uppercase tracking-wide text-text-muted">Project documents (in the repository)</p>
            <ul>
              <li v-for="d in data.project_docs" :key="d.path">
                <button type="button" class="w-full rounded-lg px-2 py-1.5 text-left text-sm hover:bg-surface-2" :class="sel === d.path ? 'bg-surface-2 font-semibold text-text' : 'text-text-muted'" @click="open(d.path)">
                  {{ d.path }} <Badge variant="neutral">{{ d.category }}</Badge>
                </button>
              </li>
            </ul>
          </div>
        </Card>

        <!-- File -->
        <Card v-if="file" :title="file.path">
          <p class="text-xs text-text-muted">
            <template v-if="file.source === 'repo'">A project document in the repository (git is its home); read it here, edit it in your editor.</template>
            <template v-else>{{ file.linked ? 'Kept where it is in the project folder; edit it there.' : `Written by ${file.author === 'human' ? 'you' : file.author}` }} · updated {{ (file.updated_at || '').slice(0, 16).replace('T', ' ') }}</template>
          </p>
          <Tabs v-model="tier" class="mt-3" :tabs="file.source === 'repo' ? [{ id: 'summary', label: 'Summary' }, { id: 'outline', label: 'Outline' }, { id: 'full', label: 'Whole file' }] : [{ id: 'summary', label: 'Summary' }, { id: 'outline', label: 'Outline' }, { id: 'full', label: 'Whole file' }, { id: 'history', label: 'History', count: file.revisions.length }]" />

          <div class="mt-3">
            <p v-if="tier === 'summary'" class="text-sm text-text">{{ file.summary }}</p>
            <pre v-else-if="tier === 'outline'" class="max-h-96 overflow-auto whitespace-pre-wrap rounded-xl bg-surface-2/60 px-4 py-3 text-xs text-text">{{ file.outline || '(no headings)' }}</pre>
            <template v-else-if="tier === 'full'">
              <textarea v-if="editing" v-model="draft" rows="18" class="w-full rounded-xl bg-surface-2 px-3 py-2 font-mono text-xs text-text" aria-label="File text" />
              <pre v-else class="max-h-[32rem] overflow-auto whitespace-pre-wrap rounded-xl bg-surface-2/60 px-4 py-3 text-xs text-text">{{ file.content }}</pre>
              <div class="mt-3 flex flex-wrap gap-2">
                <template v-if="editing">
                  <Button size="sm" variant="primary" lock="/api/library/file" @click="save">Save</Button>
                  <Button size="sm" variant="ghost" @click="editing = false">Cancel</Button>
                </template>
                <Button v-else-if="!file.linked && file.source !== 'repo'" size="sm" variant="secondary" lock="/api/library/file" @click="startEdit">Edit</Button>
                <Button v-if="!editing && file.source !== 'repo'" size="sm" variant="ghost" lock="/api/library/file" @click="remove">Remove from library</Button>
              </div>
            </template>
            <ul v-else class="divide-y divide-border/60">
              <li v-for="r in file.revisions" :key="r.rev" class="flex flex-wrap items-center justify-between gap-2 py-2 text-sm">
                <span class="text-text">Revision {{ r.rev }} <span class="text-xs text-text-muted">{{ r.at.slice(0, 16).replace('T', ' ') }} · {{ r.author }}</span></span>
                <span class="flex gap-1">
                  <Button size="sm" variant="ghost" @click="showRev(r.rev)">View</Button>
                  <Button v-if="!file.linked && r.rev !== file.revisions[0]?.rev" size="sm" variant="ghost" lock="/api/library/revert" @click="revert(r.rev)">Restore</Button>
                </span>
              </li>
              <li v-if="revText" class="pt-3"><pre class="max-h-72 overflow-auto whitespace-pre-wrap rounded-xl bg-surface-2/60 px-4 py-3 text-xs text-text">{{ revText }}</pre></li>
            </ul>
          </div>
        </Card>
        <Card v-else><p class="text-sm text-text-muted">Pick a file to read it at three levels: a one-line summary, an outline, or the whole file. Agents get the same levels from <code>library.get</code>.</p></Card>
      </div>

      <!-- Files kept in place -->
      <Card title="Keep files where they are">
        <p class="text-sm text-text-muted">Name files in your project folder (relative paths or patterns). AutoMemory reads and syncs them in place and does not care whether git tracks or ignores them.</p>
        <ul class="mt-3 space-y-1 text-sm">
          <li v-for="p in allPatterns" :key="p" class="flex items-center justify-between gap-3">
            <code class="text-text">{{ p }}</code>
            <Button size="sm" variant="ghost" lock="/api/library/link" @click="unlink(p)">Stop</Button>
          </li>
          <li v-if="!allPatterns.length" class="text-text-muted">None yet.</li>
        </ul>
        <form class="mt-3 flex gap-2" @submit.prevent="addPattern">
          <input v-model="pattern" class="flex-1 rounded-xl bg-surface-2 px-3 py-2 text-sm text-text" placeholder="PRD-private.md  or  plans/**/*.md" aria-label="Pattern" />
          <Button size="sm" variant="secondary" type="submit" lock="/api/library/link" :disabled="!pattern.trim()">Add</Button>
        </form>
      </Card>
    </template>

    <Modal :is-open="newOpen" title="New library file" max-width-class="max-w-2xl" @close="newOpen = false">
      <form class="space-y-4" @submit.prevent="create">
        <label class="block text-xs text-text-muted">Path in the library
          <input v-model="nf.path" required class="mt-1 w-full rounded-xl bg-surface-2 px-3 py-2 text-sm text-text" placeholder="plans/prd-cart.md" />
        </label>
        <label class="block text-xs text-text-muted">Text
          <textarea v-model="nf.content" required rows="12" class="mt-1 w-full rounded-xl bg-surface-2 px-3 py-2 font-mono text-xs text-text" placeholder="# Title&#10;&#10;What agents should know." />
        </label>
        <label class="flex items-center gap-2 text-sm text-text-muted"><input v-model="nf.personal" type="checkbox" /> Keep on this computer only</label>
        <p v-if="nfError" class="text-sm text-bad">{{ nfError }}</p>
        <div class="flex justify-end gap-2">
          <Button variant="ghost" type="button" @click="newOpen = false">Cancel</Button>
          <Button variant="primary" type="submit" lock="/api/library/file" :disabled="!nf.path.trim() || !nf.content.trim()">Add file</Button>
        </div>
      </form>
    </Modal>
  </div>
</template>

<script setup lang="ts">
import { ref, reactive, computed, onMounted } from 'vue';
import { request, safeCall, protectedCall } from '~/utils/api';
import { useToast } from '~/composables/useToast';

const toast = useToast();
const data = ref<any>(null);
const file = ref<any>(null);
const sel = ref('');
const tier = ref('summary');
const editing = ref(false);
const draft = ref('');
const revText = ref('');
const busy = ref('');
const syncNote = ref('');
const pattern = ref('');
const newOpen = ref(false);
const nf = reactive({ path: '', content: '', personal: false });
const nfError = ref('');

const allPatterns = computed(() => [...(data.value?.patterns.global || []), ...(data.value?.patterns.project || [])]);
const groups = computed(() => {
  const by: Record<string, any[]> = {};
  for (const f of data.value?.files || []) {
    const i = f.path.lastIndexOf('/');
    (by[i < 0 ? '' : f.path.slice(0, i)] ||= []).push(f);
  }
  return Object.keys(by).sort().map((folder) => ({ folder, files: by[folder] }));
});

async function load() {
  const r = await request<any>('/api/library');
  if (r.ok) data.value = r.data;
  if (sel.value) await open(sel.value, true);
}
onMounted(load);

async function open(path: string, keepTier = false) {
  const r = await request<any>(`/api/library/file?path=${encodeURIComponent(path)}`);
  if (!r.ok) { file.value = null; sel.value = ''; return; }
  sel.value = path;
  file.value = r.data;
  editing.value = false;
  revText.value = '';
  if (!keepTier) tier.value = 'summary';
}

async function syncNow() {
  busy.value = 'sync';
  try {
    const r = await safeCall('POST', '/api/library/sync');
    if (!r.ok) return toast.error(r.error || 'Could not sync');
    const x = r.data.report;
    syncNote.value = `Synced: ${x.written} copied into the folder, ${x.ingested} taken back, ${x.linked} kept-in-place file(s) updated${x.skipped.length ? `; skipped: ${x.skipped.join('; ')}` : ''}.`;
    await load();
  } finally {
    busy.value = '';
  }
}

async function rescan() {
  busy.value = 'rescan';
  try {
    const r = await safeCall('POST', '/api/tools/docs-rescan');
    if (!r.ok) return toast.error(r.error || 'Could not rescan');
    syncNote.value = 'Rescanning the project documents.';
    setTimeout(load, 1500);
  } finally {
    busy.value = '';
  }
}

function startEdit() { draft.value = file.value.content; editing.value = true; }

async function save() {
  const r = await protectedCall('PUT', '/api/library/file', 'write to the project library', { path: file.value.path, content: draft.value, personal: file.value.personal });
  if (r.ok) { editing.value = false; await load(); } else toast.error(r.error || 'Not saved');
}

async function remove() {
  const r = await protectedCall('DELETE', `/api/library/file?path=${encodeURIComponent(file.value.path)}`, 'remove a file from the project library');
  if (r.ok) { file.value = null; sel.value = ''; await load(); } else toast.error(r.error || 'Not removed');
}

async function showRev(rev: number) {
  const r = await request<any>(`/api/library/revision?path=${encodeURIComponent(file.value.path)}&rev=${rev}`);
  revText.value = r.ok ? r.data.content : '';
}

async function revert(rev: number) {
  const r = await protectedCall('POST', '/api/library/revert', 'restore an older version of a library file', { path: file.value.path, rev });
  if (r.ok) { toast.success(`Restored revision ${rev}`); await load(); } else toast.error(r.error || 'Not restored');
}

function openNew() { nf.path = ''; nf.content = ''; nf.personal = false; nfError.value = ''; newOpen.value = true; }

async function create() {
  const r = await protectedCall('PUT', '/api/library/file', 'write to the project library', { path: nf.path, content: nf.content, personal: nf.personal });
  if (!r.ok) { nfError.value = r.error || 'Not added'; return; }
  newOpen.value = false;
  sel.value = nf.path.trim();
  await load();
  tier.value = 'summary';
}

async function addPattern() {
  const r = await protectedCall('POST', '/api/library/link', 'change which files the library keeps in sync', { pattern: pattern.value.trim() });
  if (r.ok) { pattern.value = ''; await syncNow(); } else toast.error(r.error || 'Not added');
}

async function unlink(p: string) {
  const r = await protectedCall('DELETE', `/api/library/link?pattern=${encodeURIComponent(p)}`, 'change which files the library keeps in sync');
  if (r.ok) await load();
}
</script>
