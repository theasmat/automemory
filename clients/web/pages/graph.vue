<template>
  <div class="space-y-6">
    <PageHeader
      title="Knowledge graph"
      subtitle="Visual companion map of files, symbols, relations, and impact chains."
    >
      <template #badge>
        <Badge variant="neutral">{{ entities.length }} entities · {{ edges.length }} edges</Badge>
      </template>
      <template #actions>
        <Button variant="secondary" size="sm" :loading="loading" @click="fetchGraph">
          Refresh
        </Button>
      </template>
    </PageHeader>

    <!-- Error State -->
    <ErrorState
      v-if="error"
      title="Failed to load knowledge graph"
      :message="error"
      command="automemory graph index"
      @retry="fetchGraph"
    />

    <template v-else>
      <!-- Stats Overview -->
      <div class="grid grid-cols-2 sm:grid-cols-4 gap-3 sm:gap-4">
        <StatCard label="Total entities" :value="entities.length" />
        <StatCard label="Total edges" :value="edges.length" />
        <StatCard label="Files tracked" :value="fileCount" />
        <StatCard label="Symbols & traits" :value="symbolCount" />
      </div>

      <!-- Controls -->
      <div class="flex flex-col sm:flex-row items-stretch sm:items-center justify-between gap-3">
        <!-- Type Filter -->
        <div class="flex items-center gap-1.5 flex-wrap">
          <button
            v-for="t in typeFilters"
            :key="t.key"
            type="button"
            class="px-2.5 py-1 rounded-card text-xs font-medium transition-colors"
            :class="selectedType === t.key ? 'bg-surface-2 text-text font-semibold border border-border shadow-sm' : 'text-text-muted hover:text-text'"
            @click="selectedType = t.key"
          >
            {{ t.label }}
          </button>
        </div>

        <!-- Search -->
        <div class="w-full sm:w-64">
          <SearchBox
            v-model="searchQuery"
            placeholder="Search symbols or files..."
          />
        </div>
      </div>

      <!-- Empty State -->
      <div v-if="filteredEntities.length === 0" class="py-8">
        <Card>
          <EmptyState
            title="No graph entities found"
            message="AutoMemory can construct an AST knowledge graph across your files and symbols."
          >
            <template #extra>
              <div class="mt-4 max-w-md mx-auto text-left">
                <CopyCommand command="automemory graph index" />
              </div>
            </template>
          </EmptyState>
        </Card>
      </div>

      <!-- Entity & Relation Explorer -->
      <div v-else class="grid grid-cols-1 lg:grid-cols-3 gap-6">
        <!-- Entities List -->
        <div class="lg:col-span-1 space-y-2 max-h-[700px] overflow-y-auto pr-1">
          <div
            v-for="entity in filteredEntities"
            :key="entity.id"
            class="p-3 rounded-card border transition-all cursor-pointer text-xs"
            :class="selectedEntity?.id === entity.id ? 'bg-surface-2 border-accent text-text ring-1 ring-accent/30 font-semibold' : 'bg-surface border-border text-text hover:bg-surface-2'"
            @click="selectEntity(entity)"
          >
            <div class="flex items-center justify-between mb-1">
              <Badge :variant="getEntityBadgeVariant(entity.entity_type)">
                {{ entity.entity_type }}
              </Badge>
              <span class="text-[11px] text-text-muted font-mono truncate max-w-[120px]">
                {{ entity.path }}
              </span>
            </div>
            <div class="text-sm font-semibold truncate">{{ entity.name }}</div>
            <div v-if="entity.line_start" class="text-[11px] text-text-muted font-mono mt-1">
              Line {{ entity.line_start }}<span v-if="entity.line_end"> - {{ entity.line_end }}</span>
            </div>
          </div>
        </div>

        <!-- Entity Details & Connections -->
        <div class="lg:col-span-2">
          <Card v-if="selectedEntity">
            <template #header>
              <div class="flex items-center justify-between flex-wrap gap-2">
                <div class="flex items-center gap-2">
                  <Badge :variant="getEntityBadgeVariant(selectedEntity.entity_type)">
                    {{ selectedEntity.entity_type }}
                  </Badge>
                  <h3 class="text-base font-bold text-text">{{ selectedEntity.name }}</h3>
                </div>
                <div class="text-xs text-text-muted font-mono">
                  {{ selectedEntity.path }}
                </div>
              </div>
            </template>

            <div class="space-y-6">
              <!-- Location Info -->
              <div class="bg-surface-2 p-3 rounded-card border border-border text-xs space-y-1">
                <div class="text-text-muted">Defined in:</div>
                <div class="font-mono text-text">
                  {{ selectedEntity.path }}<span v-if="selectedEntity.line_start">:{{ selectedEntity.line_start }}</span>
                </div>
              </div>

              <!-- Outgoing Relations (calls, imports, defines) -->
              <div class="space-y-2">
                <div class="text-xs font-semibold text-text uppercase tracking-wider">
                  Outgoing relations ({{ outgoingEdges.length }})
                </div>
                <div v-if="outgoingEdges.length === 0" class="text-xs text-text-muted py-2">
                  No outgoing relations recorded.
                </div>
                <div v-else class="space-y-1.5">
                  <div
                    v-for="(edge, idx) in outgoingEdges"
                    :key="idx"
                    class="p-2.5 bg-surface-2 rounded border border-border flex items-center justify-between text-xs"
                  >
                    <div class="flex items-center gap-2">
                      <Badge variant="neutral">{{ edge.relation_type }}</Badge>
                      <span class="font-medium text-text">{{ getEntityName(edge.target_id) }}</span>
                    </div>
                    <span class="text-[11px] text-text-muted font-mono">
                      {{ Math.round((edge.confidence || 1) * 100) }}% confidence
                    </span>
                  </div>
                </div>
              </div>

              <!-- Incoming Relations (called by, imported by) -->
              <div class="space-y-2">
                <div class="text-xs font-semibold text-text uppercase tracking-wider">
                  Referenced by ({{ incomingEdges.length }})
                </div>
                <div v-if="incomingEdges.length === 0" class="text-xs text-text-muted py-2">
                  No incoming references found.
                </div>
                <div v-else class="space-y-1.5">
                  <div
                    v-for="(edge, idx) in incomingEdges"
                    :key="idx"
                    class="p-2.5 bg-surface-2 rounded border border-border flex items-center justify-between text-xs"
                  >
                    <div class="flex items-center gap-2">
                      <Badge variant="info">{{ edge.relation_type }}</Badge>
                      <span class="font-medium text-text">{{ getEntityName(edge.source_id) }}</span>
                    </div>
                    <span class="text-[11px] text-text-muted font-mono">
                      {{ Math.round((edge.confidence || 1) * 100) }}% confidence
                    </span>
                  </div>
                </div>
              </div>
            </div>
          </Card>

          <Card v-else>
            <div class="py-12 text-center text-text-muted text-xs">
              Select an entity from the list to view its connections and impact chain.
            </div>
          </Card>
        </div>
      </div>
    </template>
  </div>
