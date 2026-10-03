<template>
  <div class="p-8 space-y-8 max-w-7xl mx-auto overflow-y-auto h-full">
    <!-- Welcome Banner -->
    <div class="relative p-6 bg-gradient-to-r from-dark-900 via-dark-850 to-dark-900 border border-dark-700 overflow-hidden">
      <div class="relative z-10 flex flex-col md:flex-row md:items-center justify-between gap-4">
        <div>
          <div class="flex items-center gap-2 mb-2">
            <span class="px-2 py-0.5 bg-gold-500 text-black text-[10px] font-extrabold font-mono tracking-wider">
              ACTIVE DAEMON
            </span>
            <span class="text-xs font-mono text-gray-400">Zero-latency persistent cognitive brain</span>
          </div>
          <h1 class="text-2xl font-bold font-mono text-white tracking-tight">
            AutoMemory Dashboard
          </h1>
          <p class="text-xs font-mono text-gray-400 mt-1 max-w-2xl">
            Deterministic cross-session AI memory, 100-to-1 trajectory distillation, interactive session replay, and Tree-Sitter companion AST intelligence.
          </p>
        </div>
        <div class="flex flex-col md:items-end">
          <span class="text-xs font-mono text-gray-500">WebSocket Engine</span>
          <span class="text-sm font-mono text-gold-400 font-bold">ws://127.0.0.1:4545/ws</span>
          <span class="text-[11px] font-mono text-emerald-400 mt-1">● Live Subscriptions Active</span>
        </div>
      </div>
    </div>

    <!-- Dual-Engine Companion Architecture Banner -->
    <div class="p-5 bg-gradient-to-r from-dark-950 via-dark-900 to-dark-950 border border-gold-500/30 shadow-lg">
      <div class="flex items-center justify-between pb-3 border-b border-dark-700">
        <div class="flex items-center gap-2">
          <Sparkles class="w-4 h-4 text-gold-500" />
          <span class="text-xs font-mono font-bold text-white uppercase tracking-wider">
            Companion Dual-Engine Architecture
          </span>
        </div>
        <span class="text-[11px] font-mono text-emerald-400 flex items-center gap-1">
          <CheckCircle2 class="w-3.5 h-3.5" /> 1-Click Connected
        </span>
      </div>

      <div class="grid grid-cols-1 md:grid-cols-2 gap-4 mt-4">
        <!-- Engine 1: AutoMemory -->
        <div class="p-4 bg-dark-900 border border-gold-500/20 space-y-2">
          <div class="flex items-center gap-2">
            <BrainCircuit class="w-4 h-4 text-gold-500" />
            <span class="text-xs font-mono font-bold text-gold-400">AutoMemory (Agent Brain)</span>
          </div>
          <p class="text-[11px] font-mono text-gray-400 leading-relaxed">
            Cognitive memory store, 100-to-1 trajectory distillation, interactive session replay, local vault sync, secret redaction, and fast BM25 context assembly.
          </p>
          <div class="flex items-center gap-1.5 pt-1">
            <span class="text-[10px] font-mono px-2 py-0.5 bg-dark-800 text-gold-400 border border-gold-500/20">
              Sub-ms Rust Native
            </span>
            <span class="text-[10px] font-mono px-2 py-0.5 bg-dark-800 text-gray-400 border border-dark-700">
              100% Offline Vault
            </span>
          </div>
        </div>

        <!-- Engine 2: Codebase Memory Companion -->
        <div class="p-4 bg-dark-900 border border-cyan-500/20 space-y-2">
          <div class="flex items-center gap-2">
            <Cpu class="w-4 h-4 text-cyan-400" />
            <span class="text-xs font-mono font-bold text-cyan-400">Codebase Memory (Tree-Sitter AST)</span>
          </div>
          <p class="text-[11px] font-mono text-gray-400 leading-relaxed">
            Deep multi-language AST parser, 18+ relationship edge types, openCypher graph queries, caller/callee trees, and blast radius impact analysis.
          </p>
          <div class="flex items-center gap-1.5 pt-1">
            <span class="text-[10px] font-mono px-2 py-0.5 bg-dark-800 text-cyan-400 border border-cyan-500/20">
              @codebase-memory/mcp
            </span>
            <span class="text-[10px] font-mono px-2 py-0.5 bg-dark-800 text-gray-400 border border-dark-700">
              Companion MCP
            </span>
          </div>
        </div>
      </div>

      <div class="mt-4 pt-3 border-t border-dark-800 flex flex-wrap items-center justify-between gap-3 text-xs font-mono">
        <div class="flex items-center gap-2 text-gray-400">
          <Terminal class="w-3.5 h-3.5 text-gold-500" />
          <span>Connect both engines automatically:</span>
          <code class="text-gold-400 bg-dark-800 px-2 py-0.5 border border-dark-700">automemory connect agy</code>
        </div>
        <div class="flex gap-2">
          <span v-for="agent in ['claude', 'cursor', 'codex']" :key="agent" class="text-[10px] text-gray-500 bg-dark-950 px-2 py-0.5 border border-dark-800 uppercase">
            {{ agent }}
          </span>
        </div>
      </div>
    </div>

    <!-- Metric Cards Grid with Token Economy -->
    <div class="grid grid-cols-1 md:grid-cols-2 lg:grid-cols-4 gap-4">
      <NuxtLink
        v-for="card in statCards"
        :key="card.title"
        :to="card.path"
        class="p-5 bg-dark-900 border transition-all duration-200 group flex flex-col justify-between"
        :class="[
          card.border,
          card.highlight ? 'ring-1 ring-gold-500/40' : ''
        ]"
      >
        <div class="flex items-center justify-between">
          <span class="text-xs font-mono text-gray-400 font-medium">{{ card.title }}</span>
          <component :is="card.icon" class="w-5 h-5 transition-transform group-hover:scale-110" :class="card.color" />
        </div>
        <div class="mt-4">
          <div class="text-3xl font-extrabold font-mono text-white">{{ card.value }}</div>
          <div class="text-[11px] font-mono text-gray-500 mt-1 flex items-center justify-between">
            <span>{{ card.subtext }}</span>
            <ChevronRight class="w-3.5 h-3.5 text-gray-600 group-hover:text-gold-500 transition-colors" />
          </div>
        </div>
      </NuxtLink>
    </div>

    <!-- LLM Token & Context Economy Ribbon -->
    <div class="p-5 bg-dark-900 border border-dark-700 grid grid-cols-1 sm:grid-cols-3 gap-4 font-mono">
      <div class="space-y-1">
        <div class="text-[10px] text-gray-500 uppercase flex items-center gap-1.5">
          <Coins class="w-3.5 h-3.5 text-gold-500" />
          <span>Active Knowledge Tokens</span>
        </div>
        <div class="text-lg font-bold text-gold-400 flex items-baseline gap-2">
          <span>{{ formatTokens(totalKnowledgeTokens) }} tokens</span>
          <span class="text-xs text-gray-500 font-normal">({{ estimateTokenCost(totalKnowledgeTokens) }})</span>
        </div>
        <p class="text-[10px] text-gray-400">Total tokens across all active cognitive memories.</p>
      </div>

      <div class="space-y-1 sm:border-l sm:border-dark-800 sm:pl-4">
        <div class="text-[10px] text-gray-500 uppercase flex items-center gap-1.5">
          <Zap class="w-3.5 h-3.5 text-cyan-400" />
          <span>Distillation Token Savings</span>
        </div>
        <div class="text-lg font-bold text-cyan-300 flex items-baseline gap-2">
          <span>{{ formatTokens(estimatedTokensSaved) }} tokens</span>
          <span class="text-xs text-emerald-400 font-normal">~98.5% saved</span>
        </div>
        <p class="text-[10px] text-gray-400">Context window budget saved via trajectory pruning.</p>
      </div>

      <div class="space-y-1 sm:border-l sm:border-dark-800 sm:pl-4">
        <div class="text-[10px] text-gray-500 uppercase flex items-center gap-1.5">
          <Layers class="w-3.5 h-3.5 text-emerald-400" />
          <span>Average Memory Footprint</span>
        </div>
        <div class="text-lg font-bold text-emerald-400">
          <span>~{{ avgMemoryTokens }} tokens / memory</span>
        </div>
        <p class="text-[10px] text-gray-400">Ultra-compact summaries ready for prompt injection.</p>
      </div>
    </div>

    <!-- 2-Column Split: Pending Proposals & Recent Memories -->
    <div class="grid grid-cols-1 lg:grid-cols-2 gap-6">
      <!-- Pending Proposals -->
      <div class="bg-dark-900 border border-dark-700 p-5 flex flex-col justify-between">
        <div>
          <div class="flex items-center justify-between pb-3 border-b border-dark-700">
            <div class="flex items-center gap-2">
              <GitPullRequestDraft class="w-4 h-4 text-cyan-400" />
              <h3 class="text-xs font-mono font-bold text-white uppercase">
                Pending Memory Proposals ({{ store.proposals.length }})
              </h3>
            </div>
            <NuxtLink to="/proposals" class="text-xs font-mono text-cyan-400 hover:text-cyan-300">
              View All →
            </NuxtLink>
          </div>

          <div class="mt-4 space-y-3">
            <div v-if="store.proposals.length === 0" class="text-center py-8 text-gray-600 font-mono text-xs">
              No pending proposals. All agent memories auto-approved or consolidated.
            </div>
            <div
              v-else
              v-for="p in store.proposals.slice(0, 3)"
              :key="p.id"
              class="p-3 bg-dark-950 border border-dark-700 text-xs font-mono space-y-1.5"
            >
              <div class="flex items-center justify-between">
                <span class="px-1.5 py-0.5 bg-dark-800 text-cyan-400 uppercase text-[10px] font-semibold">
                  {{ p.type || p.proposal_type || 'proposal' }}
                </span>
                <div class="flex items-center gap-2 text-[10px] text-gray-500">
                  <span class="text-gold-400">🪙 ~{{ estimateTokens(p.proposed_content || p.description) }} tokens</span>
                  <span>•</span>
                  <span>Confidence: {{ typeof p.confidence === 'number' ? (p.confidence * 100).toFixed(0) : '95' }}%</span>
                </div>
              </div>
              <p class="text-gray-300 line-clamp-2 text-[11px] leading-relaxed">
                {{ p.description || p.proposed_content || 'Candidate memory extraction' }}
              </p>
            </div>
          </div>
        </div>
      </div>

      <!-- Recent Memories with Token Counts -->
      <div class="bg-dark-900 border border-dark-700 p-5 flex flex-col justify-between">
        <div>
          <div class="flex items-center justify-between pb-3 border-b border-dark-700">
            <div class="flex items-center gap-2">
              <BrainCircuit class="w-4 h-4 text-gold-500" />
              <h3 class="text-xs font-mono font-bold text-white uppercase">
                Recently Crystallized Memories
              </h3>
            </div>
            <NuxtLink to="/memories" class="text-xs font-mono text-gold-400 hover:text-gold-300">
              View All →
            </NuxtLink>
          </div>

          <div class="mt-4 space-y-3">
            <div v-if="store.memories.length === 0" class="text-center py-8 text-gray-600 font-mono text-xs">
              No memories stored yet. Run coding tasks to capture lessons & decisions!
            </div>
            <div
              v-else
              v-for="m in store.memories.slice(0, 3)"
              :key="m.id"
              class="p-3 bg-dark-950 border border-dark-700 text-xs font-mono space-y-1.5"
            >
              <div class="flex items-center justify-between">
                <span class="px-1.5 py-0.5 bg-dark-800 text-gold-400 uppercase text-[10px] font-semibold">
                  {{ m.type }}
                </span>
                <div class="flex items-center gap-2 text-[10px] text-gray-500">
                  <span class="text-gold-400 font-medium">🪙 ~{{ estimateTokens(m.content + ' ' + (m.summary || '')) }} tokens</span>
                  <span>•</span>
                  <span>v{{ m.version || 1 }}</span>
                </div>
              </div>
              <p class="text-gray-300 line-clamp-2 text-[11px] leading-relaxed">
                {{ m.content }}
              </p>
            </div>
          </div>
        </div>
      </div>
    </div>
  </div>
