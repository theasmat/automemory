<template>
  <div class="space-y-6">
    <!-- Greeting & High-level Status -->
    <div class="flex flex-col sm:flex-row sm:items-center justify-between gap-2">
      <div>
        <h1 class="text-xl sm:text-2xl font-bold tracking-tight text-text">
          {{ greeting }}. {{ activeSummary }}
        </h1>
        <p class="text-xs sm:text-sm text-text-muted mt-0.5">
          Everything your agents know, share, and need approval for.
        </p>
      </div>

      <div class="flex items-center gap-2">
        <Button size="sm" variant="secondary" @click="store.fetchAll()">
          <template #icon>
            <svg class="w-3.5 h-3.5" :class="store.isLoading ? 'animate-spin' : ''" fill="none" viewBox="0 0 24 24" stroke="currentColor">
              <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M4 4v5h.582m15.356 2A8.001 8.001 0 004.582 9m0 0H9m11 11v-5h-.581m0 0a8.003 8.003 0 01-15.357-2m15.357 2H15" />
            </svg>
          </template>
          Refresh
        </Button>
      </div>
    </div>

    <!-- Daemon Down Error State -->
    <ErrorState
      v-if="!store.isConnected && store.daemonError"
      title="AutoMemory isn't running"
      :message="store.daemonError"
      command="automemory serve"
      @retry="store.fetchAll()"
    />

    <!-- 2-Column Responsive Dashboard Grid -->
    <div class="grid grid-cols-1 lg:grid-cols-2 gap-6">
      <!-- Left Column: Needs you + Setup checklist -->
      <div class="space-y-6">
        <!-- Needs You Card (Attention state) -->
        <Card
          v-if="hasNeedsYou"
          title="Needs you"
          accent-border
          custom-class="border-warn/40 bg-warn/5"
        >
          <template #icon>
            <span class="w-2 h-2 rounded-full bg-warn" />
          </template>

          <div class="space-y-3 mt-1">
            <div
              v-if="store.stats.pending_proposals > 0"
              class="flex items-center justify-between gap-3 p-3 rounded-card bg-surface border border-border"
            >
              <div>
                <p class="text-sm font-semibold text-text">
                  {{ store.stats.pending_proposals }} suggestion{{ store.stats.pending_proposals === 1 ? '' : 's' }} to review
                </p>
                <p class="text-xs text-text-muted mt-0.5">
                  Agents learned new project facts or conventions awaiting verification.
                </p>
              </div>
              <NuxtLink to="/inbox">
                <Button size="sm" variant="primary">
                  Review now &rarr;
                </Button>
              </NuxtLink>
            </div>

            <div
              v-if="longHeldLocks.length > 0"
              class="flex items-center justify-between gap-3 p-3 rounded-card bg-surface border border-border"
            >
              <div>
                <p class="text-sm font-semibold text-text">
                  {{ longHeldLocks.length }} file lock{{ longHeldLocks.length === 1 ? '' : 's' }} held over 30 min
                </p>
                <p class="text-xs text-text-muted mt-0.5">
                  {{ longHeldLocks[0]?.agent }} holds {{ longHeldLocks[0]?.scope }}.
                </p>
              </div>
              <NuxtLink to="/agents">
                <Button size="sm" variant="secondary">
                  See locks &rarr;
                </Button>
              </NuxtLink>
            </div>
          </div>
        </Card>

        <!-- All Clear Pill if nothing needs human -->
        <Card v-else title="Needs you">
          <div class="flex items-center gap-2.5 text-xs sm:text-sm text-text-muted py-1">
            <span class="w-2 h-2 rounded-full bg-ok" />
            <span>All clear. No pending suggestions or blocked locks.</span>
          </div>
        </Card>

        <!-- Setup Checklist -->
        <Checklist
          title="Setup checklist"
          :items="store.setupChecklist"
        />
      </div>

      <!-- Right Column: Agents + What agents know + Activity stream -->
      <div class="space-y-6">
        <!-- Agents Card -->
        <Card title="Connected agents">
          <template #actions>
            <NuxtLink to="/agents" class="text-xs text-accent hover:underline">
              Manage locks &rarr;
            </NuxtLink>
          </template>

          <div class="space-y-2 mt-2">
            <div
              v-for="agent in store.connectedAgents"
              :key="agent.id"
              class="flex items-center justify-between p-2.5 rounded-card bg-surface-2/40 border border-border"
            >
              <div class="flex items-center gap-2.5">
                <span
                  class="w-2 h-2 rounded-full"
                  :class="[
                    agent.status === 'editing' ? 'bg-warn animate-pulse' :
                    agent.status === 'active' ? 'bg-ok' :
                    agent.status === 'idle' ? 'bg-accent' :
                    'bg-text-muted/30'
                  ]"
                />
                <div>
                  <div class="text-xs sm:text-sm font-semibold text-text flex items-center gap-2">
                    <span>{{ agent.name }}</span>
                    <Badge v-if="agent.status === 'editing'" variant="warn">
                      Editing
                    </Badge>
                  </div>
                  <div v-if="agent.locks?.length" class="text-[11px] font-mono text-text-muted truncate max-w-xs">
                    {{ agent.locks.join(', ') }}
                  </div>
                </div>
              </div>

              <span class="text-xs text-text-muted capitalize">
                {{ agent.status }}
              </span>
            </div>
          </div>
        </Card>

        <!-- What Agents Know -->
        <Card title="What agents know">
          <template #actions>
            <NuxtLink to="/brain">
              <Button size="sm" variant="secondary">
                Open Project brain &rarr;
              </Button>
            </NuxtLink>
          </template>

          <div class="space-y-3 mt-1">
            <div class="flex items-center gap-4 text-xs sm:text-sm text-text">
              <span class="font-semibold">{{ decisionCount }} decisions</span>
              <span class="text-text-muted">&bull;</span>
              <span class="font-semibold">{{ noteCount }} notes</span>
              <span class="text-text-muted">&bull;</span>
              <span class="text-text-muted">{{ store.stats.active_traces }} verified traces</span>
            </div>

            <div v-if="latestDecision" class="p-3 rounded-card bg-surface-2 border border-border">
              <span class="text-[11px] font-semibold text-accent uppercase tracking-wider block mb-1">
                Latest Decision
              </span>
              <p class="text-xs sm:text-sm font-medium text-text">
                &ldquo;{{ latestDecision.summary || latestDecision.content }}&rdquo;
              </p>
            </div>

            <button
              type="button"
              class="text-xs text-accent hover:underline flex items-center gap-1 font-medium"
              @click="previewDrawerOpen = true"
            >
              <span>Preview what the next agent is told</span>
              <span>&rarr;</span>
            </button>
          </div>
        </Card>

        <!-- Recent Activity Feed -->
        <Card title="Recent activity">
          <div v-if="recentSentences.length === 0" class="text-xs text-text-muted py-2">
            No activity events recorded yet. Connect an agent to begin.
          </div>
          <div v-else class="space-y-2 mt-1">
            <div
              v-for="(item, idx) in recentSentences"
              :key="idx"
              class="flex items-start gap-2.5 text-xs text-text py-1 border-b border-border/40 last:border-none"
            >
              <span class="text-text-muted font-mono text-[11px] flex-shrink-0 mt-0.5">
                {{ item.time }}
              </span>
              <span class="leading-relaxed">{{ item.text }}</span>
            </div>
          </div>
        </Card>
      </div>
    </div>

    <!-- Preview Drawer -->
    <Drawer
      :is-open="previewDrawerOpen"
      title="What the Next Agent is Told"
      description="The prompt payload injected into new agent sessions"
      @close="previewDrawerOpen = false"
    >
      <div class="space-y-4">
        <p class="text-xs text-text-muted">
          Every coding agent starting a session in this repository starts from this verified context block.
        </p>

        <div class="bg-surface-2 border border-border rounded-card p-4 font-mono text-xs text-text whitespace-pre-wrap overflow-x-auto max-h-[60vh]">
          {{ store.contextPreview || '_No context assembled yet. Add notes to Project brain._' }}
        </div>
      </div>
    </Drawer>
  </div>
</template>

<script setup lang="ts">
import { ref, computed } from 'vue';
import { useAutoMemoryStore } from '~/stores/automemory';

const store = useAutoMemoryStore();
const previewDrawerOpen = ref(false);

const greeting = computed(() => {
  const hour = new Date().getHours();
  if (hour < 12) return 'Good morning';
  if (hour < 18) return 'Good afternoon';
  return 'Good evening';
});

const activeSummary = computed(() => {
  const activeCount = store.connectedAgents.filter((a) => a.status === 'editing' || a.status === 'active').length;
  const project = store.activeProject || 'your project';
  if (activeCount > 0) {
    return `${activeCount} agent${activeCount === 1 ? ' is' : 's are'} working on ${project}`;
  }
  return `0 agents are currently working on ${project}`;
});

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
    const agent = e.agent || 'Agent';
    const action = e.event_type || e.tool_name || 'recorded an action';
    return {
      time,
      text: `${agent} ${action}`,
    };
  });
});
</script>
