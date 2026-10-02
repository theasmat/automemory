<template>
  <div class="p-8 space-y-6 max-w-7xl mx-auto overflow-y-auto h-full">
    <!-- Header Controls & Token Budget Counter -->
    <div class="flex flex-col md:flex-row md:items-center justify-between gap-4 pb-4 border-b border-dark-700">
      <div>
        <div class="flex items-center gap-2 flex-wrap">
          <BrainCircuit class="w-5 h-5 text-gold-500" />
          <h1 class="text-xl font-bold font-mono text-white">Cognitive Memories</h1>
          <span class="px-2 py-0.5 text-xs font-mono bg-dark-800 text-gold-400 border border-dark-700">
            {{ store.filteredMemories.length }} Active
          </span>
          <span class="px-2 py-0.5 text-xs font-mono bg-dark-900 text-emerald-400 border border-emerald-900/50 flex items-center gap-1">
            <Coins class="w-3 h-3 text-gold-500" />
            <span>~{{ formatTokens(totalDisplayedTokens) }} total tokens</span>
            <span class="text-gray-500 font-normal">({{ estimateTokenCost(totalDisplayedTokens) }})</span>
          </span>
        </div>
        <p class="text-xs font-mono text-gray-500 mt-1">
          Persistent architectural decisions, hard-won lessons, patterns, and compiler constraints.
        </p>
      </div>

      <!-- Scope Filter Switcher -->
      <div class="flex items-center bg-dark-900 border border-dark-700 p-1">
        <button
          v-for="s in scopes"
          :key="s.id"
          @click="store.scopeFilter = s.id"
          class="px-3 py-1.5 text-xs font-mono transition-colors flex items-center gap-1.5"
          :class="[
            store.scopeFilter === s.id
              ? 'bg-dark-700 text-gold-400 border-b-2 border-gold-500 font-bold'
              : 'text-gray-400 hover:text-white'
          ]"
        >
          <span>{{ s.icon }}</span>
          <span>{{ s.label }}</span>
        </button>
      </div>
    </div>

    <!-- Search & Type Filter Toolbar -->
    <div class="flex flex-col sm:flex-row items-stretch sm:items-center justify-between gap-3">
      <!-- FTS5 Search Input -->
      <div class="relative flex-1 max-w-md">
        <Search class="w-4 h-4 text-gray-500 absolute left-3 top-1/2 -translate-y-1/2" />
        <input
          v-model="store.memorySearch"
          type="text"
          placeholder="Search memories (FTS5 BM25 search)..."
          class="w-full bg-dark-900 border border-dark-700 pl-9 pr-8 py-2 text-xs font-mono text-white placeholder-gray-600 focus:outline-none focus:border-gold-500 transition-colors"
        />
        <button
          v-if="store.memorySearch"
          @click="store.memorySearch = ''"
          class="absolute right-2.5 top-1/2 -translate-y-1/2 text-gray-500 hover:text-white text-xs font-mono"
        >
          ✕
        </button>
      </div>

      <!-- Type Filter Pills -->
      <div class="flex items-center gap-1 overflow-x-auto pb-1 sm:pb-0">
        <button
          v-for="t in memoryTypes"
          :key="t"
          @click="selectedType = t"
          class="px-2.5 py-1 text-[11px] font-mono uppercase transition-colors shrink-0"
          :class="[
            selectedType === t
              ? 'bg-gold-500 text-black font-bold'
              : 'bg-dark-900 text-gray-400 hover:text-white border border-dark-700'
          ]"
        >
          {{ t }}
        </button>
      </div>
    </div>

    <!-- Empty State -->
    <div v-if="displayedMemories.length === 0" class="text-center py-16 border border-dark-800 bg-dark-950 p-8 space-y-3">
      <BrainCircuit class="w-10 h-10 text-gray-700 mx-auto" />
      <div class="text-sm font-mono text-gray-400 font-semibold">No memories matching current filters</div>
      <p class="text-xs font-mono text-gray-600 max-w-sm mx-auto">
        Execute coding tasks with agent hooks connected to automatically crystallize durable decisions and lessons.
      </p>
    </div>

    <!-- Memories Grid -->
    <div v-else class="grid grid-cols-1 md:grid-cols-2 gap-4">
      <div
        v-for="m in displayedMemories"
        :key="m.id"
        @click="openMemoryDetail(m)"
        class="p-5 bg-dark-900 border border-dark-700 hover:border-gold-500/50 transition-all duration-150 flex flex-col justify-between group space-y-4 cursor-pointer"
      >
        <div class="space-y-3">
          <!-- Card Header Badges & Token Chip -->
          <div class="flex items-center justify-between gap-2 flex-wrap">
            <div class="flex items-center gap-1.5 flex-wrap">
              <span
                class="px-2 py-0.5 text-[10px] font-mono uppercase font-bold tracking-wider"
                :class="getTypeBadgeClass(m.type)"
              >
                {{ m.type }}
              </span>

              <span class="px-1.5 py-0.5 text-[10px] font-mono text-gray-400 bg-dark-950 border border-dark-800">
                {{ m.scope === 'global' ? '☁️ Cloud' : '🖥️ Vault' }}
              </span>

              <!-- Live Token Count Badge -->
              <span class="px-1.5 py-0.5 text-[10px] font-mono text-gold-400 bg-dark-950 border border-gold-500/30 flex items-center gap-1">
                <Coins class="w-2.5 h-2.5 text-gold-500" />
                <span>~{{ estimateTokens(m.content + ' ' + (m.summary || '')) }} tokens</span>
              </span>

              <span v-if="m.version" class="px-1.5 py-0.5 text-[10px] font-mono text-cyan-400 bg-dark-950 border border-cyan-900">
                v{{ m.version }}
              </span>
            </div>

            <div class="text-[11px] font-mono text-gray-500 flex items-center gap-1.5">
              <span>{{ ((m.confidence || 0.95) * 100).toFixed(0) }}% Conf</span>
              <span class="text-gray-700">•</span>
              <span>{{ formatTime(m.created_at) }}</span>
            </div>
          </div>

          <!-- Summary & Content -->
          <div class="space-y-1.5">
            <div class="text-xs font-mono font-bold text-white leading-snug">
              {{ m.summary || m.content.slice(0, 80) }}
            </div>
            <p class="text-xs font-mono text-gray-300 leading-relaxed bg-dark-950 p-3 border border-dark-800 whitespace-pre-wrap line-clamp-4">
              {{ m.content }}
            </p>
          </div>
        </div>

        <!-- Card Footer Tags & Project -->
        <div class="pt-2 border-t border-dark-800 flex items-center justify-between gap-2 text-[10px] font-mono">
          <div class="flex items-center gap-1 flex-wrap">
            <Tag class="w-3 h-3 text-gray-600" />
            <span
              v-for="t in m.tags"
              :key="t"
              class="px-1.5 py-0.5 bg-dark-850 text-gray-400 border border-dark-700"
            >
              #{{ t }}
            </span>
          </div>

          <div class="flex items-center gap-1.5 text-gray-500 shrink-0">
            <button
              @click.stop="openEditMemory(m)"
              class="px-2 py-0.5 bg-dark-950 hover:bg-dark-800 text-cyan-400 border border-dark-700 hover:border-cyan-500/50 transition-colors"
              title="Edit memory summary and body"
            >
              Edit
            </button>
            <button
              @click.stop="archiveMemoryItem(m.id)"
              class="px-2 py-0.5 bg-dark-950 hover:bg-red-950/60 text-red-400 border border-dark-700 hover:border-red-800 transition-colors"
              title="Archive memory"
            >
              Archive
            </button>
            <span class="text-gray-600 group-hover:text-gold-400 transition-colors ml-1">Details →</span>
          </div>
        </div>
      </div>
    </div>

    <!-- Memory Detail / Edit Modal -->
    <div
      v-if="selectedMemory"
      class="fixed inset-0 bg-black/80 backdrop-blur-sm z-50 flex items-center justify-center p-4 font-mono select-none"
      @click.self="selectedMemory = null"
    >
      <div class="bg-dark-900 border border-gold-500/50 max-w-2xl w-full p-6 shadow-2xl space-y-5 max-h-[90vh] overflow-y-auto">
        <div class="flex items-center justify-between pb-3 border-b border-dark-700">
          <div class="flex items-center gap-2">
            <span
              class="px-2 py-0.5 text-[10px] font-mono uppercase font-bold"
              :class="getTypeBadgeClass(selectedMemory.type)"
            >
              {{ selectedMemory.type }}
            </span>
            <span class="text-xs text-white font-bold">{{ isEditing ? 'Edit Memory' : (selectedMemory.summary || 'Memory Details') }}</span>
          </div>
          <button @click="selectedMemory = null" class="text-gray-500 hover:text-white text-sm">✕</button>
        </div>

        <!-- Token Footprint Breakdown -->
        <div class="p-3 bg-dark-950 border border-dark-700 grid grid-cols-3 gap-3 text-[11px]">
          <div>
            <div class="text-gray-500">Content Tokens:</div>
            <div class="text-gold-400 font-bold">~{{ estimateTokens(selectedMemory.content) }} tokens</div>
          </div>
          <div>
            <div class="text-gray-500">Summary Tokens:</div>
            <div class="text-cyan-400 font-bold">~{{ estimateTokens(selectedMemory.summary) }} tokens</div>
          </div>
          <div>
            <div class="text-gray-500">Injection Cost:</div>
            <div class="text-emerald-400 font-bold">{{ estimateTokenCost(estimateTokens(selectedMemory.content)) }}</div>
          </div>
        </div>

        <!-- Edit Form vs View Mode -->
        <div v-if="isEditing" class="space-y-4">
          <div class="space-y-1">
            <label class="text-[10px] text-gray-500 uppercase">Summary</label>
            <input
              v-model="editSummary"
              type="text"
              class="w-full bg-dark-950 border border-dark-700 p-2 text-xs font-mono text-white focus:outline-none focus:border-gold-500"
            />
          </div>
          <div class="space-y-1">
            <label class="text-[10px] text-gray-500 uppercase">Content Body</label>
            <textarea
              v-model="editContent"
              rows="6"
              class="w-full bg-dark-950 border border-dark-700 p-2 text-xs font-mono text-white focus:outline-none focus:border-gold-500"
            ></textarea>
          </div>
        </div>

        <div v-else class="space-y-1">
          <div class="text-[10px] text-gray-500 uppercase">Memory Body</div>
          <pre class="p-3 bg-dark-950 border border-dark-800 text-xs text-gray-200 whitespace-pre-wrap overflow-x-auto max-h-60">{{ selectedMemory.content }}</pre>
        </div>

        <!-- Metadata & Tags -->
        <div class="grid grid-cols-2 gap-4 text-xs">
          <div>
            <span class="text-gray-500">ID: </span>
            <code class="text-gray-400 text-[10px]">{{ selectedMemory.id }}</code>
          </div>
          <div>
            <span class="text-gray-500">Scope: </span>
            <span class="text-gray-300">{{ selectedMemory.scope }} ({{ selectedMemory.project_id || 'global' }})</span>
          </div>
          <div>
            <span class="text-gray-500">Confidence: </span>
            <span class="text-emerald-400">{{ ((selectedMemory.confidence || 0.95) * 100).toFixed(0) }}%</span>
          </div>
          <div>
            <span class="text-gray-500">Created: </span>
            <span class="text-gray-400">{{ formatTime(selectedMemory.created_at) }}</span>
          </div>
        </div>

        <!-- Footer Actions -->
        <div class="pt-3 border-t border-dark-800 flex items-center justify-between">
          <div class="flex items-center gap-2">
            <button
              v-if="!isEditing"
              @click="archiveMemoryItem(selectedMemory.id)"
              class="px-3 py-1.5 bg-red-950/60 hover:bg-red-900 text-red-300 border border-red-800 text-xs transition-colors"
            >
              Archive Memory
            </button>
          </div>
          <div class="flex items-center gap-2">
            <button
              v-if="!isEditing"
              @click="startEditing(selectedMemory)"
              class="px-3 py-1.5 bg-dark-800 hover:bg-dark-700 text-cyan-400 border border-cyan-800/60 text-xs transition-colors"
            >
              Edit
            </button>
            <template v-else>
              <button
                @click="isEditing = false"
                class="px-3 py-1.5 bg-dark-800 hover:bg-dark-700 text-gray-400 text-xs"
              >
                Cancel
              </button>
              <button
                @click="saveMemoryEdit"
                class="px-3 py-1.5 bg-gold-500 hover:bg-gold-400 text-black font-bold text-xs shadow-sm shadow-gold-500/20"
              >
                Save Changes
              </button>
            </template>
            <button
              v-if="!isEditing"
              @click="selectedMemory = null"
              class="px-4 py-1.5 bg-dark-800 hover:bg-dark-700 text-white text-xs border border-dark-700"
            >
              Close
            </button>
          </div>
        </div>
      </div>
    </div>
  </div>
