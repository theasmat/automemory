<template>
  <!-- Desktop & Tablet Left Rail (>= 768px) -->
  <aside
    data-tour="nav-rail"
    class="hidden md:flex flex-col bg-bg border-r border-border transition-all duration-200 z-30 flex-shrink-0"
    :class="isExpanded ? 'w-60' : 'w-16'"
  >
    <!-- Brand / Logo -->
    <div class="h-14 flex items-center px-4 border-b border-border justify-between">
      <NuxtLink to="/" class="flex items-center gap-2.5 overflow-hidden">
        <div class="w-8 h-8 rounded-card bg-accent-solid flex items-center justify-center text-accent-fg font-black text-sm flex-shrink-0 shadow-sm">
          A
        </div>
        <div v-if="isExpanded" class="flex flex-col min-w-0">
          <span class="font-bold text-sm text-text tracking-tight truncate leading-tight">AutoMemory</span>
          <span class="text-xs text-text-muted truncate leading-tight">Shared Agent Brain</span>
        </div>
      </NuxtLink>

      <button
        type="button"
        class="hidden xl:flex text-text-muted hover:text-text p-1 rounded"
        :title="isExpanded ? 'Collapse sidebar' : 'Expand sidebar'"
        @click="isExpanded = !isExpanded"
      >
        <svg class="w-4 h-4 transition-transform" :class="isExpanded ? '' : 'rotate-180'" fill="none" viewBox="0 0 24 24" stroke="currentColor">
          <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M11 19l-7-7 7-7m8 14l-7-7 7-7" />
        </svg>
      </button>
    </div>

    <!-- Main Navigation List -->
    <nav class="flex-1 overflow-y-auto p-2 space-y-1" aria-label="Main navigation">
      <NuxtLink
        v-for="item in mainNav"
        :key="item.path"
        :to="item.path"
        class="flex items-center gap-3 px-3 py-2 rounded-card text-sm font-medium transition-colors relative"
        :class="isActive(item.path) ? 'bg-surface text-accent font-semibold shadow-sm border-l-2 border-l-accent' : 'text-text-muted hover:text-text hover:bg-surface-2 border-l-2 border-l-transparent'"
        :title="item.label"
      >
        <component :is="item.icon" class="w-4 h-4 flex-shrink-0" />
        <span v-if="isExpanded" class="truncate">{{ item.label }}</span>
        
        <!-- Badge for Inbox -->
        <span
          v-if="item.badge && item.badge > 0"
          class="ml-auto text-xs font-mono font-bold px-1.5 py-0.2 rounded-pill bg-accent-solid text-accent-fg"
          :class="isExpanded ? '' : 'absolute top-1 right-1 text-xs px-1 py-0'"
        >
          {{ item.badge > 99 ? '99+' : item.badge }}
        </span>
      </NuxtLink>

      <!-- Advanced / More Section Divider -->
      <div class="pt-3 pb-1">
        <div class="border-t border-border mx-2" />
        <div v-if="isExpanded" class="px-3 pt-3 pb-1 text-xs font-semibold text-text-muted">
          Advanced
        </div>
      </div>

      <NuxtLink
        v-for="item in advancedNav"
        :key="item.path"
        :to="item.path"
        class="flex items-center gap-3 px-3 py-1.5 rounded-card text-xs font-medium transition-colors"
        :class="isActive(item.path) ? 'bg-surface text-accent font-semibold shadow-sm border-l-2 border-l-accent' : 'text-text-muted hover:text-text hover:bg-surface-2 border-l-2 border-l-transparent'"
        :title="item.label"
      >
        <component :is="item.icon" class="w-3.5 h-3.5 flex-shrink-0" />
        <span v-if="isExpanded" class="truncate">{{ item.label }}</span>
      </NuxtLink>
    </nav>

    <!-- Footer / Version -->
    <div v-if="isExpanded" class="p-3 border-t border-border text-xs text-text-muted flex items-center justify-between">
      <span>AutoMemory v0.1</span>
      <span class="w-2 h-2 rounded-full bg-ok" title="Daemon healthy" />
    </div>
  </aside>

  <!-- Mobile Bottom Tab Bar (< 768px) -->
  <nav
    data-tour="nav-rail"
    class="md:hidden fixed bottom-0 inset-x-0 bg-bg/95 backdrop-blur-md border-t border-border z-40 flex items-center justify-around h-14 px-2"
    aria-label="Mobile navigation"
  >
    <NuxtLink
      v-for="item in mobileNav"
      :key="item.path"
      :to="item.path"
      class="flex flex-col items-center justify-center flex-1 py-1 text-xs font-medium transition-colors relative"
      :class="isActive(item.path) ? 'text-accent font-semibold' : 'text-text-muted hover:text-text'"
    >
      <component :is="item.icon" class="w-4 h-4 mb-0.5" />
      <span>{{ item.shortLabel || item.label }}</span>
      <span
        v-if="item.badge && item.badge > 0"
        class="absolute top-1 right-3 text-xs font-mono font-bold px-1 py-0 rounded-pill bg-accent-solid text-accent-fg"
      >
        {{ item.badge > 99 ? '99+' : item.badge }}
      </span>
    </NuxtLink>

    <!-- More button for mobile drawer -->
    <button
      type="button"
      class="flex flex-col items-center justify-center flex-1 py-1 text-xs font-medium text-text-muted hover:text-text"
      @click="mobileMoreOpen = true"
    >
      <MoreHorizontalIcon class="w-4 h-4 mb-0.5" />
      <span>More</span>
    </button>
  </nav>

  <!-- Mobile More Drawer -->
  <Drawer
    :is-open="mobileMoreOpen"
    title="More Tools"
    width-class="max-w-xs"
    @close="mobileMoreOpen = false"
  >
    <div class="space-y-1">
      <NuxtLink
        v-for="item in advancedNav"
        :key="item.path"
        :to="item.path"
        class="flex items-center gap-3 px-3 py-2.5 rounded-card text-sm font-medium transition-colors"
        :class="isActive(item.path) ? 'bg-surface text-accent font-semibold shadow-sm border-l-2 border-l-accent' : 'text-text hover:bg-surface-2 border-l-2 border-l-transparent'"
        @click="mobileMoreOpen = false"
      >
        <component :is="item.icon" class="w-4 h-4 text-text-muted" />
        <span>{{ item.label }}</span>
      </NuxtLink>
    </div>
  </Drawer>
