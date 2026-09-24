<template>
  <div class="p-8 space-y-6 max-w-7xl mx-auto overflow-y-auto h-full">
    <!-- Header -->
    <div class="flex flex-col sm:flex-row sm:items-center justify-between gap-4 pb-4 border-b border-dark-700">
      <div>
        <div class="flex items-center gap-2">
          <AlertTriangle class="w-5 h-5 text-red-400" />
          <h1 class="text-xl font-bold font-mono text-white">Failures & Gotchas</h1>
          <span
            class="px-2 py-0.5 text-xs font-mono border"
            :class="store.stats.unresolved_failures > 0 ? 'bg-red-950 text-red-400 border-red-800' : 'bg-dark-800 text-gray-400 border-dark-700'"
          >
            {{ store.stats.unresolved_failures }} Unresolved
          </span>
        </div>
        <p class="text-xs font-mono text-gray-500 mt-1">
          Catalog of known runtime gotchas, segfaults, compiler traps, and verified fixes.
        </p>
      </div>

      <!-- Filter Tabs -->
      <div class="flex items-center bg-dark-900 border border-dark-700 p-1">
        <button
          v-for="s in ['unresolved', 'resolved']"
          :key="s"
          @click="activeStatus = s"
          class="px-3 py-1.5 text-xs font-mono capitalize transition-colors"
          :class="[
            activeStatus === s
              ? (s === 'unresolved' ? 'bg-red-950 text-red-400 border-b-2 border-red-500 font-bold' : 'bg-emerald-950 text-emerald-400 border-b-2 border-emerald-500 font-bold')
              : 'text-gray-400 hover:text-white'
          ]"
        >
          {{ s }}
        </button>
      </div>
    </div>

    <!-- Empty State -->
    <div v-if="filteredFailures.length === 0" class="text-center py-16 border border-dark-800 bg-dark-950 p-8 space-y-3">
      <CheckCircle2 class="w-10 h-10 text-emerald-600 mx-auto" />
      <div class="text-sm font-mono text-gray-400 font-semibold">No {{ activeStatus }} gotchas found</div>
      <p class="text-xs font-mono text-gray-600 max-w-sm mx-auto">
        When an agent encounters a broken tool or unexpected compilation error, call <code>failure.save</code> to track the gotcha.
      </p>
    </div>

    <!-- Failures List -->
    <div v-else class="space-y-4">
      <div
        v-for="f in filteredFailures"
        :key="f.id"
        class="p-5 bg-dark-900 border transition-all duration-150 space-y-4 font-mono"
        :class="f.status === 'unresolved' ? 'border-red-900/60 bg-gradient-to-r from-red-950/20 via-dark-900 to-dark-900' : 'border-dark-700'"
      >
        <div class="flex items-center justify-between flex-wrap gap-2">
          <div class="flex items-center gap-2">
            <span
              class="px-2 py-0.5 text-[10px] uppercase font-bold"
              :class="f.status === 'unresolved' ? 'bg-red-950 text-red-400 border border-red-800' : 'bg-emerald-950 text-emerald-400 border border-emerald-800'"
            >
              {{ f.status }}
            </span>
            <h3 class="text-sm font-bold text-white">{{ f.task_title }}</h3>
          </div>
          <div class="flex items-center gap-2 text-xs text-gray-500">
            <!-- Token badge -->
            <span class="px-1.5 py-0.5 bg-dark-950 text-gold-400 border border-gold-500/20 text-[10px] flex items-center gap-1">
              <Coins class="w-2.5 h-2.5 text-gold-500" />
              <span>~{{ estimateTokens(f.error_message + ' ' + (f.root_cause_hypothesis || '') + ' ' + (f.fix_applied || '')) }} tokens</span>
            </span>
            <span>{{ formatTime(f.created_at) }}</span>
          </div>
        </div>

        <!-- Error Message -->
        <div class="space-y-1">
          <div class="text-[10px] text-gray-500 uppercase flex items-center justify-between">
            <span>Error Message / Stack</span>
            <span class="text-red-400">~{{ estimateTokens(f.error_message) }} tokens</span>
          </div>
          <pre class="p-3 bg-dark-950 border border-red-950 text-xs text-red-300 overflow-x-auto whitespace-pre-wrap">{{ f.error_message }}</pre>
        </div>

        <!-- Root Cause Hypothesis -->
        <div v-if="f.root_cause_hypothesis" class="space-y-1">
          <div class="text-[10px] text-gray-500 uppercase flex items-center justify-between">
            <span>Root-Cause Hypothesis</span>
            <span class="text-gray-500">~{{ estimateTokens(f.root_cause_hypothesis) }} tokens</span>
          </div>
          <p class="p-3 bg-dark-950 border border-dark-800 text-xs text-gray-300 leading-relaxed">
            {{ f.root_cause_hypothesis }}
          </p>
        </div>

        <!-- Fix Applied (if resolved) -->
        <div v-if="f.fix_applied" class="space-y-1">
          <div class="text-[10px] text-emerald-500 uppercase font-bold flex items-center justify-between">
            <span>Fix Applied & Verification</span>
            <span class="text-emerald-400 font-normal">~{{ estimateTokens(f.fix_applied) }} tokens</span>
          </div>
          <div class="p-3 bg-dark-950 border border-emerald-900/50 text-xs space-y-1.5">
            <div class="text-gray-200">{{ f.fix_applied }}</div>
            <div v-if="f.verification_result" class="text-emerald-400 text-[11px] pt-1 border-t border-dark-800">
              ✓ {{ f.verification_result }}
            </div>
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
import { AlertTriangle, CheckCircle2, Coins } from 'lucide-vue-next';

const store = useAutoMemoryStore();
const activeStatus = ref('unresolved');

const filteredFailures = computed(() => {
  return store.failures.filter((f) => f.status === activeStatus.value);
});

const formatTime = (iso: string) => {
  try {
    const d = new Date(iso);
    return d.toLocaleDateString(undefined, { month: 'short', day: 'numeric', hour: '2-digit', minute: '2-digit' });
  } catch {
    return iso;
  }
};
</script>
