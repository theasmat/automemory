<template>
  <div class="space-y-6">
    <PageHeader
      title="Documents"
      description="Project documents your agents read."
    >
      <template #badge>
        <Badge variant="neutral">{{ filteredDocs.length }} documents</Badge>
      </template>
      <template #actions>
        <Button variant="secondary" size="sm" @click="store.fetchDocs">
          Refresh
        </Button>
      </template>
    </PageHeader>

    <!-- Controls -->
    <div class="flex flex-col sm:flex-row items-stretch sm:items-center justify-between gap-3">
      <!-- Category Tabs -->
      <div class="flex items-center gap-1.5 flex-wrap">
        <button
          v-for="cat in categories"
          :key="cat.id"
          type="button"
          class="px-2.5 py-1 rounded-card text-xs font-medium transition-colors"
          :class="activeCategory === cat.id ? 'bg-surface-2 text-text font-semibold border border-border shadow-sm' : 'text-text-muted hover:text-text'"
          @click="activeCategory = cat.id"
        >
          {{ cat.label }}
        </button>
      </div>

      <!-- Search -->
      <div class="w-full sm:w-64">
        <SearchBox
          v-model="searchQuery"
          placeholder="Filter documents..."
        />
      </div>
    </div>

    <!-- Empty State -->
    <div v-if="filteredDocs.length === 0" class="py-8">
      <Card>
        <EmptyState
          title="No documents tracked yet"
          description="Track specifications and architectural decision records using the CLI:"
        >
          <template #extra>
            <div class="mt-4 max-w-md mx-auto text-left">
              <CopyCommand command="automemory doc track docs/prd/spec.md --category prd" />
            </div>
          </template>
        </EmptyState>
      </Card>
    </div>

    <!-- Document Cards -->
    <div v-else class="space-y-4">
      <Card
        v-for="doc in filteredDocs"
        :key="doc.id"
      >
        <div class="space-y-3">
          <!-- Card Header -->
          <div class="flex flex-col sm:flex-row sm:items-center justify-between gap-2 border-b border-border pb-3">
            <div class="flex items-center gap-2 flex-wrap">
              <Badge variant="neutral">{{ doc.category }}</Badge>
              <h3 class="text-sm font-semibold text-text">{{ doc.title }}</h3>
            </div>

            <div class="flex items-center gap-2 text-xs">
              <Badge
                v-if="doc.remote_synced_hash && doc.remote_synced_hash === doc.content_hash"
                variant="ok"
              >
                Synced
              </Badge>
              <Badge v-else variant="warn">
                Pending delta
              </Badge>
              <span class="text-text-muted font-mono text-xs">
                SHA: {{ doc.content_hash.slice(0, 10) }}
              </span>
            </div>
          </div>

          <!-- File Paths -->
          <div class="text-xs text-text-muted flex items-center gap-2 flex-wrap">
            <span>Path:</span>
            <code class="text-text bg-surface-2 px-1.5 py-0.5 rounded border border-border font-mono">{{ doc.relative_path }}</code>
            <span>•</span>
            <span>Canonical:</span>
            <code class="text-accent bg-surface-2 px-1.5 py-0.5 rounded border border-border font-mono">.context/docs/{{ doc.category }}/{{ getFilename(doc.relative_path) }}</code>
          </div>

          <!-- L0 Abstract Box -->
          <div v-if="doc.l0_abstract" class="bg-surface-2 rounded-card border-l-2 border-accent p-3 text-xs space-y-1">
            <div class="text-xs font-semibold text-accent ">Ambient Abstract (~15-20 tokens)</div>
            <div class="text-text leading-relaxed">{{ doc.l0_abstract }}</div>
          </div>

          <!-- L1 Overview Details -->
          <div v-if="doc.l1_overview" class="bg-surface-2 rounded-card border border-border p-3 space-y-1.5">
            <div class="text-xs font-semibold text-text ">Hierarchical Overview (~150 tokens)</div>
            <pre class="text-xs font-mono text-text-muted whitespace-pre-wrap leading-relaxed overflow-x-auto">{{ doc.l1_overview }}</pre>
          </div>
        </div>
      </Card>
    </div>
  </div>
</template>

<script setup lang="ts">
import { ref, computed, onMounted } from 'vue';
import { useAutoMemoryStore } from '~/stores/automemory';

const store = useAutoMemoryStore();
const activeCategory = ref('all');
const searchQuery = ref('');

onMounted(async () => {
  await store.fetchDocs();
});

const categories = [
  { id: 'all', label: 'All' },
  { id: 'prd', label: 'PRD' },
  { id: 'adr', label: 'ADR' },
  { id: 'business-rules', label: 'Business rules' },
  { id: 'api', label: 'API specs' },
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

const getFilename = (path: string) => {
  if (!path) return '';
  const parts = path.split('/');
  return parts[parts.length - 1] || path;
};
</script>
