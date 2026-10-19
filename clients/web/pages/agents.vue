<template>
  <div class="space-y-6">
    <PageHeader
      data-tour="agents-header"
      title="Agents &amp; locks"
      description="Who is working, and which files are locked so agents do not collide."
    >
      <template #badge>
        <Badge variant="ok" dot>
          Live
        </Badge>
      </template>
    </PageHeader>

    <ErrorState
      v-if="!store.isConnected && store.daemonError"
      title="AutoMemory isn't running"
      :message="store.daemonError"
      command="automemory serve"
      @retry="store.fetchCoordination()"
    />

    <!-- Top Card: Who is Working -->
    <Card title="Who is working right now">
      <div v-if="activeWorkers.length === 0" class="text-sm text-text-muted py-2">
        No agents currently holding file locks or running tasks.
      </div>
      <div v-else class="space-y-3 mt-2">
        <div
          v-for="worker in activeWorkers"
          :key="worker.agent + worker.scope"
          class="flex flex-col sm:flex-row sm:items-center justify-between gap-2 p-3 rounded-xl bg-surface-2/60"
        >
          <div class="flex items-center gap-3">
            <span class="w-2.5 h-2.5 rounded-full bg-warn animate-pulse flex-shrink-0" />
            <div>
              <div class="text-sm font-semibold text-text flex items-center gap-2">
                <span>{{ worker.agent }}</span>
                <span class="text-xs text-text-muted font-normal">({{ worker.session_id }})</span>
              </div>
              <div class="text-xs font-mono text-accent mt-0.5">
                {{ worker.scope }}
              </div>
            </div>
          </div>

          <div class="flex items-center gap-3 text-xs text-text-muted self-end sm:self-auto">
            <span>{{ worker.goal || 'Editing file' }}</span>
            <span class="font-mono text-xs">&bull; {{ worker.remaining_seconds }}s left</span>
          </div>
        </div>
      </div>
    </Card>

    <!-- File Locks Table -->
    <Card title="Active file locks">
      <template #actions>
        <Badge variant="neutral">
          {{ store.activeLeases.length }} active
        </Badge>
      </template>

      <div v-if="store.activeLeases.length === 0" class="py-2">
        <EmptyState
          title="No files currently locked"
          description="When an agent edits a file or acquires an advisory lock, it appears here to prevent other agents from colliding."
        />
      </div>

      <div v-else class="overflow-x-auto mt-2">
        <table class="w-full text-left text-sm">
          <thead>
            <tr class="border-b border-border text-text-muted text-xs ">
              <th class="py-2 px-3">File / Scope</th>
              <th class="py-2 px-3">Held by</th>
              <th class="py-2 px-3">Remaining</th>
              <th class="py-2 px-3">Goal</th>
              <th class="py-2 px-3 text-right">Action</th>
            </tr>
          </thead>
          <tbody class="divide-y divide-border/60">
            <tr v-for="lease in store.activeLeases" :key="lease.scope" class="hover:bg-surface-2/40">
              <td class="py-3 px-3 font-mono text-xs text-accent font-medium">
                {{ lease.scope }}
              </td>
              <td class="py-3 px-3">
                <span class="font-medium text-text">{{ lease.agent }}</span>
              </td>
              <td class="py-3 px-3 font-mono text-xs text-text-muted">
                {{ lease.remaining_seconds }}s
              </td>
              <td class="py-3 px-3 text-text-muted max-w-xs truncate">
                {{ lease.goal || 'Automatic edit lease' }}
              </td>
              <td class="py-3 px-3 text-right">
                <Button
                  size="sm"
                  variant="danger"
                  @click="handleReleaseLock(lease)"
                >
                  Release
                </Button>
              </td>
            </tr>
          </tbody>
        </table>
      </div>
    </Card>

    <!-- Hand-offs (Batons) -->
    <Card title="Recent hand-off notes">
      <div v-if="store.coordination.batons.length === 0" class="text-xs text-text-muted py-2">
        No hand-off notes passed between agent sessions yet.
      </div>
      <div v-else class="space-y-2 mt-2">
        <div
          v-for="baton in store.coordination.batons"
          :key="baton.id"
          class="p-3 rounded-xl bg-surface-2/60 text-xs space-y-1.5"
        >
          <div class="flex items-center justify-between">
            <div class="font-semibold text-text flex items-center gap-2">
              <span>{{ baton.agent || 'Agent' }}</span>
              <span class="text-text-muted">&rarr; {{ baton.to_session || 'next session' }}</span>
            </div>
            <span class="text-text-muted font-mono text-xs">{{ baton.created_at }}</span>
          </div>
          <p class="text-text font-mono text-xs bg-surface p-2 rounded border border-border whitespace-pre-wrap">
            {{ baton.packet }}
          </p>
        </div>
      </div>
    </Card>

    <!-- Collapsible Explainer -->
    <Card>
      <div class="text-sm text-text-muted space-y-2">
        <p class="font-semibold text-text">How file locks work:</p>
        <p>
          Before modifying shared files, agents automatically acquire an advisory file lock with a time-to-live.
          If another parallel worker attempts to edit the same file, it is notified with 0ms latency to coordinate rather than overwrite.
        </p>
      </div>
    </Card>

    <!-- Emergency stop -->
    <Card title="Pause all agents">
      <div class="flex flex-wrap items-center justify-between gap-3">
        <p class="text-sm text-text-muted max-w-md">Releases every file lock at once, so work in progress stops being protected. Asks for your password.</p>
        <Button size="sm" variant="danger" @click="stopAll">Release all locks</Button>
      </div>
    </Card>

    <!-- Set up an agent -->
    <Card title="Set up an agent">
      <p class="text-sm text-text-muted mb-4">Register AutoMemory with a coding agent in this project{{ setup?.project ? ` (${setup.project})` : '' }}. Asks for your password.</p>
      <p v-if="!setup?.agents?.length" class="text-sm text-text-muted">No project yet. Run <code class="font-mono">automemory connect claude</code> once in a folder.</p>
      <ul v-else class="divide-y divide-border/60">
        <li v-for="a in setup.agents.filter((x: any) => x.native_hooks || x.registered_here)" :key="a.id" class="py-3 first:pt-0 last:pb-0 flex flex-wrap items-center justify-between gap-3">
          <div class="min-w-0">
            <p class="text-[15px] text-text">{{ agentName(a.id) }}</p>
            <p class="text-sm text-text-muted">{{ a.native_hooks ? 'Automatic hooks and memory tools' : 'Memory tools only' }} · {{ a.registered_here ? 'Connected here' : 'Not connected' }}</p>
          </div>
          <div class="flex gap-2">
            <Button v-if="!a.registered_here" size="sm" variant="primary" @click="connectAgent(a.id)">Connect</Button>
            <template v-else>
              <Button size="sm" variant="secondary" @click="previewRemove(a.id)">What would be removed?</Button>
              <Button size="sm" variant="ghost" @click="disconnectAgent(a.id)">Remove</Button>
            </template>
          </div>
        </li>
      </ul>
      <details v-if="setup?.agents?.length" class="mt-4">
        <summary class="text-sm text-accent cursor-pointer">More agents (memory tools only)</summary>
        <ul class="mt-3 divide-y divide-border/60">
          <li v-for="a in setup.agents.filter((x: any) => !x.native_hooks && !x.registered_here)" :key="a.id" class="py-3 flex items-center justify-between gap-3">
            <span class="text-[15px] text-text">{{ agentName(a.id) }}</span>
            <Button size="sm" variant="secondary" @click="connectAgent(a.id)">Connect</Button>
          </li>
        </ul>
      </details>
      <pre v-if="agentOutput" class="mt-4 text-xs bg-surface-2 rounded-xl p-3 whitespace-pre-wrap max-h-56 overflow-auto">{{ agentOutput }}</pre>
    </Card>
  </div>
