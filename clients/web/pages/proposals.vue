<template>
  <div class="p-8 space-y-6 max-w-7xl mx-auto overflow-y-auto h-full">
    <!-- Header -->
    <div class="flex flex-col md:flex-row md:items-center justify-between gap-4 pb-4 border-b border-dark-700">
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
          Zero-SQL human-in-the-loop review queue for autopilot command halts, deduplication merges, and memory updates.
        </p>
      </div>

      <!-- Action Buttons: Refresh, Reject All, Approve All -->
      <div class="flex items-center gap-2 flex-wrap">
        <button
          @click="refreshAll"
          :disabled="store.isLoading"
          class="flex items-center gap-1.5 px-3 py-1.5 bg-dark-900 hover:bg-dark-800 text-gray-300 border border-dark-700 text-xs font-mono transition-colors disabled:opacity-50"
          title="Refresh proposals and statistics"
        >
          <RefreshCw class="w-3.5 h-3.5" :class="{ 'animate-spin': store.isLoading }" />
          <span>Refresh</span>
        </button>

        <button
          v-if="pendingCount > 0"
          @click="confirmAction = 'reject_all'"
          :disabled="store.isLoading"
          class="flex items-center gap-1.5 px-3 py-1.5 bg-dark-900 hover:bg-red-950 text-red-400 border border-red-900/60 text-xs font-mono transition-colors disabled:opacity-50"
        >
          <XOctagon class="w-3.5 h-3.5" />
          <span>Reject All</span>
        </button>

        <button
          v-if="pendingCount > 0"
          @click="confirmAction = 'approve_all'"
          :disabled="store.isLoading"
          class="flex items-center gap-1.5 px-3 py-1.5 bg-emerald-950 hover:bg-emerald-900 text-emerald-300 border border-emerald-800 text-xs font-mono font-bold transition-colors shadow-sm shadow-emerald-500/20 disabled:opacity-50"
        >
          <CheckCheck class="w-3.5 h-3.5" />
          <span>Approve All</span>
        </button>
      </div>
    </div>

    <!-- Bulk Confirmation Modal / Banner -->
    <div
      v-if="confirmAction"
      class="p-4 bg-dark-950 border text-xs font-mono space-y-3"
      :class="confirmAction === 'approve_all' ? 'border-emerald-600/60 bg-emerald-950/20' : 'border-red-600/60 bg-red-950/20'"
    >
      <div class="flex items-center gap-2 font-bold" :class="confirmAction === 'approve_all' ? 'text-emerald-400' : 'text-red-400'">
        <AlertTriangle class="w-4 h-4" />
        <span>
          {{ confirmAction === 'approve_all' ? 'Confirm Batch Approval' : 'Confirm Batch Rejection' }}
        </span>
      </div>
      <p class="text-gray-300 leading-relaxed">
        <template v-if="confirmAction === 'approve_all'">
          Are you sure you want to approve all <strong class="text-white">{{ pendingCount }}</strong> pending proposals?
          Any duplicate merge proposals will automatically archive redundant copies while preserving the primary memory records.
        </template>
        <template v-else>
          Are you sure you want to reject all <strong class="text-white">{{ pendingCount }}</strong> pending proposals?
          None of the suggested memory updates or archival actions will be applied.
        </template>
      </p>
      <div class="flex items-center gap-2 pt-1">
        <button
          @click="executeBatchDecision(confirmAction === 'approve_all')"
          :disabled="store.isLoading"
          class="px-4 py-1.5 font-bold uppercase tracking-wider text-[11px] transition-colors"
          :class="confirmAction === 'approve_all' ? 'bg-emerald-600 hover:bg-emerald-500 text-white' : 'bg-red-600 hover:bg-red-500 text-white'"
        >
          <span v-if="store.isLoading">Processing...</span>
          <span v-else>Yes, Proceed</span>
        </button>
        <button
          @click="confirmAction = null"
          :disabled="store.isLoading"
          class="px-3 py-1.5 bg-dark-800 hover:bg-dark-700 text-gray-300 text-[11px]"
        >
          Cancel
        </button>
      </div>
    </div>

    <!-- Summary KPI Statistics Grid -->
    <div class="grid grid-cols-2 md:grid-cols-4 gap-4 font-mono">
      <div class="p-4 bg-dark-900 border border-dark-700 space-y-1">
        <div class="flex items-center justify-between text-gray-500 text-[11px]">
          <span>PENDING REVIEW</span>
          <Clock class="w-3.5 h-3.5 text-amber-400" />
        </div>
        <div class="text-2xl font-bold text-amber-400">{{ pendingCount }}</div>
        <div class="text-[10px] text-gray-500">Awaiting user decision</div>
      </div>

      <div class="p-4 bg-dark-900 border border-dark-700 space-y-1">
        <div class="flex items-center justify-between text-gray-500 text-[11px]">
          <span>APPROVED</span>
          <CheckCircle2 class="w-3.5 h-3.5 text-emerald-400" />
        </div>
        <div class="text-2xl font-bold text-emerald-400">{{ approvedCount }}</div>
        <div class="text-[10px] text-gray-500">Executed & resolved</div>
      </div>

      <div class="p-4 bg-dark-900 border border-dark-700 space-y-1">
        <div class="flex items-center justify-between text-gray-500 text-[11px]">
          <span>REJECTED</span>
          <XOctagon class="w-3.5 h-3.5 text-red-400" />
        </div>
        <div class="text-2xl font-bold text-red-400">{{ rejectedCount }}</div>
        <div class="text-[10px] text-gray-500">Denied by operator</div>
      </div>

      <div class="p-4 bg-dark-900 border border-dark-700 space-y-1">
        <div class="flex items-center justify-between text-gray-500 text-[11px]">
          <span>TOTAL PROPOSALS</span>
          <Layers class="w-3.5 h-3.5 text-cyan-400" />
        </div>
        <div class="text-2xl font-bold text-cyan-400">{{ totalCount }}</div>
        <div class="text-[10px] text-gray-500">Lifetime queue history</div>
      </div>
    </div>

    <!-- Category Breakdown Pills (if breakdown exists) -->
    <div v-if="proposalsSummary?.by_type?.length" class="flex items-center gap-2 flex-wrap font-mono text-xs">
      <span class="text-gray-500 text-[11px]">Types:</span>
      <div
        v-for="item in proposalsSummary.by_type"
        :key="item.proposal_type"
        class="px-2.5 py-1 bg-dark-900 border border-dark-700 flex items-center gap-2 text-[11px]"
      >
        <span class="text-gray-300 font-semibold">{{ item.proposal_type }}</span>
        <span class="text-amber-400 font-bold">{{ item.pending }} pending</span>
        <span class="text-gray-600">/</span>
        <span class="text-gray-400">{{ item.total }} total</span>
      </div>
    </div>

    <!-- Filter Tabs -->
    <div class="flex items-center justify-between flex-wrap gap-2 pt-2">
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

      <div class="text-xs font-mono text-gray-500">
        Showing {{ filteredProposals.length }} of {{ store.proposals.length }} records
      </div>
    </div>

    <!-- Empty State -->
    <div v-if="filteredProposals.length === 0" class="text-center py-16 border border-dark-800 bg-dark-950 p-8 space-y-3 font-mono">
      <CheckCircle2 class="w-10 h-10 text-emerald-500 mx-auto" />
      <div class="text-sm text-gray-300 font-semibold">Queue is clear</div>
      <p class="text-xs text-gray-500 max-w-sm mx-auto">
        No pending review proposals match the current filter. Autopilot commands and memory changes requiring review will appear here.
      </p>
      <div class="pt-2">
        <button
          @click="refreshAll"
          class="px-3 py-1.5 bg-dark-900 hover:bg-dark-800 text-xs text-gray-300 border border-dark-700"
        >
          Refresh Queue
        </button>
      </div>
    </div>

    <!-- Proposals List -->
    <div v-else class="space-y-4 font-mono">
      <div
        v-for="p in filteredProposals"
        :key="p.id"
        class="p-5 bg-dark-900 border border-dark-700 hover:border-cyan-500/50 transition-all duration-150 space-y-4 shadow-lg"
      >
        <!-- Header Row -->
        <div class="flex items-center justify-between flex-wrap gap-2">
          <div class="flex items-center gap-2 flex-wrap">
            <span
              class="px-2 py-0.5 text-[10px] uppercase font-bold border"
              :class="getBadgeClass(p)"
            >
              {{ getBadgeLabel(p) }}
            </span>

            <span v-if="p.target_id" class="px-2 py-0.5 bg-dark-950 text-gray-400 border border-dark-800 text-[10px]">
              target: <code class="text-white">{{ p.target_id.substring(0, 16) }}...</code>
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
              :disabled="store.isLoading"
              class="flex items-center gap-1.5 px-3 py-1.5 bg-dark-850 hover:bg-red-950 text-red-400 border border-red-900/60 text-xs font-mono transition-colors disabled:opacity-50"
            >
              <X class="w-3.5 h-3.5" />
              <span>Reject</span>
            </button>
            <button
              @click="store.decideProposal(p.id, true)"
              :disabled="store.isLoading"
              class="flex items-center gap-1.5 px-3 py-1.5 bg-emerald-950 hover:bg-emerald-900 text-emerald-300 border border-emerald-800 text-xs font-mono font-bold transition-colors shadow-sm shadow-emerald-500/20 disabled:opacity-50"
            >
              <Check class="w-3.5 h-3.5" />
              <span>{{ getApproveButtonLabel(p) }}</span>
            </button>
          </div>
        </div>

        <!-- Description / Command Snippet -->
        <div class="space-y-1.5">
          <div class="text-[10px] text-gray-500 uppercase flex items-center justify-between">
            <span>{{ isGuardrail(p) ? 'Target Command / Intervention' : 'Proposed Content & Diff' }}</span>
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

          <!-- If duplicate merge, show styled comparison callout -->
          <div
            v-else-if="isDuplicateMerge(p)"
            class="p-3 bg-dark-950 border border-cyan-900/40 rounded font-mono text-xs text-gray-300 leading-relaxed whitespace-pre-wrap"
          >
            <div class="flex items-center gap-2 text-cyan-400 text-[10px] mb-1.5 select-none font-bold">
              <Archive class="w-3 h-3" />
              <span>DUPLICATE TRAJECTORY MERGE</span>
            </div>
            <p>{{ p.proposed_content || p.description }}</p>
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
import { ref, computed, onMounted } from 'vue';
import { useAutoMemoryStore } from '~/stores/automemory';
import { estimateTokens } from '~/utils/tokens';
import {
  GitPullRequestDraft,
  CheckCircle2,
  Check,
  CheckCheck,
  X,
  XOctagon,
  Terminal,
  RefreshCw,
  AlertTriangle,
  Layers,
  Clock,
  Archive,
} from 'lucide-vue-next';

