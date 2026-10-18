<template>
  <AppShell>
    <NuxtPage />
  </AppShell>
</template>

<script setup lang="ts">
import { onMounted } from 'vue';
import { useAutoMemoryStore } from '~/stores/automemory';
import { useTheme } from '~/composables/useTheme';

const store = useAutoMemoryStore();
const { initTheme } = useTheme();

onMounted(() => {
  initTheme();
  store.initWebSocket();
  store.fetchAll();
});
</script>
