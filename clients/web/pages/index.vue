<template>
  <div class="space-y-8">
    <!-- Greeting -->
    <div class="flex items-end justify-between gap-4">
      <div>
        <h1 class="text-2xl sm:text-[32px] sm:leading-tight font-semibold tracking-tight text-text">{{ greeting }}</h1>
        <p class="text-base text-text-muted mt-1">{{ store.activeProject ? `Project ${store.activeProject}` : 'All projects' }}</p>
      </div>
      <Button size="sm" variant="secondary" @click="store.fetchAll()">
        <template #icon>
          <svg class="w-3.5 h-3.5" :class="store.isLoading ? 'animate-spin' : ''" fill="none" viewBox="0 0 24 24" stroke="currentColor">
            <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M4 4v5h.582m15.356 2A8.001 8.001 0 004.582 9m0 0H9m11 11v-5h-.581m0 0a8.003 8.003 0 01-15.357-2m15.357 2H15" />
          </svg>
        </template>
        Refresh
      </Button>
    </div>

    <ErrorState
      v-if="!store.isConnected && store.daemonError"
      title="AutoMemory isn't running"
      :message="store.daemonError"
      command="automemory serve"
      @retry="store.fetchAll()"
    />

    <div v-if="store.daemonOutdated" class="rounded-card bg-warn-bg px-5 py-4 text-sm text-text" role="status">
      This dashboard is newer than the AutoMemory daemon that is running, so some parts will be empty.
      Restart it with <code class="font-mono">automemory serve</code>.
    </div>

    <WelcomeCard />

    <!-- At a glance: three tiles, one number each -->
    <section aria-label="At a glance" class="grid grid-cols-3 gap-3 sm:gap-4">
      <NuxtLink to="/agents" data-tour="agents-card" class="group">
        <Card hover custom-class="h-full">
          <p class="text-sm text-text-muted">Agents</p>
          <p class="mt-1 text-2xl sm:text-3xl font-semibold tracking-tight text-text">{{ workingCount }}</p>
          <p class="mt-1 text-sm text-text-muted hidden sm:block">{{ agentsLine }}</p>
        </Card>
      </NuxtLink>
      <NuxtLink to="/brain" data-tour="brain-card" class="group">
        <Card hover custom-class="h-full">
          <p class="text-sm text-text-muted">Project brain</p>
          <p class="mt-1 text-2xl sm:text-3xl font-semibold tracking-tight text-text">{{ noteCount }}</p>
          <p class="mt-1 text-sm text-text-muted hidden sm:block">{{ decisionCount }} decision{{ decisionCount === 1 ? '' : 's' }}</p>
        </Card>
      </NuxtLink>
      <NuxtLink to="/inbox" data-tour="needs-you" class="group">
        <Card hover custom-class="h-full">
          <p class="text-sm text-text-muted flex items-center gap-2">
            Needs you
            <span v-if="needsCount > 0" class="w-2 h-2 rounded-full bg-warn" aria-hidden="true" />
          </p>
          <p class="mt-1 text-2xl sm:text-3xl font-semibold tracking-tight text-text">{{ needsCount }}</p>
          <p class="mt-1 text-sm text-text-muted hidden sm:block">{{ needsCount > 0 ? 'Review suggestions and locks' : 'All clear' }}</p>
        </Card>
      </NuxtLink>
    </section>

    <!-- Long-held lock notice, only when relevant -->
    <div
      v-if="longHeldLocks.length > 0"
      class="flex items-center justify-between gap-3 rounded-card bg-warn-bg px-5 py-4"
    >
      <p class="text-sm text-text">
        <span class="font-medium">{{ longHeldLocks[0]?.agent }}</span> has held
        <span class="font-mono">{{ longHeldLocks[0]?.scope }}</span> for a while.
      </p>
      <NuxtLink to="/agents"><Button size="sm" variant="secondary">See locks</Button></NuxtLink>
    </div>

    <!-- Guided setup -->
    <div data-tour="setup-card">
      <Checklist title="Get started" :items="store.setupChecklist" />
    </div>

    <!-- Is it working? -->
    <Card v-if="valueLines.length" title="Last 7 days" data-tour="value-card">
      <ul class="grid grid-cols-1 sm:grid-cols-2 gap-x-6 gap-y-2">
        <li v-for="l in valueLines" :key="l.label" class="flex items-baseline gap-3 text-sm">
          <span class="text-xl font-semibold text-text tabular-nums w-10 text-right">{{ l.n }}</span>
          <span class="text-text-muted">{{ l.label }}</span>
        </li>
      </ul>
    </Card>

    <!-- Details -->
    <section class="grid grid-cols-1 lg:grid-cols-2 gap-4">
      <Card title="Your agents">
        <template #actions>
          <NuxtLink to="/agents" class="text-sm text-accent hover:underline">Manage</NuxtLink>
        </template>
        <ul class="divide-y divide-border/60">
          <li v-for="agent in store.connectedAgents" :key="agent.id" class="flex items-center justify-between py-3 first:pt-0 last:pb-0">
            <div class="flex items-center gap-3 min-w-0">
              <span
                class="w-2 h-2 rounded-full flex-shrink-0"
                :class="agent.status === 'editing' ? 'bg-warn animate-pulse' : agent.status === 'active' ? 'bg-ok' : agent.status === 'idle' ? 'bg-accent' : 'bg-text-muted/30'"
              />
              <div class="min-w-0">
                <div class="text-[15px] text-text">{{ agent.name }}</div>
                <div v-if="agent.locks?.length" class="text-xs font-mono text-text-muted truncate">{{ agent.locks.join(', ') }}</div>
              </div>
            </div>
            <span class="text-sm text-text-muted capitalize flex-shrink-0">{{ agent.status === 'editing' ? 'Editing' : agent.status.replace('_', ' ') }}</span>
          </li>
        </ul>
      </Card>

      <Card title="Recent activity">
        <template #actions>
          <button type="button" class="text-sm text-accent hover:underline" @click="previewDrawerOpen = true">What agents are told</button>
        </template>
        <p v-if="recentSentences.length === 0" class="text-sm text-text-muted">Nothing yet. Activity appears here once an agent connects.</p>
        <ul v-else class="divide-y divide-border/60">
          <li v-for="(item, idx) in recentSentences" :key="idx" class="flex items-baseline gap-3 py-3 first:pt-0 last:pb-0">
            <span class="text-xs text-text-muted w-12 flex-shrink-0 tabular-nums">{{ item.time }}</span>
            <span class="text-sm text-text">{{ item.text }}</span>
          </li>
        </ul>
      </Card>
    </section>

    <!-- Preview Drawer -->
    <Drawer
      :is-open="previewDrawerOpen"
      title="What the next agent is told"
      description="Every new session in this project starts from this."
      @close="previewDrawerOpen = false"
    >
      <div class="bg-surface-2 rounded-xl p-4 font-mono text-xs text-text whitespace-pre-wrap overflow-x-auto max-h-[60vh]">
        {{ store.contextPreview || 'Nothing assembled yet. Add a note in Project brain.' }}
      </div>
    </Drawer>
  </div>
