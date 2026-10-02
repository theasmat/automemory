<template>
  <div class="p-8 space-y-6 max-w-7xl mx-auto overflow-y-auto h-full">
    <!-- Header -->
    <div class="flex flex-col sm:flex-row sm:items-center justify-between gap-4 pb-4 border-b border-dark-700">
      <div>
        <div class="flex items-center gap-2">
          <GitPullRequestDraft class="w-5 h-5 text-cyan-400" />
          <h1 class="text-xl font-bold font-mono text-white">Review Proposals Queue</h1>
          <span
            class="px-2 py-0.5 text-xs font-mono border"
            :class="pendingCount > 0 ? 'bg-amber-950 text-amber-400 border-amber-800' : 'bg-dark-800 text-gray-400 border-dark-700'"
          >
            {{ pendingCount }} Pending
          </span>
        </div>
        <p class="text-xs font-mono text-gray-500 mt-1">
          Human-in-the-loop review queue for autopilot command halts, conflicting memory updates, and policy exceptions.
        </p>
      </div>

      <!-- Filter Tabs -->
      <div class="flex items-center bg-dark-900 border border-dark-700 p-1">
        <button
          v-for="tab in filterTabs"
          :key="tab.id"
          @click="activeFilter = tab.id"
          class="px-3 py-1 text-xs font-mono transition-colors"
          :class="[
            activeFilter === tab.id
              ? 'bg-dark-700 text-cyan-400 border-b-2 border-cyan-400 font-bold'
              : 'text-gray-400 hover:text-white'
          ]"
        >
          {{ tab.label }}
          <span v-if="tab.count > 0" class="ml-1 text-[10px] px-1 bg-dark-800 text-gray-400 rounded">
            {{ tab.count }}
          </span>
        </button>
      </div>
    </div>

    <!-- Empty State -->
    <div v-if="filteredProposals.length === 0" class="text-center py-16 border border-dark-800 bg-dark-950 p-8 space-y-3">
      <CheckCircle2 class="w-10 h-10 text-emerald-500 mx-auto" />
      <div class="text-sm font-mono text-gray-300 font-semibold">Queue is clear</div>
      <p class="text-xs font-mono text-gray-500 max-w-sm mx-auto">
        No proposals matching this filter. Autopilot commands and agent memory changes requiring human consent will appear here.
      </p>
    </div>

    <!-- Proposals List -->
    <div v-else class="space-y-4">
      <div
        v-for="p in filteredProposals"
        :key="p.id"
        class="p-5 bg-dark-900 border border-dark-700 hover:border-cyan-500/50 transition-all duration-150 space-y-4 font-mono shadow-lg"
      >
        <!-- Header Row -->
        <div class="flex items-center justify-between flex-wrap gap-2">
          <div class="flex items-center gap-2 flex-wrap">
            <span
              class="px-2 py-0.5 text-[10px] uppercase font-bold border"
              :class="isGuardrail(p) ? 'bg-amber-950/80 text-amber-300 border-amber-800/80' : 'bg-cyan-950/80 text-cyan-300 border-cyan-800/80'"
            >
              {{ isGuardrail(p) ? 'Guardrail Override' : (p.proposal_type || p.type || 'Memory Update') }}
            </span>

            <span v-if="p.target_id" class="px-2 py-0.5 bg-dark-950 text-gray-400 border border-dark-800 text-[10px]">
              tool: <code class="text-white">{{ p.target_id }}</code>
            </span>

            <span v-if="p.project_id" class="px-2 py-0.5 bg-dark-950 text-gold-400/90 border border-dark-800 text-[10px]">
              project: {{ p.project_id }}
            </span>

            <span class="text-[11px] text-gray-500">
              ID: {{ p.id.substring(0, 18) }}...
            </span>
          </div>

          <!-- Action Buttons -->
          <div class="flex items-center gap-2">
            <button
              @click="store.decideProposal(p.id, false)"
              class="flex items-center gap-1.5 px-3 py-1.5 bg-dark-850 hover:bg-red-950 text-red-400 border border-red-900/60 text-xs font-mono transition-colors"
            >
              <X class="w-3.5 h-3.5" />
              <span>Reject</span>
            </button>
            <button
              @click="store.decideProposal(p.id, true)"
              class="flex items-center gap-1.5 px-3 py-1.5 bg-emerald-950 hover:bg-emerald-900 text-emerald-300 border border-emerald-800 text-xs font-mono font-bold transition-colors shadow-sm shadow-emerald-500/20"
            >
              <Check class="w-3.5 h-3.5" />
              <span>{{ isGuardrail(p) ? 'Permit Execution' : 'Approve & Save' }}</span>
            </button>
          </div>
        </div>

        <!-- Description / Command Snippet -->
        <div class="space-y-1.5">
          <div class="text-[10px] text-gray-500 uppercase flex items-center justify-between">
            <span>{{ isGuardrail(p) ? 'Target Command / Intervention' : 'Proposed Content' }}</span>
            <span v-if="!isGuardrail(p)" class="text-gray-500">
              ~{{ estimateTokens(p.proposed_content || p.description) }} tokens
            </span>
          </div>

          <!-- If guardrail command, show styled shell terminal box -->
          <div
            v-if="isGuardrail(p)"
            class="p-3 bg-dark-950 border border-dark-800 rounded font-mono text-xs text-amber-200/90 leading-relaxed overflow-x-auto"
          >
            <div class="flex items-center gap-2 text-gray-500 text-[10px] mb-1.5 select-none">
              <Terminal class="w-3 h-3 text-amber-400" />
              <span>BLOCKED COMMAND (Autopilot Review Mode)</span>
            </div>
            <code>{{ getCommandSnippet(p) }}</code>
          </div>

          <!-- Else show memory text -->
          <p
            v-else
            class="p-3 bg-dark-950 border border-dark-800 text-xs text-gray-200 leading-relaxed whitespace-pre-wrap font-mono"
          >
            {{ p.proposed_content || p.description }}
          </p>
        </div>

        <!-- Footer: Reason / Suggested Action / Timestamp -->
        <div class="flex items-center justify-between pt-2 border-t border-dark-800/80 text-xs text-gray-500 flex-wrap gap-2">
          <div class="flex items-center gap-2">
            <span class="text-gray-600">Action:</span>
            <span class="text-gray-400">{{ p.suggested_action || p.reason || 'Operator approval required' }}</span>
          </div>

          <div v-if="p.created_at" class="text-[10px] text-gray-600">
            Created: {{ formatTime(p.created_at) }}
          </div>
        </div>
      </div>
    </div>
  </div>
