<template>
  <div class="p-8 space-y-6 max-w-7xl mx-auto overflow-y-auto h-full">
    <!-- Header Controls & Status -->
    <div class="flex flex-col md:flex-row md:items-center justify-between gap-4 pb-4 border-b border-dark-700">
      <div>
        <div class="flex items-center gap-2 flex-wrap">
          <FileText class="w-5 h-5 text-gold-500" />
          <h1 class="text-xl font-bold font-mono text-white">Product Specs & Guidelines</h1>
          <span class="px-2 py-0.5 text-xs font-mono bg-dark-800 text-gold-400 border border-dark-700">
            {{ filteredDocs.length }} Tracked
          </span>
          <span class="px-2 py-0.5 text-xs font-mono bg-dark-900 text-cyan-400 border border-cyan-900/50 flex items-center gap-1.5">
            <span class="w-1.5 h-1.5 rounded-full bg-cyan-400 animate-pulse"></span>
            <span>OpenViking L0/L1 Tiered Context</span>
          </span>
        </div>
        <p class="text-xs font-mono text-gray-500 mt-1">
          Canonical product requirements, architectural decision records (ADRs), and business invariants compiled for coding agents.
        </p>
      </div>

      <!-- Category Filter Pills -->
      <div class="flex items-center bg-dark-900 border border-dark-700 p-1 flex-wrap gap-1">
        <button
          v-for="cat in categories"
          :key="cat.id"
          @click="activeCategory = cat.id"
          class="px-2.5 py-1 text-xs font-mono transition-colors"
          :class="[
            activeCategory === cat.id
              ? 'bg-dark-700 text-gold-400 border-b-2 border-gold-500 font-bold'
              : 'text-gray-400 hover:text-white'
          ]"
        >
          {{ cat.label }}
        </button>
      </div>
    </div>

    <!-- Search Toolbar -->
    <div class="flex flex-col sm:flex-row items-stretch sm:items-center justify-between gap-3">
      <div class="relative flex-1 max-w-md">
        <Search class="w-4 h-4 text-gray-500 absolute left-3 top-1/2 -translate-y-1/2" />
        <input
          v-model="searchQuery"
          type="text"
          placeholder="Filter specs by title, category, or path..."
          class="w-full bg-dark-900 border border-dark-700 pl-9 pr-8 py-2 text-xs font-mono text-white placeholder-gray-600 focus:outline-none focus:border-gold-500 transition-colors"
        />
        <button
          v-if="searchQuery"
          @click="searchQuery = ''"
          class="absolute right-2.5 top-1/2 -translate-y-1/2 text-gray-500 hover:text-white text-xs font-mono"
        >
          ✕
        </button>
      </div>

      <div class="flex items-center gap-2 text-xs font-mono text-gray-500">
        <span>Single-Source:</span>
        <code class="text-cyan-400 bg-dark-900 px-1.5 py-0.5 border border-dark-800">.context/ -> ~/.automemory/context</code>
      </div>
    </div>

    <!-- Empty State -->
    <div
      v-if="filteredDocs.length === 0"
      class="border border-dashed border-dark-700 p-12 text-center space-y-3 bg-dark-900/30"
    >
      <FileText class="w-8 h-8 text-gray-600 mx-auto" />
      <div class="text-sm font-mono text-gray-400 font-bold">No product specifications tracked yet</div>
      <p class="text-xs font-mono text-gray-600 max-w-md mx-auto">
        Track specifications using the CLI:
        <code class="text-gold-400 block mt-1">automemory doc track docs/prd/spec.md --category prd</code>
        or run auto-discovery:
        <code class="text-gold-400 block mt-1">automemory doc scan</code>
      </p>
    </div>

    <!-- Document Cards Grid -->
    <div v-else class="space-y-4">
      <div
        v-for="doc in filteredDocs"
        :key="doc.id"
        class="border border-dark-700 bg-dark-950 hover:border-dark-600 transition-all p-5 space-y-4"
      >
        <!-- Card Header -->
        <div class="flex flex-col sm:flex-row sm:items-center justify-between gap-2 border-b border-dark-800/80 pb-3">
          <div class="flex items-center gap-2.5 flex-wrap">
            <span
              class="px-2 py-0.5 text-[10px] font-mono font-bold tracking-wider uppercase border"
              :class="getCategoryBadgeClass(doc.category)"
            >
              {{ doc.category }}
            </span>
            <span class="text-sm font-mono font-bold text-white">{{ doc.title }}</span>
          </div>

          <div class="flex items-center gap-2 text-xs font-mono">
            <span
              v-if="doc.remote_synced_hash && doc.remote_synced_hash === doc.content_hash"
              class="flex items-center gap-1 text-emerald-400 bg-emerald-500/10 px-2 py-0.5 border border-emerald-500/20"
            >
              <CheckCircle2 class="w-3 h-3" />
              <span>Synced</span>
            </span>
            <span
              v-else
              class="flex items-center gap-1 text-amber-400 bg-amber-500/10 px-2 py-0.5 border border-amber-500/20"
            >
              <Cloud class="w-3 h-3" />
              <span>Pending Delta</span>
            </span>
            <span class="text-gray-500 font-mono text-[11px]">
              SHA: {{ doc.content_hash.slice(0, 10) }}
            </span>
          </div>
        </div>

        <!-- File Path & Context Mirror -->
        <div class="text-xs font-mono text-gray-400 flex items-center gap-2 flex-wrap">
          <span class="text-gray-600">Local Path:</span>
          <code class="text-gray-300 bg-dark-900 px-1.5 py-0.5 border border-dark-800">{{ doc.relative_path }}</code>
          <span class="text-gray-600">•</span>
          <span class="text-gray-600">Canonical:</span>
          <code class="text-cyan-400 bg-dark-900 px-1.5 py-0.5 border border-dark-800">.context/docs/{{ doc.category }}/{{ getFilename(doc.relative_path) }}</code>
        </div>

        <!-- L0 Abstract Box -->
        <div v-if="doc.l0_abstract" class="bg-dark-900/60 border-l-2 border-gold-500 p-3 text-xs font-mono space-y-1">
          <div class="text-[10px] text-gold-500 uppercase tracking-wider font-bold">L0 Ambient Abstract (~15-20 tokens)</div>
          <div class="text-gray-200 leading-relaxed">{{ doc.l0_abstract }}</div>
        </div>

        <!-- L1 Overview Details -->
        <div v-if="doc.l1_overview" class="border border-dark-800 bg-dark-900/20 p-3 space-y-2">
          <div class="text-[10px] text-cyan-400 uppercase tracking-wider font-bold">L1 Hierarchical Overview (~150 tokens)</div>
          <pre class="text-xs font-mono text-gray-300 whitespace-pre-wrap leading-relaxed overflow-x-auto">{{ doc.l1_overview }}</pre>
        </div>
      </div>
    </div>
  </div>