</template>

<script setup lang="ts">
import { ref, computed, onMounted } from 'vue';
import { request } from '~/utils/api';
import { useAutoMemoryStore, agentLabel } from '~/stores/automemory';

/** "claude" -> "Claude Code"; unknown names are shown as they are. */
function agentName(a: any): string {
  const id = agentLabel(a);
  if (!id) return 'An agent';
  if (id.includes('claude')) return 'Claude Code';
  if (id.includes('cursor')) return 'Cursor';
  if (id.includes('codex')) return 'Codex';
  if (id.includes('agy') || id.includes('antigravity')) return 'Antigravity';
  return id.charAt(0).toUpperCase() + id.slice(1);
}

const store = useAutoMemoryStore();
const previewDrawerOpen = ref(false);

const value = ref<Record<string, number>>({});
onMounted(async () => {
  const r = await request<Record<string, number>>('/api/value?days=7');
  if (r.ok && r.data) value.value = r.data;
});
const valueLines = computed(() =>
  [
    ['collisions_prevented', 'edit collisions prevented'],
    ['risky_commands_stopped', 'risky commands stopped'],
    ['notes_recalled', 'notes given to agents'],
    ['notes_added_by_you', 'notes you added'],
    ['notes_added_by_agents', 'notes agents added'],
    ['handoffs_picked_up', 'hand-offs picked up'],
  ]
    .map(([k, label]) => ({ n: value.value[k] || 0, label }))
    .filter((l) => l.n > 0)
);

const greeting = computed(() => {
  const hour = new Date().getHours();
  if (hour < 12) return 'Good morning';
  if (hour < 18) return 'Good afternoon';
  return 'Good evening';
});

const workingCount = computed(
  () => store.connectedAgents.filter((a) => a.status === 'editing' || a.status === 'active').length
);
const linkedCount = computed(() => store.connectedAgents.filter((a) => a.status !== 'not connected').length);
// The big number already says how many are working; the line adds what it does not.
const agentsLine = computed(() => {
  if (linkedCount.value === 0) return 'None connected yet';
  return `${workingCount.value > 0 ? 'working now' : 'idle'} · ${linkedCount.value} connected`;
});
const needsCount = computed(() => store.stats.pending_proposals + longHeldLocks.value.length);

const hasNeedsYou = computed(() => {
  return store.stats.pending_proposals > 0 || longHeldLocks.value.length > 0;
});

const longHeldLocks = computed(() => {
  return store.activeLeases.filter((l: any) => {
    return (l.remaining_seconds && l.remaining_seconds < 300) || false;
  });
});

const decisionCount = computed(() => {
  return store.memories.filter((m) => m.type?.toLowerCase() === 'decision').length;
});

const noteCount = computed(() => {
  return store.memories.length;
});

const latestDecision = computed(() => {
  return store.memories.find((m) => m.type?.toLowerCase() === 'decision');
});

const recentSentences = computed(() => {
  return store.recentEvents.slice(0, 5).map((e: any) => {
    const time = e.timestamp || e.created_at ? new Date(e.timestamp || e.created_at).toLocaleTimeString([], { hour: '2-digit', minute: '2-digit' }) : 'now';
    const agent = agentName(e.agent);
    const action = e.event_type || e.tool_name || 'recorded an action';
    return {
      time,
      text: `${agent} ${action}`,
    };
  });
});
</script>
