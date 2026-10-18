<template>
  <div class="flex h-screen bg-bg text-text font-sans antialiased overflow-hidden">
    <!-- Skip to content for accessibility -->
    <a
      href="#main-content"
      class="sr-only focus:not-sr-only focus:absolute focus:top-3 focus:left-3 focus:z-50 focus:px-3 focus:py-1.5 focus:bg-accent focus:text-accent-fg focus:rounded-card focus:font-semibold"
    >
      Skip to content
    </a>

    <!-- Navigation Rail -->
    <NavRail />

    <!-- Main Workspace -->
    <div class="flex-1 flex flex-col min-w-0 h-full overflow-hidden">
      <!-- Top Chrome Bar -->
      <TopBar
        @open-palette="openPalette"
        @open-shortcuts="openShortcuts"
      />

      <!-- Main Content Region -->
      <main
        id="main-content"
        class="flex-1 overflow-y-auto px-4 py-6 sm:px-6 sm:py-8 pb-20 md:pb-8"
        role="main"
      >
        <div class="max-w-6xl mx-auto w-full">
          <slot />
        </div>
      </main>
    </div>

    <!-- Modals & Feedback Overlays -->
    <Toast />
    <PasswordModal />
    <CommandPalette ref="paletteRef" />
    <ShortcutHelp ref="shortcutsRef" />
  </div>
</template>

<script setup lang="ts">
import { ref } from 'vue';

const paletteRef = ref<{ open: () => void } | null>(null);
const shortcutsRef = ref<{ open: () => void } | null>(null);

function openPalette() {
  paletteRef.value?.open();
}

function openShortcuts() {
  shortcutsRef.value?.open();
}
</script>