</template>

<script setup lang="ts">
import { ref, computed, onMounted } from 'vue';
import { useAutoMemoryStore } from '~/stores/automemory';
import { FileText, Search, Cloud, CheckCircle2 } from 'lucide-vue-next';

const store = useAutoMemoryStore();
const activeCategory = ref('all');
const searchQuery = ref('');

onMounted(async () => {
  await store.fetchDocs();
});

const categories = [
  { id: 'all', label: 'All Categories' },
  { id: 'prd', label: 'PRD' },
  { id: 'adr', label: 'ADR' },
  { id: 'business-rules', label: 'Business Rules' },
  { id: 'api', label: 'API Specs' },
  { id: 'general', label: 'General' },
];

const filteredDocs = computed(() => {
  let list = store.docs || [];
  if (activeCategory.value !== 'all') {
    list = list.filter((d: any) => d.category === activeCategory.value);
  }
  if (searchQuery.value.trim()) {
    const q = searchQuery.value.toLowerCase();
    list = list.filter((d: any) =>
      d.title.toLowerCase().includes(q) ||
      d.category.toLowerCase().includes(q) ||
      d.relative_path.toLowerCase().includes(q)
    );
  }
  return list;
});

const getCategoryBadgeClass = (category: string) => {
  switch (category?.toLowerCase()) {
    case 'prd':
      return 'bg-fuchsia-500/10 text-fuchsia-400 border-fuchsia-500/30';
    case 'adr':
      return 'bg-cyan-500/10 text-cyan-400 border-cyan-500/30';
    case 'business-rules':
      return 'bg-amber-500/10 text-amber-400 border-amber-500/30';
    case 'api':
      return 'bg-emerald-500/10 text-emerald-400 border-emerald-500/30';
    default:
      return 'bg-gray-500/10 text-gray-400 border-gray-500/30';
  }
};

const getFilename = (path: string) => {
  if (!path) return '';
  const parts = path.split('/');
  return parts[parts.length - 1] || path;
};
</script>