const store = useAutoMemoryStore();
const activeFilter = ref<'all' | 'guardrails' | 'memories' | 'duplicates'>('all');
const confirmAction = ref<'approve_all' | 'reject_all' | null>(null);

onMounted(async () => {
  await refreshAll();
});

async function refreshAll() {
  await Promise.allSettled([
    store.fetchProposals(),
    store.fetchProposalsSummary(),
  ]);
}

const pendingCount = computed(() => {
  if (store.proposalsSummary?.total_pending !== undefined) {
    return store.proposalsSummary.total_pending;
  }
  return store.proposals.length;
});

const approvedCount = computed(() => store.proposalsSummary?.total_approved ?? 0);
const rejectedCount = computed(() => store.proposalsSummary?.total_rejected ?? 0);
const totalCount = computed(() => store.proposalsSummary?.total ?? store.proposals.length);
const proposalsSummary = computed(() => store.proposalsSummary);

function isGuardrail(p: any): boolean {
  const type = p.proposal_type || p.type || '';
  return type.toLowerCase().includes('guardrail') || (p.description && p.description.includes('Blocked high-impact command'));
}

function isDuplicateMerge(p: any): boolean {
  const type = p.proposal_type || p.type || '';
  return type === 'duplicate_merge';
}

function getBadgeClass(p: any): string {
  if (isGuardrail(p)) return 'bg-amber-950/80 text-amber-300 border-amber-800/80';
  if (isDuplicateMerge(p)) return 'bg-cyan-950/80 text-cyan-300 border-cyan-800/80';
  return 'bg-purple-950/80 text-purple-300 border-purple-800/80';
}

