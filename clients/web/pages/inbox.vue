<template>
  <div class="space-y-6">
    <PageHeader
      data-tour="inbox-header"
      title="Inbox"
      description="Review suggestions submitted by agents before they become persistent project knowledge."
    >
      <template #badge>
        <Badge v-if="store.proposals.length > 0" variant="accent">
          {{ store.proposals.length }} pending
        </Badge>
        <Badge v-else variant="ok">
          Inbox zero
        </Badge>
      </template>

      <template #actions>
        <Button
          v-if="store.proposals.length > 0"
          size="sm"
          variant="primary"
          @click="store.decideAllProposals(true)"
        >
          Approve all ({{ store.proposals.length }})
        </Button>
      </template>
    </PageHeader>

    <ErrorState
      v-if="!store.isConnected && store.daemonError"
      title="AutoMemory isn't running"
      :message="store.daemonError"
      command="automemory serve"
      @retry="store.fetchProposals()"
    />

    <!-- Empty State -->
    <div v-if="store.proposals.length === 0" class="py-6">
      <EmptyState
        title="Inbox zero"
        description="Agents will suggest things here when they learn architectural patterns, gotchas, or conventions worth keeping."
      />
    </div>

    <!-- Master / Detail Review Pane -->
    <div v-else class="grid grid-cols-1 lg:grid-cols-3 gap-6 items-start">
      <!-- Left Column: List of Suggestions -->
      <div class="space-y-2 lg:col-span-1 max-h-[75vh] overflow-y-auto pr-1">
        <div
          v-for="(item, idx) in store.proposals"
          :key="item.id"
          class="p-3.5 rounded-card border text-left cursor-pointer transition-colors"
          :class="[
            selectedId === item.id
              ? 'bg-accent/10 border-accent text-text'
              : 'bg-surface border-border hover:bg-surface-2 text-text-muted hover:text-text'
          ]"
          @click="selectedIndex = idx"
        >
          <div class="flex items-center justify-between gap-2 mb-1.5">
            <Badge variant="accent">
              {{ typeLabel(item.proposal_type) }}
            </Badge>
            <span class="text-[11px] font-mono text-text-muted">
              {{ formatAge(item.created_at) }}
            </span>
          </div>

          <p class="text-xs sm:text-sm font-semibold text-text line-clamp-2">
            {{ statement(item) }}
          </p>

        </div>
      </div>

      <!-- Right Column: Selected Proposal Detail -->
      <div v-if="selectedProposal" class="lg:col-span-2 space-y-4">
        <Card>
          <div class="space-y-4">
            <!-- Header with Suggestion Source -->
            <div class="flex items-center justify-between border-b border-border pb-3">
              <div>
                <span class="text-xs text-text-muted">Suggestion from</span>
                <h3 class="text-base font-bold text-text">Agent Suggestion</h3>
              </div>
              <Badge v-if="typeof selectedProposal.confidence === 'number'" variant="accent">
                Confidence {{ Math.round(selectedProposal.confidence * 100) }}%
              </Badge>
              <Badge v-else variant="accent">{{ typeLabel(selectedProposal.proposal_type) }}</Badge>
            </div>

            <!-- Proposed Content -->
            <div>
              <span class="text-xs font-semibold text-text uppercase tracking-wider block mb-1">
                Suggested statement
              </span>
              <p class="text-base font-semibold text-text leading-snug">
                {{ statement(selectedProposal) }}
              </p>
              <div v-if="selectedProposal.proposed_memory?.content" class="mt-2 p-3 rounded-card bg-surface-2 border border-border text-xs sm:text-sm text-text whitespace-pre-wrap">
                {{ selectedProposal.proposed_memory.content }}
              </div>
            </div>

            <!-- Why Agent Thinks So -->
            <div class="p-3.5 rounded-card bg-surface-2/60 border border-border space-y-1.5">
              <span class="text-xs font-semibold text-accent block">
                Why the agent suggested this
              </span>
              <p class="text-xs text-text-muted leading-relaxed">
                {{ selectedProposal.rationale || 'An agent noticed this while working. Approve it to share it with every agent on this project.' }}
              </p>
            </div>

            <!-- Rejection Reason Input (Optional) -->
            <div v-if="isRejecting" class="p-3 rounded-card bg-bad/5 border border-bad/20 space-y-2">
              <label class="block text-xs font-medium text-bad">
                Rejection reason (helps train future suggestions):
              </label>
              <input
                v-model="rejectionReason"
                type="text"
                class="w-full bg-surface border border-border rounded-card px-3 py-1.5 text-xs text-text focus:outline-none"
                placeholder="e.g. Outdated pattern, violates current policy"
                @keydown.enter="confirmReject"
              />
              <div class="flex justify-end gap-2">
                <Button size="sm" variant="ghost" @click="isRejecting = false">
                  Cancel
                </Button>
                <Button size="sm" variant="danger" @click="confirmReject">
                  Confirm Rejection
                </Button>
              </div>
            </div>

            <!-- Action Buttons -->
            <div v-else class="flex items-center justify-between pt-2 border-t border-border">
              <span class="text-xs text-text-muted font-mono hidden sm:inline">
                Keyboard: [A] approve, [R] reject, [J/K] navigate
              </span>

              <div class="flex items-center gap-2 ml-auto">
                <Button
                  size="sm"
                  variant="danger"
                  @click="isRejecting = true"
                >
                  Reject [R]
                </Button>
                <Button
                  size="sm"
                  variant="primary"
                  @click="confirmApprove"
                >
                  Approve [A] &rarr;
                </Button>
              </div>
            </div>
          </div>
        </Card>
      </div>
    </div>
  </div>
