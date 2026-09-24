<template>
  <div class="flex h-screen bg-black text-gray-200 overflow-hidden font-sans">
    <!-- Persistent Navigation Sidebar -->
    <LayoutSidebar />

    <!-- Main Workspace Area -->
    <div class="flex-1 flex flex-col min-w-0 h-full overflow-hidden">
      <!-- Live Status & Project Bar -->
      <LayoutStatusBar />

      <!-- Active Page Route (Stateful Nuxt Pages) -->
      <main class="flex-1 overflow-y-auto bg-black relative">
        <NuxtPage />
      </main>
    </div>

    <!-- Live WebSocket Notification Toast -->
    <div
      v-if="store.liveToast"
      class="fixed bottom-6 right-6 z-50 p-4 border bg-dark-900/95 backdrop-blur-md shadow-2xl flex items-center gap-3 transition-all duration-300 max-w-md font-mono text-xs"
      :class="[
        store.liveToast.type === 'success'
          ? 'border-emerald-500/50 text-emerald-300'
          : store.liveToast.type === 'warning'
          ? 'border-amber-500/50 text-amber-300'
          : 'border-gold-500/50 text-gold-300'
      ]"
    >
      <span class="w-2 h-2 rounded-full animate-ping" :class="store.liveToast.type === 'success' ? 'bg-emerald-400' : 'bg-gold-400'"></span>
      <span class="leading-relaxed">{{ store.liveToast.message }}</span>
    </div>
  </div>
</template>

<script setup lang="ts">
import { onMounted } from 'vue';
import { useAutoMemoryStore } from '~/stores/automemory';

const store = useAutoMemoryStore();

onMounted(() => {
  store.initWebSocket();
  store.fetchAll();
});
</script>
