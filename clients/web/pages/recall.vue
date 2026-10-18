<template>
  <div class="space-y-6">
    <PageHeader
      title="Recall settings"
      description="How agents search their memory."
    >
      <template #badge>
        <Badge variant="neutral">{{ currentModeLabel }} mode</Badge>
      </template>
      <template #actions>
        <Button variant="secondary" size="sm" @click="fetchPreview">
          Refresh preview
        </Button>
      </template>
    </PageHeader>

    <div class="grid grid-cols-1 gap-6 max-w-3xl">
      <!-- Settings Form -->
      <div class=" space-y-6">
        <!-- Retrieval Strategy Card -->
        <Card>
          <template #header>
            <div class="flex items-center justify-between">
              <h3 class="text-sm font-semibold text-text">Retrieval strategy</h3>
              <Badge variant="info">Context engine</Badge>
            </div>
          </template>

          <div class="space-y-4">
            <div class="grid grid-cols-1 sm:grid-cols-3 gap-3">
              <button
                v-for="mode in modes"
                :key="mode.id"
                type="button"
                class="p-3 rounded-card border text-left transition-all space-y-1"
                :class="settings.mode === mode.id ? 'bg-surface-2 border-accent text-text ring-1 ring-accent/30' : 'bg-surface border-border text-text-muted hover:text-text hover:bg-surface-2'"
                @click="settings.mode = mode.id"
              >
                <div class="flex items-center justify-between">
                  <span class="text-xs font-bold text-text">{{ mode.name }}</span>
                  <span
                    class="w-2 h-2 rounded-full"
                    :class="settings.mode === mode.id ? 'bg-accent' : 'bg-border'"
                  />
                </div>
                <p class="text-xs text-text-muted leading-tight">{{ mode.description }}</p>
              </button>
            </div>

            <div class="pt-4 border-t border-border grid grid-cols-1 sm:grid-cols-2 gap-4">
              <div class="space-y-1">
                <label class="block text-xs font-medium text-text">Embedding provider</label>
                <select
                  v-model="settings.provider"
                  class="w-full bg-surface-2 border border-border rounded-card p-2 text-xs text-text focus:outline-none focus:ring-1 focus:ring-accent"
                >
                  <option value="fastembed">FastEmbed (Local ONNX, zero-config)</option>
                  <option value="ollama">Ollama (nomic-embed-text)</option>
                  <option value="openai">OpenAI (text-embedding-3-small)</option>
                </select>
              </div>

              <div class="space-y-1">
                <label class="block text-xs font-medium text-text">Top-K recall limit</label>
                <input
                  v-model.number="settings.topK"
                  type="number"
                  min="3"
                  max="50"
                  class="w-full bg-surface-2 border border-border rounded-card p-2 text-xs text-text focus:outline-none focus:ring-1 focus:ring-accent"
                />
              </div>
            </div>
          </div>
        </Card>

        <!-- Context Budgets Card -->
        <Card>
          <template #header>
            <h3 class="text-sm font-semibold text-text">Token budget & decay</h3>
          </template>

          <div class="space-y-4">
            <div class="grid grid-cols-1 sm:grid-cols-2 gap-4">
              <div class="space-y-1">
                <label class="block text-xs font-medium text-text">Max dynamic context tokens</label>
                <input
                  v-model.number="settings.maxContextTokens"
                  type="number"
                  min="500"
                  max="8000"
                  step="250"
                  class="w-full bg-surface-2 border border-border rounded-card p-2 text-xs text-text focus:outline-none focus:ring-1 focus:ring-accent"
                />
                <span class="text-xs text-text-muted">Dynamic recall injection limit per prompt.</span>
              </div>

              <div class="space-y-1">
                <label class="block text-xs font-medium text-text">Recency decay half-life</label>
                <input
                  v-model.number="settings.decayDays"
                  type="number"
                  min="1"
                  max="90"
                  class="w-full bg-surface-2 border border-border rounded-card p-2 text-xs text-text focus:outline-none focus:ring-1 focus:ring-accent"
                />
                <span class="text-xs text-text-muted">Days until memory relevance decays by 50%.</span>
              </div>
            </div>

            <div class="pt-2 flex items-center justify-between">
              <span class="text-xs text-text-muted">CLI command to re-index embeddings:</span>
              <Button variant="primary" size="sm" @click="saveSettings">
                Save settings
              </Button>
            </div>

            <CopyCommand command="automemory embeddings index" />
          </div>
        </Card>
      </div>

      <!-- Live Preview Drawer / Sidebar -->
      <div class="space-y-4">
        <Card>
          <template #header>
            <div class="flex items-center justify-between">
              <h3 class="text-[15px] font-semibold text-text">What agents are sent</h3>
              <Badge variant="neutral">~{{ contextTokens }} tokens</Badge>
            </div>
          </template>

          <div class="space-y-3">
            <p class="text-xs text-text-muted leading-relaxed">
              The context a new agent session starts with, as it is built right now.
            </p>

            <div v-if="loadingPreview" class="space-y-2 py-4">
              <Skeleton class="h-4 w-full" />
              <Skeleton class="h-4 w-3/4" />
              <Skeleton class="h-24 w-full" />
            </div>

            <pre
              v-else-if="contextPayload"
              class="text-xs text-text font-mono whitespace-pre-wrap leading-relaxed bg-surface-2 p-3 rounded-card border border-border overflow-x-auto max-h-96"
            >{{ contextPayload }}</pre>

            <div v-else class="text-xs text-text-muted py-6 text-center">
              No context payload generated yet.
            </div>
          </div>
        </Card>
      </div>
    </div>
  </div>
</template>

<script setup lang="ts">
import { ref, computed, onMounted } from 'vue';
import { apiGet } from '~/utils/api';
import { useToast } from '~/composables/useToast';
import { estimateTokens } from '~/utils/tokens';

const toast = useToast();

const modes = [
  { id: 'hybrid', name: 'Hybrid (BM25 + Vector)', description: 'Recommended: blends keyword matching with semantic vector ranking.' },
  { id: 'keyword', name: 'Keyword only (BM25)', description: 'Ultra-fast FTS5 search with zero extra dependencies.' },
  { id: 'vector', name: 'Dense vector', description: 'Semantic cosine similarity over embedded memory items.' },
];

const settings = ref({
  mode: 'hybrid',
  provider: 'fastembed',
  topK: 12,
  maxContextTokens: 2000,
  decayDays: 14,
});

const currentModeLabel = computed(() => {
  return modes.find((m) => m.id === settings.value.mode)?.name || 'Hybrid';
});

const contextPayload = ref('');
const loadingPreview = ref(false);

const contextTokens = computed(() => {
  return estimateTokens(contextPayload.value);
});

const fetchPreview = async () => {
  loadingPreview.value = true;
  const res = await apiGet<{ context?: string; rules?: string }>('/api/context');
  loadingPreview.value = false;
  if (res.ok && res.data) {
    const raw = res.data.context || res.data.rules || JSON.stringify(res.data, null, 2);
    // Some payloads arrive with literal \n sequences; show real line breaks.
    contextPayload.value = raw.replace(/\\n/g, '\n');
  }
};

const saveSettings = () => {
  if (typeof window !== 'undefined') {
    localStorage.setItem('am_recall_settings', JSON.stringify(settings.value));
  }
  toast.success('Recall settings saved.');
};

onMounted(() => {
  if (typeof window !== 'undefined') {
    const saved = localStorage.getItem('am_recall_settings');
    if (saved) {
      try {
        settings.value = { ...settings.value, ...JSON.parse(saved) };
      } catch {}
    }
  }
  fetchPreview();
});
</script>
