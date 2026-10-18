<template>
  <div class="space-y-6">
    <PageHeader
      title="Rules of the project"
      description="The standing rules every agent follows in this project."
    >
      <template #actions>
        <Button variant="secondary" size="sm" @click="store.fetchConstitution">
          Refresh
        </Button>
      </template>
    </PageHeader>

    <EmptyState
      v-if="!store.constitution"
      title="No project rules yet"
      description="As you record decisions, AutoMemory turns the lasting ones into rules every agent follows."
    >
      <template #action>
        <NuxtLink to="/brain"><Button variant="primary" size="sm">Open Project brain</Button></NuxtLink>
      </template>
    </EmptyState>

    <Card v-else>
      <p class="text-sm text-text-muted mb-4">Every agent in this project is shown these rules when it starts.</p>
      <Prose :text="store.constitution" />
    </Card>
  </div>
</template>

<script setup lang="ts">
import { onMounted } from 'vue';
import { useAutoMemoryStore } from '~/stores/automemory';

const store = useAutoMemoryStore();

onMounted(() => {
  store.fetchConstitution();
});
</script>