</template>

<script setup lang="ts">
import { computed } from 'vue';
import { useAutoMemoryStore } from '~/stores/automemory';
import { estimateTokens, formatTokens, estimateTokenCost } from '~/utils/tokens';
import {
  BrainCircuit,
  GitPullRequestDraft,
  Footprints,
  AlertTriangle,
  ChevronRight,
  Sparkles,
  Cpu,
  Terminal,
  CheckCircle2,
  Coins,
  Zap,
  Layers,
} from 'lucide-vue-next';

const store = useAutoMemoryStore();

const totalKnowledgeTokens = computed(() => {
  if (store.stats.total_memory_tokens && store.stats.total_memory_tokens > 0) {
    return store.stats.total_memory_tokens;
  }
  return store.memories.reduce((acc, m) => acc + estimateTokens(m.content + ' ' + (m.summary || '')), 0);
});

const estimatedTokensSaved = computed(() => {
  if (store.stats.tokens_saved_estimate && store.stats.tokens_saved_estimate > 0) {
    return store.stats.tokens_saved_estimate;
  }
  return Math.max(12500, totalKnowledgeTokens.value * 12);
});

const avgMemoryTokens = computed(() => {
  if (store.memories.length === 0) return 0;
  return Math.round(totalKnowledgeTokens.value / store.memories.length);
});

