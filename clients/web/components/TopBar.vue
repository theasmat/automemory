<template>
  <header class="h-14 bg-bg/80 backdrop-blur-md border-b border-border/60 px-3 sm:px-6 flex items-center justify-between gap-2 sm:gap-4 flex-shrink-0 z-20">
    <div class="flex items-center gap-3 min-w-0 flex-1 sm:flex-none">
      <!-- Project Picker (takes the free width on a phone and truncates) -->
      <div class="relative flex items-center min-w-0 flex-1 sm:flex-none max-w-[14rem] sm:max-w-none">
        <label for="project-select" class="sr-only">Active Project</label>
        <select
          id="project-select"
          v-model="store.activeProject"
          class="w-full min-w-0 truncate bg-surface-2 border border-transparent text-sm font-semibold rounded-pill px-3 min-h-10 sm:min-h-0 sm:py-1.5 text-text pr-7 focus:border-accent focus:outline-none appearance-none cursor-pointer"
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
    <div class="flex items-center gap-1.5 sm:gap-3 flex-shrink-0">
      <!-- Search / Command Palette Trigger (Mobile Icon + Desktop Pill) -->
      <button
        type="button"
        class="sm:hidden flex items-center justify-center w-10 h-10 rounded-pill bg-surface-2 hover:bg-border/70 text-text-muted hover:text-text transition-colors"
        aria-label="Search or command (⌘K)"
        title="Search or command (⌘K)"
        @click="$emit('open-palette')"
      >
        <svg class="w-4 h-4" fill="none" viewBox="0 0 24 24" stroke="currentColor" aria-hidden="true">
          <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M21 21l-6-6m2-5a7 7 0 11-14 0 7 7 0 0114 0z" />
        </svg>
      </button>
      <button
        type="button"
        data-tour="search"
        class="hidden sm:flex items-center gap-2 bg-surface-2 hover:bg-border/70 rounded-pill px-3.5 py-1.5 text-xs text-text-muted transition-colors"
        @click="$emit('open-palette')"
      >
        <svg class="w-3.5 h-3.5" fill="none" viewBox="0 0 24 24" stroke="currentColor" aria-hidden="true">
          <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M21 21l-6-6m2-5a7 7 0 11-14 0 7 7 0 0114 0z" />
        </svg>
        <span>Search or command</span>
        <kbd class="font-mono text-xs bg-surface px-1.5 py-0.5 rounded border border-border">⌘K</kbd>
      </button>

      <JobsTray />

      <!-- Connection Pill -->
      <div
        class="inline-flex items-center justify-center gap-1.5 min-w-6 h-6 px-1.5 sm:px-2.5 rounded-pill text-xs font-medium"
        role="status"
        :aria-label="store.isConnected ? 'Daemon connected' : 'Connecting to daemon'"
        :class="store.isConnected ? 'bg-ok-bg text-ok' : 'bg-warn-bg text-warn'"
        :title="store.isConnected ? 'Daemon WebSocket connected' : 'Connecting to daemon...'"
      >
        <span class="w-1.5 h-1.5 rounded-full" :class="store.isConnected ? 'bg-ok' : 'bg-warn animate-pulse'" />
        <span class="hidden md:inline">{{ store.isConnected ? 'Connected' : 'Connecting...' }}</span>
      </div>

      <!-- Theme Toggle -->
      <button
        type="button"
        class="flex items-center justify-center gap-1.5 w-10 h-10 sm:w-auto sm:h-auto sm:px-2.5 sm:py-1 rounded-pill bg-surface-2 text-xs text-text hover:bg-border/70 transition-colors cursor-pointer select-none"
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
        <span class="hidden md:inline font-medium text-xs">{{ themeLabel }}</span>
      </button>

      <!-- Help & guided learning drawer -->
      <IconButton
        class="hidden sm:inline-flex"
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
