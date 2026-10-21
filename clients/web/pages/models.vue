<template>
  <div class="space-y-6 max-w-3xl">
    <PageHeader title="Models" description="The two optional models: one that helps search by meaning, one that suggests notes. Both are off until you turn them on." />

    <Tabs v-model="tab" :tabs="tabs" />

    <ModelsSearchModelPanel v-if="tab === 'search'" />
    <ModelsAiHelperPanel v-else />
  </div>
</template>

<script setup lang="ts">
import { computed } from 'vue';

const route = useRoute();
const router = useRouter();
const tabs = [
  { id: 'search', label: 'Search model' },
  { id: 'helper', label: 'AI helper' },
];
const tab = computed({
  get: () => (route.query.tab === 'helper' ? 'helper' : 'search'),
  set: (v: string) => router.replace({ path: '/models', query: { tab: v } }),
});
</script>