</template>

<script setup lang="ts">
import { ref, computed } from 'vue';
import { useAutoMemoryStore } from '~/stores/automemory';
import { estimateTokens, formatTokens, estimateTokenCost } from '~/utils/tokens';
import type { Memory } from '~/types';
import { BrainCircuit, Search, Tag, Coins } from 'lucide-vue-next';

const store = useAutoMemoryStore();
const selectedType = ref('all');
const selectedMemory = ref<Memory | null>(null);

const scopes = [
  { id: 'all' as const, label: 'All Scopes', icon: '🌐' },
  { id: 'vault' as const, label: 'Local Vault', icon: '🖥️' },
  { id: 'cloud' as const, label: 'Cloud Synced', icon: '☁️' },
];

const memoryTypes = ['all', 'decision', 'fact', 'lesson', 'pattern', 'warning', 'skill'];

const displayedMemories = computed(() => {
  let list = store.filteredMemories;
  if (selectedType.value !== 'all') {
    list = list.filter((m) => m.type.toLowerCase() === selectedType.value.toLowerCase());
  }
  return list;
});

const totalDisplayedTokens = computed(() => {
  return displayedMemories.value.reduce(
    (acc, m) => acc + estimateTokens(m.content + ' ' + (m.summary || '')),
    0
  );
});

