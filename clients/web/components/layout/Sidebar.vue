<template>
  <aside class="w-64 border-r border-dark-700 bg-dark-950 flex flex-col justify-between shrink-0 h-screen select-none">
    <!-- Brand Header -->
    <div>
      <div class="p-5 border-b border-dark-700 flex items-center gap-3">
        <div class="w-8 h-8 rounded-none bg-gold-500 text-black flex items-center justify-center font-bold font-mono text-sm tracking-tighter shadow-lg shadow-gold-500/20">
          AM
        </div>
        <div>
          <div class="text-sm font-bold font-mono text-white tracking-wider flex items-center gap-1.5">
            AutoMemory <span class="text-[10px] px-1 bg-gold-500/20 text-gold-400 border border-gold-500/40 font-mono">PRO</span>
          </div>
          <div class="text-[10px] text-gray-500 font-mono">Agent Brain & Judgment</div>
        </div>
      </div>

      <!-- Navigation Links -->
      <nav class="p-3 space-y-1">
        <NuxtLink
          v-for="item in navItems"
          :key="item.path"
          :to="item.path"
          class="flex items-center justify-between px-3 py-2.5 text-xs font-mono transition-all duration-150 group"
          :class="[
            isCurrentRoute(item.path)
              ? 'bg-dark-850 text-gold-400 border-l-2 border-gold-500 font-semibold'
              : 'text-gray-400 hover:text-gray-100 hover:bg-dark-900 border-l-2 border-transparent'
          ]"
        >
          <div class="flex items-center gap-2.5">
            <component :is="item.icon" class="w-4 h-4 transition-transform group-hover:scale-110" :class="isCurrentRoute(item.path) ? 'text-gold-500' : 'text-gray-500 group-hover:text-gray-300'" />
            <span>{{ item.label }}</span>
          </div>

          <span
            v-if="item.badge !== undefined && item.badge > 0"
            class="px-1.5 py-0.2 text-[10px] font-mono"
            :class="item.badgeClass || 'bg-dark-800 text-gray-400 border border-dark-700'"
          >
            {{ item.badge }}
          </span>
        </NuxtLink>
      </nav>
    </div>

    <!-- Daemon & Version Footer -->
    <div class="p-4 border-t border-dark-700 bg-dark-900/50 space-y-2">
      <div class="flex items-center justify-between text-[11px] font-mono">
        <span class="text-gray-500">Daemon</span>
        <span class="flex items-center gap-1.5 text-emerald-400 font-bold">
          <span class="w-1.5 h-1.5 rounded-full bg-emerald-400 animate-pulse"></span>
          Sub-ms Rust
        </span>
      </div>
      <div class="flex items-center justify-between text-[10px] font-mono text-gray-600">
        <span>Dual-Engine</span>
        <span class="text-gray-400">v0.1.0</span>
      </div>
    </div>
  </aside>
</template>

<script setup lang="ts">
import { computed } from 'vue';
import { useRoute } from 'vue-router';
import { useAutoMemoryStore } from '~/stores/automemory';
import {
  LayoutDashboard,
  BrainCircuit,
  History,
  Footprints,
  AlertTriangle,
  GitPullRequestDraft,
  ShieldAlert,
  Sparkles,
  BookOpen,
  FileText,
  Cloud,
} from 'lucide-vue-next';

const route = useRoute();
const store = useAutoMemoryStore();

const isCurrentRoute = (path: string) => {
  if (path === '/overview' || path === '/') {
    return route.path === '/' || route.path === '/overview';
  }
  return route.path.startsWith(path);
};

const navItems = computed(() => [
  {
    path: '/overview',
    label: 'Overview',
    icon: LayoutDashboard,
  },
  {
    path: '/docs',
    label: 'Product Specs',
    icon: FileText,
    badge: store.docs?.length,
    badgeClass: 'bg-cyan-500/10 text-cyan-400 border border-cyan-500/30',
  },
  {
    path: '/sync',
    label: 'Cloud Sync',
    icon: Cloud,
    badge: store.syncStatus?.pending_deltas > 0 ? store.syncStatus.pending_deltas : undefined,
    badgeClass: 'bg-amber-500/20 text-amber-400 border border-amber-500/50 animate-pulse',
  },
  {
    path: '/memories',
    label: 'Memories',
    icon: BrainCircuit,
    badge: store.stats.active_memories,
    badgeClass: 'bg-gold-500/10 text-gold-400 border border-gold-500/30',
  },
  {
    path: '/skills',
    label: 'Procedural Skills',
    icon: Sparkles,
    badge: store.skills.length,
    badgeClass: 'bg-gold-500/10 text-gold-400 border border-gold-500/30',
  },
  {
    path: '/constitution',
    label: 'Constitution',
    icon: BookOpen,
  },
  {
    path: '/replay',
    label: 'Session Replay',
    icon: History,
    badge: store.stats.total_sessions,
    badgeClass: 'bg-cyan-500/10 text-cyan-400 border border-cyan-500/30',
  },
  {
    path: '/traces',
    label: 'Success Traces',
    icon: Footprints,
    badge: store.stats.active_traces,
    badgeClass: 'bg-emerald-500/10 text-emerald-400 border border-emerald-500/30',
  },
  {
    path: '/failures',
    label: 'Failures & Gotchas',
    icon: AlertTriangle,
    badge: store.stats.unresolved_failures,
    badgeClass: store.stats.unresolved_failures > 0 ? 'bg-red-500/20 text-red-400 border border-red-500/50 animate-pulse' : 'bg-dark-800 text-gray-400',
  },
  {
    path: '/proposals',
    label: 'Review Proposals',
    icon: GitPullRequestDraft,
    badge: store.stats.pending_proposals,
    badgeClass: store.stats.pending_proposals > 0 ? 'bg-amber-500/20 text-amber-400 border border-amber-500/50' : 'bg-dark-800 text-gray-400',
  },
  {
    path: '/guardrails',
    label: 'Safety Guardrails',
    icon: ShieldAlert,
  },
]);
</script>
