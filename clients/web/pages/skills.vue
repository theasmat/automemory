<template>
  <div class="p-8 space-y-6 max-w-7xl mx-auto overflow-y-auto h-full">
    <!-- Header -->
    <div class="flex items-center justify-between pb-4 border-b border-dark-700">
      <div>
        <div class="flex items-center gap-2">
          <Sparkles class="w-5 h-5 text-gold-400" />
          <h1 class="text-xl font-bold font-mono text-white">Procedural Skills</h1>
          <span class="px-2 py-0.5 text-xs font-mono bg-dark-800 text-gold-400 border border-dark-700">
            {{ store.skills.length }} Playbooks
          </span>
        </div>
        <p class="text-xs font-mono text-gray-500 mt-1">
          Auto-distilled procedural playbooks synthesized from verified success traces and golden trajectories.
        </p>
      </div>
      <button
        @click="store.fetchSkills"
        class="px-3 py-1.5 text-xs font-mono bg-dark-800 hover:bg-dark-700 text-gold-400 border border-dark-600 transition-colors"
      >
        Refresh Playbooks
      </button>
    </div>

    <!-- Empty State -->
    <div v-if="store.skills.length === 0" class="text-center py-16 border border-dark-800 bg-dark-950 p-8 space-y-3">
      <Sparkles class="w-10 h-10 text-gray-700 mx-auto" />
      <div class="text-sm font-mono text-gray-400 font-semibold">No procedural skills distilled yet</div>
      <p class="text-xs font-mono text-gray-600 max-w-sm mx-auto">
        As the agent executes tasks and records success traces, AutoMemory automatically synthesizes reusable procedural skills.
      </p>
    </div>

    <!-- Skills List -->
    <div v-else class="space-y-4">
      <div
        v-for="s in store.skills"
        :key="s.id"
        class="p-5 bg-dark-900 border border-dark-700 hover:border-gold-500/50 transition-all duration-150 space-y-4 font-mono"
      >
        <div class="flex items-center justify-between flex-wrap gap-2">
          <div class="flex items-center gap-2">
            <span class="px-2 py-0.5 text-[10px] uppercase font-bold bg-gold-950 text-gold-400 border border-gold-800">
              Procedural Skill
            </span>
            <h3 class="text-sm font-bold text-white">{{ s.name }}</h3>
          </div>
          <div class="flex items-center gap-2 text-xs text-gray-500">
            <span class="px-1.5 py-0.5 bg-dark-950 text-emerald-400 border border-emerald-500/20 text-[10px]">
              {{ s.success_count }} Success Run(s)
            </span>
            <span class="px-1.5 py-0.5 bg-dark-950 text-gold-400 border border-gold-500/20 text-[10px]">
              ~{{ estimateTokens(s.name + ' ' + s.description + ' ' + JSON.stringify(s.steps)) }} tokens
            </span>
          </div>
        </div>

        <p class="text-xs text-gray-400">{{ s.description }}</p>

        <!-- Trigger Patterns -->
        <div v-if="s.trigger_patterns && s.trigger_patterns.length > 0" class="flex items-center gap-2 flex-wrap">
          <span class="text-[10px] text-gray-500 uppercase font-bold">Triggers:</span>
          <span
            v-for="pat in s.trigger_patterns"
            :key="pat"
            class="px-2 py-0.5 text-[10px] bg-dark-950 text-gray-300 border border-dark-800"
          >
            {{ pat }}
          </span>
        </div>

        <!-- Ordered Steps -->
        <div class="space-y-2">
          <div class="text-[10px] uppercase text-gray-500 font-bold">Execution Steps:</div>
          <div
            v-for="(step, idx) in s.steps"
            :key="idx"
            class="p-2.5 bg-dark-950 border border-dark-800 text-xs text-emerald-400 font-mono"
          >
            {{ step }}
          </div>
        </div>
      </div>
    </div>
  </div>
</template>

<script setup lang="ts">
import { Sparkles } from 'lucide-vue-next';
import { useAutoMemoryStore } from '~/stores/automemory';
import { estimateTokens } from '~/utils/tokens';

const store = useAutoMemoryStore();

onMounted(() => {
  store.fetchSkills();
});
</script>