const openMemoryDetail = (m: Memory) => {
  isEditing.value = false;
  selectedMemory.value = m;
};

const isEditing = ref(false);
const editSummary = ref('');
const editContent = ref('');

const openEditMemory = (m: Memory) => {
  selectedMemory.value = m;
  startEditing(m);
};

const startEditing = (m: Memory) => {
  isEditing.value = true;
  editSummary.value = m.summary;
  editContent.value = m.content;
};

const saveMemoryEdit = async () => {
  if (!selectedMemory.value) return;
  const ok = await store.updateMemory(selectedMemory.value.id, {
    summary: editSummary.value,
    content: editContent.value,
  });
  if (ok) {
    selectedMemory.value.summary = editSummary.value;
    selectedMemory.value.content = editContent.value;
    isEditing.value = false;
  }
};

const archiveMemoryItem = async (id: string) => {
  if (confirm('Archive this cognitive memory? It will no longer be injected into active LLM context.')) {
    await store.archiveMemory(id);
    if (selectedMemory.value?.id === id) {
      selectedMemory.value = null;
    }
  }
};

const getTypeBadgeClass = (type: string) => {
  switch (type.toLowerCase()) {
    case 'decision':
      return 'bg-gold-500/20 text-gold-400 border border-gold-500/40';
    case 'fact':
      return 'bg-cyan-500/20 text-cyan-400 border border-cyan-500/40';
    case 'lesson':
      return 'bg-emerald-500/20 text-emerald-400 border border-emerald-500/40';
    case 'pattern':
      return 'bg-purple-500/20 text-purple-400 border border-purple-500/40';
    case 'warning':
      return 'bg-red-500/20 text-red-400 border border-red-500/40';
    default:
      return 'bg-dark-800 text-gray-300 border border-dark-700';
  }
};

const formatTime = (iso: string) => {
  try {
    const d = new Date(iso);
    return d.toLocaleDateString(undefined, { month: 'short', day: 'numeric', hour: '2-digit', minute: '2-digit' });
  } catch {
    return iso;
  }
};
</script>