</template>

<script setup lang="ts">
import { computed } from 'vue';
import { useAutoMemoryStore } from '~/stores/automemory';
import { ref, onMounted } from 'vue';
import { protectedCall, safeCall, request } from '~/utils/api';
import { useToast } from '~/composables/useToast';

const store = useAutoMemoryStore();
const { success } = useToast();

const activeWorkers = computed(() => {
  return store.activeLeases;
});

async function handleReleaseLock(lease: any) {
  const r = await protectedCall('POST', '/api/locks/release', `release the lock on ${lease.scope}`, { scope: lease.scope });
  if (r.ok) {
    success(`Released lock on ${lease.scope}`);
    await store.fetchCoordination();
  }
}

const setup = ref<any>(null);
const agentOutput = ref('');
const NAMES: Record<string, string> = { claude: 'Claude Code', agy: 'Antigravity', codex: 'Codex', cursor: 'Cursor', windsurf: 'Windsurf', 'gemini-cli': 'Gemini CLI', copilot: 'GitHub Copilot', cline: 'Cline', roo: 'Roo Code', zed: 'Zed', opencode: 'OpenCode', continue: 'Continue', goose: 'Goose', kilo: 'Kilo Code', aider: 'Aider', warp: 'Warp', devin: 'Devin', openclaw: 'OpenClaw', hermes: 'Hermes', 'claude-desktop': 'Claude Desktop' };
const agentName = (id: string) => NAMES[id] || id;

async function loadSetup() {
  const res = await request<any>('/api/agents/setup');
  if (res.ok) setup.value = res.data;
}

async function connectAgent(id: string) {
  const r = await protectedCall('POST', '/api/agents/connect', `connect ${agentName(id)}`, { agent: id });
  agentOutput.value = r.ok ? r.data.output : r.error || '';
  if (r.ok) success(`${agentName(id)} connected. Restart it so its hooks load.`);
  await loadSetup();
}

async function previewRemove(id: string) {
  const r = await safeCall('POST', '/api/agents/disconnect', { agent: id, dry_run: true });
  agentOutput.value = r.ok ? r.data.output : r.error || '';
}

async function disconnectAgent(id: string) {
  const r = await protectedCall('POST', '/api/agents/disconnect', `remove ${agentName(id)}`, { agent: id });
  agentOutput.value = r.ok ? r.data.output : r.error || '';
  if (r.ok) success(`${agentName(id)} removed from this project.`);
  await loadSetup();
}

async function stopAll() {
  const r = await protectedCall('POST', '/api/locks/stop', 'release every lock');
  if (r.ok) {
    success(`Released ${r.data.released} lock(s).`);
    await store.fetchCoordination();
  }
}

onMounted(loadSetup);
</script>
