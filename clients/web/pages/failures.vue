<template>
  <div class="space-y-6">
    <PageHeader
      title="Problems & fixes"
      description="What went wrong, and how it was fixed."
    >
      <template #badge>
        <Badge :variant="store.stats.unresolved_failures > 0 ? 'bad' : 'neutral'">
          {{ store.stats.unresolved_failures }} unresolved
        </Badge>
      </template>
      <template #actions>
        <Button variant="secondary" size="sm" @click="store.fetchFailures">
          Refresh
        </Button>
      </template>
    </PageHeader>

    <!-- Filter Tabs -->
    <div class="flex items-center gap-2 border-b border-border pb-3">
      <button
        v-for="s in [
          { key: 'unresolved', label: 'Unresolved' },
          { key: 'resolved', label: 'Resolved' },
        ]"
        :key="s.key"
        type="button"
        class="px-3 py-1.5 rounded-card text-xs font-medium transition-colors"
        :class="activeStatus === s.key ? 'bg-surface-2 text-text font-semibold border border-border shadow-sm' : 'text-text-muted hover:text-text'"
        @click="activeStatus = s.key"
      >
        {{ s.label }}
      </button>
    </div>

    <!-- Empty State -->
    <div v-if="filteredFailures.length === 0" class="py-8">
      <Card>
        <EmptyState
          :title="`No ${activeStatus} problems found`"
          description="When an agent encounters a broken tool or unexpected compilation error, failure.save records it so other agents avoid repeating it."
        />
      </Card>
    </div>

    <!-- Failures List -->
    <div v-else class="space-y-4">
      <Card
        v-for="f in filteredFailures"
        :key="f.id"
      >
        <div class="space-y-4">
          <div class="flex items-center justify-between flex-wrap gap-2">
            <div class="flex items-center gap-2">
              <Badge :variant="f.status === 'unresolved' ? 'bad' : 'ok'">
                {{ f.status }}
              </Badge>
              <h3 class="text-sm font-semibold text-text">{{ f.task_title }}</h3>
            </div>
            <div class="flex items-center gap-2 text-xs">
              <Button
                v-if="f.status === 'unresolved'"
                variant="primary"
                size="sm"
                @click="openResolveModal(f)"
              >
                Mark resolved
              </Button>
              <Badge variant="neutral">
                ~{{ estimateTokens(f.error_message + ' ' + (f.root_cause_hypothesis || '') + ' ' + (f.fix_applied || '')) }} tokens
              </Badge>
              <span class="text-text-muted">{{ formatTime(f.created_at) }}</span>
            </div>
          </div>

          <!-- Error Message -->
          <div class="space-y-1">
            <div class="text-xs font-semibold text-text flex items-center justify-between">
              <span>Error message / stack</span>
              <span class="text-xs text-text-muted">~{{ estimateTokens(f.error_message) }} tokens</span>
            </div>
            <pre class="p-3 bg-surface-2 rounded-card border border-bad/30 text-xs text-text font-mono overflow-x-auto whitespace-pre-wrap">{{ f.error_message }}</pre>
          </div>

          <!-- Root Cause Hypothesis -->
          <div v-if="f.root_cause_hypothesis" class="space-y-1">
            <div class="text-xs font-semibold text-text">Root cause hypothesis</div>
            <p class="p-3 bg-surface-2 rounded-card border border-border text-xs text-text-muted leading-relaxed">
              {{ f.root_cause_hypothesis }}
            </p>
          </div>

          <!-- Fix Applied (if resolved) -->
          <div v-if="f.fix_applied" class="space-y-1">
            <div class="text-xs font-semibold text-ok flex items-center justify-between">
              <span>Fix applied & verification</span>
              <span class="text-xs text-text-muted font-normal">~{{ estimateTokens(f.fix_applied) }} tokens</span>
            </div>
            <div class="p-3 bg-surface-2 rounded-card border border-ok/30 text-xs space-y-1.5">
              <div class="text-text">{{ f.fix_applied }}</div>
              <div v-if="f.verification_result" class="text-ok text-xs pt-1 border-t border-border">
                ✓ {{ f.verification_result }}
              </div>
            </div>
          </div>
        </div>
      </Card>
    </div>

    <!-- Resolve Failure Modal -->
    <Modal
      :is-open="!!resolvingFailure"
      title="Mark problem as resolved"
      @close="resolvingFailure = null"
    >
      <div v-if="resolvingFailure" class="space-y-4">
        <div class="text-xs text-text-muted">
          Task: <span class="font-semibold text-text">{{ resolvingFailure.task_title }}</span>
        </div>

        <div class="space-y-3">
          <div class="space-y-1">
            <label class="block text-xs font-medium text-text">Fix applied / remediation</label>
            <textarea
              v-model="fixAppliedText"
              rows="3"
              placeholder="Describe the code change, flag update, or environment fix..."
              class="w-full bg-surface-2 border border-border rounded-card p-2 text-xs text-text focus:outline-none focus:ring-1 focus:ring-accent"
            />
          </div>

          <div class="space-y-1">
            <label class="block text-xs font-medium text-text">Verification proof (optional)</label>
            <input
              v-model="verificationText"
              type="text"
              placeholder="e.g. cargo test passed 100% / HTTP 200 OK"
              class="w-full bg-surface-2 border border-border rounded-card p-2 text-xs text-text focus:outline-none focus:ring-1 focus:ring-accent"
            />
          </div>
        </div>

        <div class="flex items-center justify-end gap-2 pt-2">
          <Button variant="ghost" size="sm" @click="resolvingFailure = null">
            Cancel
          </Button>
          <Button
            variant="primary"
            size="sm"
            :disabled="!fixAppliedText.trim()"
            @click="submitResolve"
          >
            Save resolution
          </Button>
        </div>
      </div>
    </Modal>
  </div>
</template>

<script setup lang="ts">
import { ref, computed } from 'vue';
import { useAutoMemoryStore } from '~/stores/automemory';
import { estimateTokens } from '~/utils/tokens';
import type { FailureRecord } from '~/types';

const store = useAutoMemoryStore();
const activeStatus = ref('unresolved');

const resolvingFailure = ref<FailureRecord | null>(null);
const fixAppliedText = ref('');
const verificationText = ref('');

const openResolveModal = (f: FailureRecord) => {
  resolvingFailure.value = f;
  fixAppliedText.value = '';
  verificationText.value = 'Verified in test environment';
};

const submitResolve = async () => {
  if (!resolvingFailure.value || !fixAppliedText.value.trim()) return;
  const ok = await store.resolveFailure(
    resolvingFailure.value.id,
    fixAppliedText.value.trim(),
    verificationText.value.trim() || undefined
  );
  if (ok) {
    resolvingFailure.value = null;
  }
};

const filteredFailures = computed(() => {
  return store.failures.filter((f) => f.status === activeStatus.value);
});

const formatTime = (iso: string) => {
  try {
    const d = new Date(iso);
    return d.toLocaleDateString(undefined, { month: 'short', day: 'numeric', hour: '2-digit', minute: '2-digit' });
  } catch {
    return iso;
  }
};
</script>