</template>

<script setup lang="ts">
import { ref, computed } from 'vue';
import { useAutoMemoryStore } from '~/stores/automemory';
import { estimateTokens } from '~/utils/tokens';
import { GitPullRequestDraft, CheckCircle2, Check, X, Terminal } from 'lucide-vue-next';

const store = useAutoMemoryStore();
const activeFilter = ref<'all' | 'guardrails' | 'memories'>('all');

const pendingCount = computed(() => store.proposals.length);

function isGuardrail(p: any): boolean {
  const type = p.proposal_type || p.type || '';
  return type.toLowerCase().includes('guardrail') || (p.description && p.description.includes('Blocked high-impact command'));
}

function getCommandSnippet(p: any): string {
  if (p.description) {
    const match = p.description.match(/command under Review mode:\s*(.+)$/i);
    if (match) return match[1];
    return p.description;
  }
  return p.proposed_content || '';
}

function formatTime(iso: string): string {
  try {
    const date = new Date(iso);
    return date.toLocaleString();
  } catch {
    return iso;
  }
}

const filteredProposals = computed(() => {
  if (activeFilter.value === 'guardrails') {
    return store.proposals.filter(isGuardrail);
  }
  if (activeFilter.value === 'memories') {
    return store.proposals.filter((p) => !isGuardrail(p));
  }
  return store.proposals;
});

const filterTabs = computed(() => [
  { id: 'all' as const, label: 'All', count: store.proposals.length },
  { id: 'guardrails' as const, label: 'Guardrail Halts', count: store.proposals.filter(isGuardrail).length },
  { id: 'memories' as const, label: 'Memory Updates', count: store.proposals.filter((p) => !isGuardrail(p)).length },
]);
</script>
