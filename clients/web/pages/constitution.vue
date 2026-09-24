<template>
  <div class="p-8 space-y-6 max-w-7xl mx-auto overflow-y-auto h-full font-mono">
    <!-- Header -->
    <div class="flex items-center justify-between pb-4 border-b border-dark-700">
      <div>
        <div class="flex items-center gap-2">
          <ShieldAlert class="w-5 h-5 text-purple-400" />
          <h1 class="text-xl font-bold text-white">Hot Memory Constitution</h1>
          <span class="px-2 py-0.5 text-xs bg-dark-800 text-purple-400 border border-dark-700">
            CONSTITUTION.md
          </span>
        </div>
        <p class="text-xs text-gray-500 mt-1">
          Inviolable codebase invariants, safety rules, and top architectural decisions loaded at the start of every session.
        </p>
      </div>
      <button
        @click="store.fetchConstitution"
        class="px-3 py-1.5 text-xs bg-dark-800 hover:bg-dark-700 text-purple-400 border border-dark-600 transition-colors"
      >
        Refresh Constitution
      </button>
    </div>

    <!-- Constitution Viewer -->
    <div class="p-6 bg-dark-900 border border-dark-700 space-y-4">
      <div class="flex items-center justify-between border-b border-dark-800 pb-3">
        <span class="text-xs text-gray-400 uppercase font-bold">Active Constitution Payload</span>
        <span class="px-2 py-0.5 text-[10px] bg-dark-950 text-gold-400 border border-gold-500/20">
          ~{{ estimateTokens(store.constitution) }} / 500 max tokens
        </span>
      </div>

      <div v-if="!store.constitution" class="py-12 text-center text-gray-600 text-xs">
        No active constitution generated yet. Run tasks to distill key invariants.
      </div>
      <pre v-else class="text-xs text-gray-300 whitespace-pre-wrap leading-relaxed bg-dark-950 p-4 border border-dark-800">{{ store.constitution }}</pre>
    </div>
  </div>
</template>

<script setup lang="ts">
import { ShieldAlert } from 'lucide-vue-next';
import { useAutoMemoryStore } from '~/stores/automemory';
import { estimateTokens } from '~/utils/tokens';

const store = useAutoMemoryStore();

onMounted(() => {
  store.fetchConstitution();
});
</script>
