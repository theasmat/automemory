<template>
  <div class="p-8 space-y-6 max-w-7xl mx-auto overflow-y-auto h-full">
    <!-- Header -->
    <div class="flex items-center justify-between pb-4 border-b border-dark-700">
      <div>
        <div class="flex items-center gap-2">
          <GitPullRequestDraft class="w-5 h-5 text-cyan-400" />
          <h1 class="text-xl font-bold font-mono text-white">Review Proposals</h1>
          <span
            class="px-2 py-0.5 text-xs font-mono border"
            :class="store.stats.pending_proposals > 0 ? 'bg-amber-950 text-amber-400 border-amber-800' : 'bg-dark-800 text-gray-400 border-dark-700'"
          >
            {{ store.stats.pending_proposals }} Pending Review
          </span>
        </div>
        <p class="text-xs font-mono text-gray-500 mt-1">
          Human-in-the-loop review queue for conflicting memories, deprecations, and high-impact policy updates.
        </p>
      </div>
    </div>

    <!-- Empty State -->
    <div v-if="store.proposals.length === 0" class="text-center py-16 border border-dark-800 bg-dark-950 p-8 space-y-3">
      <CheckCircle2 class="w-10 h-10 text-emerald-600 mx-auto" />
      <div class="text-sm font-mono text-gray-400 font-semibold">No pending proposals</div>
      <p class="text-xs font-mono text-gray-600 max-w-sm mx-auto">
        All agent memories have been auto-approved or consolidated. Proposals requiring human consent will appear here.
      </p>
    </div>

    <!-- Proposals List -->
    <div v-else class="space-y-4">
      <div
        v-for="p in store.proposals"
        :key="p.id"
        class="p-5 bg-dark-900 border border-dark-700 hover:border-cyan-500/50 transition-all duration-150 space-y-4 font-mono"
      >
        <div class="flex items-center justify-between flex-wrap gap-2">
          <div class="flex items-center gap-2">
            <span class="px-2 py-0.5 text-[10px] uppercase font-bold bg-dark-800 text-cyan-400 border border-dark-700">
              {{ p.type || p.proposal_type || 'Proposal' }}
            </span>
            <span class="px-1.5 py-0.5 bg-dark-950 text-gold-400 border border-gold-500/20 text-[10px] flex items-center gap-1">
              <Coins class="w-2.5 h-2.5 text-gold-500" />
              <span>~{{ estimateTokens(p.proposed_content || p.description) }} tokens</span>
            </span>
            <span class="text-xs text-gray-500">
              Confidence: {{ typeof p.confidence === 'number' ? (p.confidence * 100).toFixed(0) : '95' }}%
            </span>
          </div>

          <!-- Action Buttons -->
          <div class="flex items-center gap-2">
            <button
              @click="store.decideProposal(p.id, false)"
              class="flex items-center gap-1 px-3 py-1 bg-dark-850 hover:bg-red-950/60 text-red-400 border border-red-900/60 text-xs font-mono transition-colors"
            >
              <X class="w-3.5 h-3.5" />
              <span>Reject</span>
            </button>
            <button
              @click="store.decideProposal(p.id, true)"
              class="flex items-center gap-1 px-3 py-1 bg-emerald-950 hover:bg-emerald-900 text-emerald-300 border border-emerald-800 text-xs font-mono font-bold transition-colors shadow-sm shadow-emerald-500/20"
            >
              <Check class="w-3.5 h-3.5" />
              <span>Approve & Save</span>
            </button>
          </div>
        </div>

        <!-- Proposed Content -->
        <div class="space-y-1.5">
          <div class="text-[10px] text-gray-500 uppercase flex items-center justify-between">
            <span>Proposed Memory Content</span>
            <span class="text-gray-500">~{{ estimateTokens(p.proposed_content || p.description) }} tokens</span>
          </div>
          <p class="p-3 bg-dark-950 border border-dark-800 text-xs text-gray-200 leading-relaxed whitespace-pre-wrap">
            {{ p.proposed_content || p.description }}
          </p>
        </div>

        <!-- Reason -->
        <div v-if="p.reason" class="text-xs text-gray-400 pt-2 border-t border-dark-800 flex items-start gap-1.5">
          <span class="text-gray-600">Reason:</span>
          <span>{{ p.reason }}</span>
        </div>
      </div>
    </div>
  </div>
</template>

<script setup lang="ts">
import { useAutoMemoryStore } from '~/stores/automemory';
import { estimateTokens } from '~/utils/tokens';
import { GitPullRequestDraft, CheckCircle2, Check, X, Coins } from 'lucide-vue-next';

const store = useAutoMemoryStore();
</script>
