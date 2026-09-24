<template>
  <div class="p-8 space-y-6 max-w-7xl mx-auto overflow-y-auto h-full">
    <!-- Header -->
    <div class="flex items-center justify-between pb-4 border-b border-dark-700">
      <div>
        <div class="flex items-center gap-2">
          <Footprints class="w-5 h-5 text-emerald-400" />
          <h1 class="text-xl font-bold font-mono text-white">Success Traces</h1>
          <span class="px-2 py-0.5 text-xs font-mono bg-dark-800 text-emerald-400 border border-dark-700">
            {{ store.traces.length }} Recorded
          </span>
        </div>
        <p class="text-xs font-mono text-gray-500 mt-1">
          Repeatable golden execution paths, step-by-step verified procedures, and regression baselines.
        </p>
      </div>
    </div>

    <!-- Empty State -->
    <div v-if="store.traces.length === 0" class="text-center py-16 border border-dark-800 bg-dark-950 p-8 space-y-3">
      <Footprints class="w-10 h-10 text-gray-700 mx-auto" />
      <div class="text-sm font-mono text-gray-400 font-semibold">No success traces stored yet</div>
      <p class="text-xs font-mono text-gray-600 max-w-sm mx-auto">
        When an agent successfully completes a complex bugfix or setup, call <code>trace.save</code> to record the golden path.
      </p>
    </div>

    <!-- Traces List -->
    <div v-else class="space-y-4">
      <div
        v-for="t in store.traces"
        :key="t.id"
        class="p-5 bg-dark-900 border border-dark-700 hover:border-emerald-500/50 transition-all duration-150 space-y-4 font-mono"
      >
        <div class="flex items-center justify-between flex-wrap gap-2">
          <div class="flex items-center gap-2">
            <span class="px-2 py-0.5 text-[10px] uppercase font-bold bg-emerald-950 text-emerald-400 border border-emerald-800">
              Golden Trajectory
            </span>
            <h3 class="text-sm font-bold text-white">{{ t.task_intent }}</h3>
          </div>
          <div class="flex items-center gap-2 text-xs text-gray-500">
            <!-- Trace Token Footprint -->
            <span class="px-1.5 py-0.5 bg-dark-950 text-gold-400 border border-gold-500/20 text-[10px] flex items-center gap-1">
              <Coins class="w-2.5 h-2.5 text-gold-500" />
              <span>~{{ estimateTokens(t.task_intent + ' ' + JSON.stringify(t.golden_steps)) }} tokens</span>
            </span>
            <span>{{ formatTime(t.created_at) }}</span>
          </div>
        </div>

        <!-- Golden Steps List -->
        <div class="space-y-2">
          <div
            v-for="(step, idx) in t.golden_steps"
            :key="idx"
            class="p-3 bg-dark-950 border border-dark-800 text-xs space-y-1"
          >
            <div class="flex items-center justify-between text-gold-400 font-bold">
              <span>Step {{ idx + 1 }}: {{ step.action || 'Command Action' }}</span>
              <span class="text-[10px] text-gray-500 font-normal">~{{ estimateTokens(step.action + ' ' + (step.command || '') + ' ' + (step.diff || '')) }} tokens</span>
            </div>
            <div v-if="step.command" class="bg-dark-900 p-2 border border-dark-800 text-cyan-300">
              $ {{ step.command }}
            </div>
            <div v-if="step.diff" class="bg-dark-900 p-2 border border-dark-800 text-gray-300 whitespace-pre-wrap font-mono">
              {{ step.diff }}
            </div>
          </div>
        </div>

        <!-- Verification Command & Tags -->
        <div class="pt-3 border-t border-dark-800 flex items-center justify-between flex-wrap gap-2 text-xs">
          <div v-if="t.verification_command" class="flex items-center gap-1.5 text-gray-400">
            <span class="text-gray-500">Verification:</span>
            <code class="text-emerald-400 bg-dark-950 px-2 py-0.5 border border-dark-800">{{ t.verification_command }}</code>
          </div>

          <div class="flex items-center gap-1 flex-wrap">
            <span
              v-for="tag in t.tags"
              :key="tag"
              class="px-1.5 py-0.5 bg-dark-850 text-gray-400 text-[10px] border border-dark-700"
            >
              #{{ tag }}
            </span>
          </div>
        </div>
      </div>
    </div>
  </div>
</template>

<script setup lang="ts">
import { useAutoMemoryStore } from '~/stores/automemory';
import { estimateTokens } from '~/utils/tokens';
import { Footprints, Coins } from 'lucide-vue-next';

const store = useAutoMemoryStore();

const formatTime = (iso: string) => {
  try {
    const d = new Date(iso);
    return d.toLocaleDateString(undefined, { month: 'short', day: 'numeric', hour: '2-digit', minute: '2-digit' });
  } catch {
    return iso;
  }
};
</script>
