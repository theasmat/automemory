<template>
  <div class="space-y-6">
    <PageHeader title="Activity" description="What agents were stopped from doing, and what was changed from this dashboard." />

    <Tabs v-model="tab" :tabs="tabs" />

    <Card>
      <EmptyState
        v-if="shown.length === 0"
        title="Nothing here yet"
        description="Blocked commands, locked-file warnings and dashboard changes show up here as they happen."
      />
      <ul v-else class="divide-y divide-border/60">
        <li v-for="(a, i) in shown" :key="i" class="py-3 first:pt-0 last:pb-0 flex items-start justify-between gap-4">
          <div class="min-w-0">
            <p class="text-sm text-text">{{ describeActivity(a) }}</p>
            <p v-if="a.details?.reason" class="text-xs text-text-muted break-words">{{ a.details.reason }}</p>
          </div>
          <span class="text-xs text-text-muted flex-shrink-0">{{ ago(a.at) }}</span>
        </li>
      </ul>
    </Card>
    <p class="text-xs text-text-muted">Showing the latest 100 events.</p>
  </div>
</template>

<script setup lang="ts">
import { ref, computed, onMounted } from 'vue';
import { request } from '~/utils/api';
import { describeActivity, ago } from '~/utils/activity';

const rows = ref<any[]>([]);
const tab = ref('all');
const blocked = (a: any) => a.action === 'hook.deny' || a.action === 'hook.ask';
const tabs = computed(() => [
  { id: 'all', label: 'Everything', count: rows.value.length },
  { id: 'blocked', label: 'Blocked or questioned', count: rows.value.filter(blocked).length },
  { id: 'dashboard', label: 'Dashboard changes', count: rows.value.filter((a) => a.action === 'ui.action').length },
]);
const shown = computed(() =>
  tab.value === 'blocked' ? rows.value.filter(blocked) : tab.value === 'dashboard' ? rows.value.filter((a) => a.action === 'ui.action') : rows.value
);

onMounted(async () => {
  const r = await request<any[]>('/api/activity');
  if (r.ok && Array.isArray(r.data)) rows.value = r.data;
});
</script>
