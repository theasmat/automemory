<template>
  <div class="space-y-6">
    <PageHeader title="Activity" description="What agents did and learned, what was blocked, and what was changed from this dashboard." />

    <Tabs v-model="tab" :tabs="tabs" />

    <ActivityBlockedPanel v-if="tab === 'blocked'" />
    <ActivityReplayPanel v-else-if="tab === 'sessions'" />
    <ActivityTracesPanel v-else-if="tab === 'traces'" />
    <ActivityFailuresPanel v-else-if="tab === 'problems'" />
    <ActivitySkillsPanel v-else-if="tab === 'skills'" />
  </div>
</template>

<script setup lang="ts">
import { computed } from 'vue';

// The tab lives in the address (?tab=skills) so a link opens it and the back button works.
const route = useRoute();
const router = useRouter();
const tabs = [
  { id: 'blocked', label: 'Blocked & changes' },
  { id: 'sessions', label: 'Sessions' },
  { id: 'traces', label: 'What worked' },
  { id: 'problems', label: 'Problems & fixes' },
  { id: 'skills', label: 'Learned skills' },
];
const tab = computed({
  get: () => (tabs.some((t) => t.id === route.query.tab) ? String(route.query.tab) : 'blocked'),
  set: (v: string) => router.replace({ path: '/activity', query: { tab: v } }),
});
</script>