function getBadgeLabel(p: any): string {
  if (isGuardrail(p)) return 'Guardrail Override';
  if (isDuplicateMerge(p)) return 'Duplicate Merge';
  return p.proposal_type || p.type || 'Memory Update';
}

function getApproveButtonLabel(p: any): string {
  if (isGuardrail(p)) return 'Permit Execution';
  if (isDuplicateMerge(p)) return 'Merge & Archive';
  return 'Approve & Save';
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

async function executeBatchDecision(approved: boolean) {
  confirmAction.value = null;
  await store.decideAllProposals(approved, activeFilter.value === 'all' ? null : activeFilter.value);
}

const filteredProposals = computed(() => {
  if (activeFilter.value === 'guardrails') {
    return store.proposals.filter(isGuardrail);
  }
  if (activeFilter.value === 'duplicates') {
    return store.proposals.filter(isDuplicateMerge);
  }
  if (activeFilter.value === 'memories') {
    return store.proposals.filter((p) => !isGuardrail(p) && !isDuplicateMerge(p));
  }
  return store.proposals;
});

const filterTabs = computed(() => [
  { id: 'all' as const, label: 'All', count: store.proposals.length },
  { id: 'duplicates' as const, label: 'Duplicates', count: store.proposals.filter(isDuplicateMerge).length },
  { id: 'guardrails' as const, label: 'Guardrail Halts', count: store.proposals.filter(isGuardrail).length },
  { id: 'memories' as const, label: 'Memory Updates', count: store.proposals.filter((p) => !isGuardrail(p) && !isDuplicateMerge(p)).length },
]);
</script>