</template>

<script setup lang="ts">
import { ref, computed, onMounted } from 'vue';
import { apiGet } from '~/utils/api';

interface GraphEntity {
  id: string;
  project_id?: string;
  entity_type: string;
  name: string;
  path: string;
  line_start?: number;
  line_end?: number;
  metadata?: Record<string, any>;
  created_at: string;
  updated_at: string;
}

interface GraphEdge {
  source_id: string;
  target_id: string;
  relation_type: string;
  confidence: number;
  created_at: string;
}

const loading = ref(false);
const error = ref<string | null>(null);
const entities = ref<GraphEntity[]>([]);
const edges = ref<GraphEdge[]>([]);
const selectedEntity = ref<GraphEntity | null>(null);
const selectedType = ref('all');
const searchQuery = ref('');

const typeFilters = [
  { key: 'all', label: 'All types' },
  { key: 'file', label: 'Files' },
  { key: 'function', label: 'Functions' },
  { key: 'struct', label: 'Structs' },
  { key: 'class', label: 'Classes' },
  { key: 'module', label: 'Modules' },
];

const fetchGraph = async () => {
  loading.value = true;
  error.value = null;
  const res = await apiGet<{ entities: GraphEntity[]; edges: GraphEdge[] }>('/api/graph');
  loading.value = false;
  if (!res.ok) {
    error.value = res.error || 'Failed to load graph data';
    return;
  }
  entities.value = res.data?.entities || [];
  edges.value = res.data?.edges || [];
  if (entities.value.length > 0 && !selectedEntity.value) {
    selectedEntity.value = entities.value[0];
  }
};

onMounted(() => {
  fetchGraph();
});

const fileCount = computed(() => entities.value.filter((e) => e.entity_type === 'file').length);
const symbolCount = computed(() => entities.value.filter((e) => e.entity_type !== 'file').length);

const filteredEntities = computed(() => {
  let list = entities.value;
  if (selectedType.value !== 'all') {
    list = list.filter((e) => e.entity_type === selectedType.value);
  }
  if (searchQuery.value.trim()) {
    const q = searchQuery.value.toLowerCase();
    list = list.filter((e) => e.name.toLowerCase().includes(q) || e.path.toLowerCase().includes(q));
  }
  return list;
});

const selectEntity = (entity: GraphEntity) => {
  selectedEntity.value = entity;
};

const outgoingEdges = computed(() => {
  if (!selectedEntity.value) return [];
  return edges.value.filter((e) => e.source_id === selectedEntity.value?.id);
});

const incomingEdges = computed(() => {
  if (!selectedEntity.value) return [];
  return edges.value.filter((e) => e.target_id === selectedEntity.value?.id);
});

const getEntityName = (id: string) => {
  const target = entities.value.find((e) => e.id === id);
  return target ? target.name : id;
};

const getEntityBadgeVariant = (type: string): 'neutral' | 'ok' | 'warn' | 'bad' | 'info' => {
  switch (type) {
    case 'file':
      return 'neutral';
    case 'function':
      return 'info';
    case 'struct':
    case 'class':
      return 'ok';
    case 'module':
      return 'warn';
    default:
      return 'neutral';
  }
};
</script>
