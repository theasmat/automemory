<template>
  <header class="h-14 bg-bg border-b border-border px-4 sm:px-6 flex items-center justify-between gap-4 flex-shrink-0 z-20">
    <div class="flex items-center gap-3 min-w-0">
      <!-- Project Picker -->
      <div class="relative flex items-center">
        <label for="project-select" class="sr-only">Active Project</label>
        <select
          id="project-select"
          v-model="store.activeProject"
          class="bg-surface-2 border border-border text-xs sm:text-sm font-semibold rounded-card px-2.5 py-1 text-text pr-7 focus:border-accent focus:outline-none appearance-none cursor-pointer"
          @change="onProjectChange"
        >
          <option :value="null">All Projects</option>
          <option v-for="p in store.projects" :key="p.id" :value="p.id">
            {{ p.name || p.slug || p.id }}
          </option>
        </select>
        <div class="absolute right-2 pointer-events-none text-text-muted text-xs">▾</div>
      </div>

      <!-- Mode indicator pill -->
      <NuxtLink
        to="/safety"
        data-tour="safety-mode"
        class="hidden sm:inline-flex"
        title="Click to change safety mode"
      >
        <Badge :variant="store.currentMode === 'strict' ? 'bad' : store.currentMode === 'review' ? 'accent' : 'ok'">
          {{ store.currentMode === 'strict' ? 'Strict' : store.currentMode === 'review' ? 'Balanced' : 'Relaxed' }}
        </Badge>
      </NuxtLink>
    </div>

    <!-- Actions & Status -->
    <div class="flex items-center gap-2 sm:gap-3 flex-shrink-0">
      <!-- Search / Command Palette Trigger -->
      <button
        type="button"
        class="hidden sm:flex items-center gap-2 bg-surface-2 hover:bg-surface border border-border rounded-card px-3 py-1 text-xs text-text-muted transition-colors hover:border-text-muted/40"
        @click="$emit('open-palette')"
      >
        <svg class="w-3.5 h-3.5" fill="none" viewBox="0 0 24 24" stroke="currentColor">
          <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M21 21l-6-6m2-5a7 7 0 11-14 0 7 7 0 0114 0z" />
        </svg>
        <span>Search or command</span>
        <kbd class="font-mono text-[10px] bg-surface px-1 py-0.5 rounded border border-border">⌘K</kbd>
      </button>

      <!-- Connection Pill -->
      <div
        class="inline-flex items-center gap-1.5 px-2.5 py-0.5 rounded-pill text-xs font-medium border"
        :class="store.isConnected ? 'bg-ok-bg text-ok border-ok/30' : 'bg-warn-bg text-warn border-warn/30'"
        :title="store.isConnected ? 'Daemon WebSocket connected' : 'Connecting to daemon...'"
      >
        <span class="w-1.5 h-1.5 rounded-full" :class="store.isConnected ? 'bg-ok' : 'bg-warn animate-pulse'" />
        <span class="hidden md:inline">{{ store.isConnected ? 'Connected' : 'Connecting...' }}</span>
      </div>

      <!-- Theme Toggle -->
      <button
        type="button"
        class="flex items-center gap-1.5 px-2.5 py-1 rounded-card border border-border bg-surface-2/60 text-xs text-text hover:text-accent hover:border-accent transition-colors cursor-pointer select-none"
        :aria-label="'Theme: ' + themeLabel"
        :title="'Theme: ' + themeLabel + ' (click to cycle System / Light / Dark)'"
        @click="cycleTheme"
      >
        <!-- System icon -->
        <svg v-if="themeMode === 'system'" class="w-3.5 h-3.5 text-accent" fill="none" viewBox="0 0 24 24" stroke="currentColor">
          <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M9.75 17L9 20l-1 1h8l-1-1-.75-3M3 13h18M5 17h14a2 2 0 002-2V5a2 2 0 00-2-2H5a2 2 0 00-2 2v10a2 2 0 002 2z" />
        </svg>
        <!-- Dark icon -->
        <svg v-else-if="themeMode === 'dark'" class="w-3.5 h-3.5 text-accent" fill="none" viewBox="0 0 24 24" stroke="currentColor">
          <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M20.354 15.354A9 9 0 018.646 3.646 9.003 9.003 0 0012 21a9.003 9.003 0 008.354-5.646z" />
        </svg>
        <!-- Light icon -->
        <svg v-else class="w-3.5 h-3.5 text-accent" fill="none" viewBox="0 0 24 24" stroke="currentColor">
          <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M12 3v1m0 16v1m9-9h-1M4 12H3m15.364 6.364l-.707-.707M6.343 6.343l-.707-.707m12.728 0l-.707.707M6.343 17.657l-.707.707M16 12a4 4 0 11-8 0 4 4 0 018 0z" />
        </svg>
        <span class="font-medium text-xs">{{ themeLabel }}</span>
      </button>

      <!-- Help & guided learning drawer -->
      <IconButton
        aria-label="Help & Guided Learning"
        title="Help & Guided Learning (?)"
        size="sm"
        @click="store.openHelpDrawer()"
      >
        <span class="font-mono text-xs font-semibold">?</span>
      </IconButton>
    </div>
  </header>
</template>

<script setup lang="ts">
import { useAutoMemoryStore } from '~/stores/automemory';
import { useTheme } from '~/composables/useTheme';

const store = useAutoMemoryStore();
const { themeMode, themeLabel, cycleTheme } = useTheme();

defineEmits<{
  (e: 'open-palette'): void;
  (e: 'open-shortcuts'): void;
}>();

function onProjectChange() {
  store.fetchAll();
}
</script>