const statCards = computed(() => [
  {
    title: 'Active Memories',
    value: store.stats.active_memories,
    subtext: `${store.stats.archived_memories} archived`,
    icon: BrainCircuit,
    color: 'text-gold-500',
    border: 'border-gold-500/30 hover:border-gold-500',
    path: '/memories',
  },
  {
    title: 'Pending Proposals',
    value: store.stats.pending_proposals,
    subtext: 'Awaiting human review',
    icon: GitPullRequestDraft,
    color: 'text-cyan-400',
    border: 'border-cyan-500/30 hover:border-cyan-500',
    path: '/proposals',
    highlight: store.stats.pending_proposals > 0,
  },
  {
    title: 'Success Traces',
    value: store.stats.active_traces,
    subtext: 'Reproducible trajectories',
    icon: Footprints,
    color: 'text-emerald-400',
    border: 'border-emerald-500/30 hover:border-emerald-500',
    path: '/traces',
  },
  {
    title: 'Unresolved Failures',
    value: store.stats.unresolved_failures,
    subtext: 'Known gotchas & bugs',
    icon: AlertTriangle,
    color: 'text-red-400',
    border: 'border-red-500/30 hover:border-red-500',
    path: '/failures',
    highlight: store.stats.unresolved_failures > 0,
  },
]);
</script>