</template>

<script setup lang="ts">
/** The sentence to show for a suggestion (the API gives `description`; older shapes carry the memory). */
function statement(p: any): string {
  return p?.proposed_memory?.summary || p?.description || p?.content_diff || 'New suggestion';
}

/** "pattern_promotion" -> "Pattern" */
function typeLabel(t: string): string {
  const word = (t || 'suggestion').split('_')[0];
  return word.charAt(0).toUpperCase() + word.slice(1);
}

import { ref, computed, onMounted, onUnmounted } from 'vue';
import { useAutoMemoryStore } from '~/stores/automemory';

const store = useAutoMemoryStore();
const selectedIndex = ref(0);
const isRejecting = ref(false);
const rejectionReason = ref('');

const selectedProposal = computed(() => {
  return store.proposals[selectedIndex.value] || null;
});

const selectedId = computed(() => {
  return selectedProposal.value?.id || null;
});

function formatAge(dateStr?: string): string {
  if (!dateStr) return '';
  const diffSec = Math.floor((Date.now() - new Date(dateStr).getTime()) / 1000);
  if (diffSec < 3600) return `${Math.floor(diffSec / 60)}m ago`;
  if (diffSec < 86400) return `${Math.floor(diffSec / 3600)}h ago`;
  return `${Math.floor(diffSec / 86400)}d ago`;
}

async function confirmApprove() {
  if (!selectedProposal.value) return;
  await store.decideProposal(selectedProposal.value.id, true);
  if (selectedIndex.value >= store.proposals.length) {
    selectedIndex.value = Math.max(0, store.proposals.length - 1);
  }
}

async function confirmReject() {
  if (!selectedProposal.value) return;
  await store.decideProposal(selectedProposal.value.id, false, rejectionReason.value);
  rejectionReason.value = '';
  isRejecting.value = false;
  if (selectedIndex.value >= store.proposals.length) {
    selectedIndex.value = Math.max(0, store.proposals.length - 1);
  }
}

function handleKeydown(e: KeyboardEvent) {
  const target = e.target as HTMLElement;
  if (['input', 'textarea', 'select'].includes(target.tagName.toLowerCase())) return;

  if (e.key === 'j' || e.key === 'ArrowDown') {
    if (selectedIndex.value < store.proposals.length - 1) {
      selectedIndex.value++;
    }
  } else if (e.key === 'k' || e.key === 'ArrowUp') {
    if (selectedIndex.value > 0) {
      selectedIndex.value--;
    }
  } else if (e.key === 'a' || e.key === 'A') {
    confirmApprove();
  } else if (e.key === 'r' || e.key === 'R') {
    isRejecting.value = true;
  }
}

onMounted(() => {
  window.addEventListener('keydown', handleKeydown);
});

onUnmounted(() => {
  window.removeEventListener('keydown', handleKeydown);
});
</script>
