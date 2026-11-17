<template>
  <!-- Tablet & Desktop left rail (>= 640px): icons only below 1280px, full sidebar above (the person can toggle) -->
  <aside
    data-tour="nav-rail"
    class="hidden sm:flex flex-col bg-bg border-r border-border transition-all duration-200 z-30 flex-shrink-0"
    :class="isExpanded ? 'w-60' : 'w-16'"
  >
    <!-- Brand / Logo -->
    <div class="h-14 flex items-center border-b border-border" :class="isExpanded ? 'px-4 justify-between' : 'justify-center'">
      <NuxtLink to="/" class="flex items-center gap-2.5 overflow-hidden" :aria-label="isExpanded ? undefined : 'AutoMemory home'">
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
        v-if="isExpanded"
        class="flex text-text-muted hover:text-text p-1 rounded"
        title="Collapse sidebar"
        aria-label="Collapse sidebar"
        @click="userExpanded = false"
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
        class="flex items-center gap-3 px-3 py-2.5 rounded-card text-sm font-medium transition-colors relative"
        :class="[!isExpanded && 'justify-center', isActive(item.path) ? 'bg-surface text-accent font-semibold shadow-sm border-l-2 border-l-accent' : 'text-text-muted hover:text-text hover:bg-surface-2 border-l-2 border-l-transparent']"
        :title="item.label"
        :aria-label="item.label"
      >
        <component :is="item.icon" class="w-5 h-5 flex-shrink-0" aria-hidden="true" />
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
        class="flex items-center gap-3 px-3 py-2 rounded-card text-xs font-medium transition-colors"
        :class="[!isExpanded && 'justify-center', isActive(item.path) ? 'bg-surface text-accent font-semibold shadow-sm border-l-2 border-l-accent' : 'text-text-muted hover:text-text hover:bg-surface-2 border-l-2 border-l-transparent']"
        :title="item.label"
        :aria-label="item.label"
      >
        <component :is="item.icon" class="w-4 h-4 flex-shrink-0" aria-hidden="true" />
        <span v-if="isExpanded" class="truncate">{{ item.label }}</span>
      </NuxtLink>
    </nav>

    <!-- Footer / Version -->
    <div v-if="isExpanded" class="p-3 border-t border-border text-xs text-text-muted flex items-center justify-between">
      <span>AutoMemory v0.1</span>
      <span class="w-2 h-2 rounded-full bg-ok" title="Daemon healthy" />
    </div>
    <button
      v-else
      type="button"
      class="h-11 border-t border-border flex items-center justify-center text-text-muted hover:text-text"
      title="Expand sidebar"
      aria-label="Expand sidebar"
      @click="userExpanded = true"
    >
      <svg class="w-4 h-4 rotate-180" fill="none" viewBox="0 0 24 24" stroke="currentColor" aria-hidden="true"><path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M11 19l-7-7 7-7m8 14l-7-7 7-7" /></svg>
    </button>
  </aside>

  <!-- Phone bottom bar (< 640px): five slots, the rest is under More -->
  <nav
    data-tour="nav-rail"
    class="sm:hidden fixed bottom-0 inset-x-0 bg-bg/95 backdrop-blur-md border-t border-border z-40 flex items-stretch justify-around h-[calc(3.5rem+env(safe-area-inset-bottom))] pb-[env(safe-area-inset-bottom)] px-1"
    aria-label="Mobile navigation"
  >
    <NuxtLink
      v-for="item in mobileNav"
      :key="item.path"
      :to="item.path"
      class="flex flex-col items-center justify-center flex-1 min-w-0 text-xs font-medium transition-colors relative"
      :class="isActive(item.path) ? 'text-accent font-semibold' : 'text-text-muted hover:text-text'"
      :aria-current="isActive(item.path) ? 'page' : undefined"
    >
      <span v-if="isActive(item.path)" class="absolute top-0 h-0.5 w-8 rounded-b bg-accent-solid" aria-hidden="true" />
      <component :is="item.icon" class="w-5 h-5 mb-0.5" aria-hidden="true" />
      <span>{{ item.shortLabel || item.label }}</span>
      <span
        v-if="item.badge && item.badge > 0"
        class="absolute top-1.5 left-1/2 ml-1.5 text-xs font-mono font-bold px-1 py-0 rounded-pill bg-accent-solid text-accent-fg"
      >
        {{ item.badge > 99 ? '99+' : item.badge }}
      </span>
    </NuxtLink>

    <!-- More button for mobile drawer -->
    <button
      type="button"
      class="flex flex-col items-center justify-center flex-1 min-w-0 text-xs font-medium text-text-muted hover:text-text relative"
      aria-haspopup="dialog"
      @click="mobileMoreOpen = true"
    >
      <MoreHorizontalIcon class="w-5 h-5 mb-0.5" aria-hidden="true" />
      <span>More</span>
    </button>
  </nav>

  <!-- Mobile More Drawer -->
  <Drawer
    :is-open="mobileMoreOpen"
    title="More"
    width-class="max-w-xs"
    @close="mobileMoreOpen = false"
  >
    <div class="space-y-1">
      <NuxtLink
        v-for="item in moreNav"
        :key="item.path"
        :to="item.path"
        class="flex items-center gap-3 px-3 min-h-11 rounded-card text-sm font-medium transition-colors"
        :class="isActive(item.path) ? 'bg-surface text-accent font-semibold shadow-sm border-l-2 border-l-accent' : 'text-text hover:bg-surface-2 border-l-2 border-l-transparent'"
        @click="mobileMoreOpen = false"
      >
        <component :is="item.icon" class="w-5 h-5 text-text-muted" aria-hidden="true" />
        <span>{{ item.label }}</span>
        <span v-if="item.path === '/safety'" class="ml-auto text-xs text-text-muted">{{ modeLabel }}</span>
      </NuxtLink>
    </div>
    <div class="mt-4 pt-3 border-t border-border space-y-1">
      <button type="button" class="w-full flex items-center gap-3 px-3 min-h-11 rounded-card text-sm font-medium text-text hover:bg-surface-2" @click="cycleTheme">
        <span class="w-5 text-center text-text-muted" aria-hidden="true">◐</span>
        <span>Theme</span>
        <span class="ml-auto text-xs text-text-muted">{{ themeLabel }}</span>
      </button>
      <button type="button" class="w-full flex items-center gap-3 px-3 min-h-11 rounded-card text-sm font-medium text-text hover:bg-surface-2" @click="openHelp">
        <span class="w-5 text-center font-mono text-text-muted" aria-hidden="true">?</span>
        <span>Help &amp; guided learning</span>
      </button>
    </div>
  </Drawer>