</template>

<script setup lang="ts">
import { ref, computed } from 'vue';
import { useRoute } from 'vue-router';
import { useAutoMemoryStore } from '~/stores/automemory';
import {
  Home as HomeIcon,
  Brain as BrainIcon,
  ShieldAlert as ShieldIcon,
  Inbox as InboxIcon,
  RefreshCw as SyncIcon,
  Users as AgentsIcon,
  FileText as DocsIcon,
  Scroll as RulesIcon,
  History as HistoryIcon,
  Share2 as GraphIcon,
  Sliders as RecallIcon,
  MoreHorizontal as MoreHorizontalIcon,
  Settings as SettingsIcon,
  Wrench as ToolsIcon,
} from 'lucide-vue-next';

const route = useRoute();
const store = useAutoMemoryStore();
const isExpanded = ref(true);
const mobileMoreOpen = ref(false);

const pendingCount = computed(() => {
  return store.stats?.pending_proposals || store.proposals?.length || 0;
});

const mainNav = computed(() => [
  { path: '/', label: 'Home', icon: HomeIcon, shortLabel: 'Home' },
  { path: '/brain', label: 'Project brain', icon: BrainIcon, shortLabel: 'Brain' },
  { path: '/agents', label: 'Agents & locks', icon: AgentsIcon, shortLabel: 'Agents' },
  { path: '/inbox', label: 'Inbox', icon: InboxIcon, shortLabel: 'Inbox', badge: pendingCount.value },
  { path: '/sync', label: 'Sync & devices', icon: SyncIcon, shortLabel: 'Sync' },
  { path: '/safety', label: 'Safety', icon: ShieldIcon, shortLabel: 'Safety' },
]);

const mobileNav = computed(() => [
  { path: '/', label: 'Home', icon: HomeIcon, shortLabel: 'Home' },
  { path: '/brain', label: 'Brain', icon: BrainIcon, shortLabel: 'Brain' },
  { path: '/agents', label: 'Agents', icon: AgentsIcon, shortLabel: 'Agents' },
  { path: '/inbox', label: 'Inbox', icon: InboxIcon, shortLabel: 'Inbox', badge: pendingCount.value },
  { path: '/safety', label: 'Safety', icon: ShieldIcon, shortLabel: 'Safety' },
]);

const advancedNav = [
  { path: '/continuity', label: 'Continuity', icon: SyncIcon },
  { path: '/library', label: 'Library', icon: DocsIcon },
  { path: '/rooms', label: 'Planning rooms', icon: AgentsIcon },
  { path: '/docs', label: 'Documents', icon: DocsIcon },
  { path: '/constitution', label: 'Rules of the project', icon: RulesIcon },
  { path: '/graph', label: 'Knowledge graph', icon: GraphIcon },
  { path: '/activity', label: 'Activity', icon: HistoryIcon },
  { path: '/models', label: 'Models', icon: RecallIcon },
  { path: '/tools', label: 'Tools', icon: ToolsIcon },
  { path: '/settings', label: 'Settings', icon: SettingsIcon },
];

function isActive(path: string): boolean {
  return route.path === path;
}
</script>