</template>

<script setup lang="ts">
import { ref, computed } from 'vue';
import { useRoute } from 'vue-router';
import { useMediaQuery } from '@vueuse/core';
import { useAutoMemoryStore } from '~/stores/automemory';
import { useTheme } from '~/composables/useTheme';
import {
  Home as HomeIcon,
  Brain as BrainIcon,
  ShieldAlert as ShieldIcon,
  Inbox as InboxIcon,
  RefreshCw as SyncIcon,
  Users as AgentsIcon,
  FileText as DocsIcon,
  History as HistoryIcon,
  Layers as ContinuityIcon,
  Sliders as RecallIcon,
  MoreHorizontal as MoreHorizontalIcon,
  Settings as SettingsIcon,
  Wrench as ToolsIcon,
  MessagesSquare as RoomsIcon,
} from 'lucide-vue-next';

const route = useRoute();
const store = useAutoMemoryStore();
// Full sidebar from 1280px, icon rail below; a person's own toggle wins.
const wide = useMediaQuery('(min-width: 1280px)');
const userExpanded = ref<boolean | null>(null);
const isExpanded = computed({
  get: () => userExpanded.value ?? wide.value,
  set: (v: boolean) => { userExpanded.value = v; },
});
const mobileMoreOpen = ref(false);
const { themeLabel, cycleTheme } = useTheme();
const modeLabel = computed(() => (store.currentMode === 'strict' ? 'Strict' : store.currentMode === 'review' ? 'Balanced' : 'Relaxed'));
function openHelp() {
  mobileMoreOpen.value = false;
  store.openHelpDrawer();
}

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
]);

const advancedNav = [
  { path: '/continuity', label: 'Continuity', icon: ContinuityIcon },
  { path: '/library', label: 'Library', icon: DocsIcon },
  { path: '/rooms', label: 'Planning rooms', icon: RoomsIcon },
  { path: '/activity', label: 'Activity', icon: HistoryIcon },
  { path: '/models', label: 'Models', icon: RecallIcon },
  { path: '/tools', label: 'Tools', icon: ToolsIcon },
  { path: '/settings', label: 'Settings', icon: SettingsIcon },
];

// Under More on a phone: what the bottom bar has no room for, then the advanced pages.
const moreNav = computed(() => [
  { path: '/safety', label: 'Safety', icon: ShieldIcon },
  { path: '/sync', label: 'Sync & devices', icon: SyncIcon },
  ...advancedNav,
]);

function isActive(path: string): boolean {
  return route.path === path;
}
</script>
