<template>
  <div class="space-y-6">
    <PageHeader
      title="Safety"
      description="Choose how careful agents must be, and what is blocked."
    >
      <template #badge>
        <Badge :variant="store.currentMode === 'strict' ? 'bad' : store.currentMode === 'review' ? 'accent' : 'ok'">
          Mode: {{ formatMode(store.currentMode) }}
        </Badge>
      </template>
    </PageHeader>

    <ErrorState
      v-if="!store.isConnected && store.daemonError"
      title="AutoMemory isn't running"
      :message="store.daemonError"
      command="automemory serve"
      @retry="store.fetchGuardrailRules()"
    />

    <!-- Mode Selector Cards -->
    <div class="space-y-3">
      <h2 class="text-lg font-semibold text-text">How careful should agents be?</h2>
      <div data-tour="safety-modes" class="grid grid-cols-1 md:grid-cols-3 gap-4">
        <button
          v-for="m in modes"
          :key="m.id"
          type="button"
          class="p-5 rounded-card text-left transition-all shadow-card"
          :class="[
            store.currentMode === m.id
              ? 'bg-accent/10 ring-2 ring-accent'
              : 'bg-surface border border-border/70 hover:bg-surface-2'
          ]"
          @click="store.setMode(m.id)"
        >
          <div class="flex items-center justify-between gap-2 mb-2">
            <span class="text-[17px] font-semibold text-text">{{ m.title }}</span>
            <span v-if="store.currentMode === m.id" class="w-2 h-2 rounded-full bg-accent" />
          </div>
          <p class="text-xs text-text-muted leading-relaxed">{{ m.description }}</p>
        </button>
      </div>
    </div>

    <!-- Password Management Card -->
    <Card title="Password">
      <div class="space-y-3 mt-1">
        <p class="text-sm text-text-muted leading-relaxed">
          A password keeps agents from changing safety settings or approving their own suggestions. Only you can.
        </p>
        <div class="flex items-center gap-3">
          <CopyCommand command="automemory rule passwd" label="To set or change your password via terminal:" />
        </div>
      </div>
    </Card>

    <!-- Test a Command Sandbox -->
    <Card title="Try a command">
      <form class="space-y-3 mt-1" @submit.prevent="testCommand">
        <div class="flex gap-2">
          <input
            v-model="commandToTest"
            type="text"
            required
            class="flex-1 bg-surface-2 border border-transparent rounded-xl px-3.5 py-2 text-sm text-text font-mono focus:border-accent focus:outline-none"
            placeholder="e.g. git push --force origin main"
          />
          <Button size="sm" variant="secondary" type="submit" :loading="isTesting">
            Evaluate
          </Button>
        </div>

        <div v-if="testResult" class="p-3 rounded-card border text-sm" :class="testResult.decision === 'deny' ? 'bg-bad/10 border-bad/30 text-bad' : 'bg-ok/10 border-ok/30 text-ok'">
          <div class="font-semibold flex items-center gap-1.5">
            <span>Verdict:</span>
            <span class="">{{ testResult.decision }}</span>
          </div>
          <p v-if="testResult.reason" class="text-xs text-text-muted mt-1">
            {{ testResult.reason }}
          </p>
        </div>
      </form>
    </Card>

    <!-- Guardrail Rules Table -->
    <Card title="Blocking rules">
      <template #actions>
        <Button size="sm" variant="secondary" @click="isAddRuleOpen = true">
          + Add rule
        </Button>
      </template>

      <div class="overflow-x-auto mt-2">
        <table class="w-full text-left text-sm">
          <thead>
            <tr class="border-b border-border text-text-muted text-xs ">
              <th class="py-2 px-3">Rule Name</th>
              <th class="py-2 px-3">Pattern</th>
              <th class="py-2 px-3">Severity</th>
              <th class="py-2 px-3">Status</th>
              <th class="py-2 px-3 text-right">Actions</th>
            </tr>
          </thead>
          <tbody class="divide-y divide-border/60">
            <tr v-for="rule in store.guardrailRules" :key="rule.id" class="hover:bg-surface-2/40">
              <td class="py-3 px-3 font-semibold text-text">
                {{ rule.name }}
                <div v-if="rule.description" class="text-xs text-text-muted font-normal mt-0.5">
                  {{ rule.description }}
                </div>
              </td>
              <td class="py-3 px-3 font-mono text-xs text-accent">
                {{ rule.pattern }}
              </td>
              <td class="py-3 px-3">
                <Badge :variant="rule.is_critical ? 'bad' : 'warn'">
                  {{ rule.is_critical ? 'Critical' : 'High-risk' }}
                </Badge>
              </td>
              <td class="py-3 px-3">
                <button
                  type="button"
                  class="text-xs font-semibold hover:underline"
                  :class="rule.enabled ? 'text-ok' : 'text-text-muted'"
                  @click="store.toggleGuardrailRule(rule.id, !rule.enabled)"
                >
                  {{ rule.enabled ? 'Enabled' : 'Disabled' }}
                </button>
              </td>
              <td class="py-3 px-3 text-right">
                <Button size="sm" variant="danger" @click="store.deleteGuardrailRule(rule.id)">
                  Delete
                </Button>
              </td>
            </tr>
          </tbody>
        </table>
      </div>
    </Card>

    <!-- Add Rule Modal -->
    <Modal
      :is-open="isAddRuleOpen"
      title="Add Custom Guardrail Rule"
      @close="isAddRuleOpen = false"
    >
      <form class="space-y-4" @submit.prevent="submitAddRule">
        <div>
          <label class="block text-xs font-semibold text-text mb-1">Rule Name</label>
          <input
            v-model="newRuleName"
            type="text"
            required
            class="w-full bg-surface-2 border border-transparent rounded-xl px-3.5 py-2 text-sm text-text focus:outline-none"
            placeholder="e.g. Block force pushes"
          />
        </div>

        <div>
          <label class="block text-xs font-semibold text-text mb-1">Regex Pattern</label>
          <input
            v-model="newRulePattern"
            type="text"
            required
            class="w-full bg-surface-2 border border-transparent rounded-xl px-3.5 py-2 text-sm font-mono text-text focus:outline-none"
            placeholder="e.g. git\s+push.*(--force|-f)"
          />
        </div>

        <div>
          <label class="block text-xs font-semibold text-text mb-1">Description</label>
          <input
            v-model="newRuleDesc"
            type="text"
            class="w-full bg-surface-2 border border-transparent rounded-xl px-3.5 py-2 text-sm text-text focus:outline-none"
            placeholder="Why this is prohibited"
          />
        </div>

        <div class="flex justify-end gap-2 pt-2">
          <Button variant="ghost" size="sm" @click="isAddRuleOpen = false">
            Cancel
          </Button>
          <Button variant="primary" size="sm" type="submit">
            Add rule
          </Button>
        </div>
      </form>
    </Modal>
  </div>
</template>

<script setup lang="ts">
import { ref } from 'vue';
import { useAutoMemoryStore } from '~/stores/automemory';

const store = useAutoMemoryStore();

const modes = [
  {
    id: 'vibe' as const,
    title: 'Relaxed',
    description: 'Agents are warned about dangerous operations, but nothing is blocked. Great for local exploration.',
  },
  {
    id: 'review' as const,
    title: 'Balanced (Default)',
    description: 'Risky destructive commands and clashing concurrent file edits are blocked. Safe for daily work.',
  },
  {
    id: 'strict' as const,
    title: 'Strict',
    description: 'Every edit requires an advisory file lock; unlisted commands are blocked. Zero risk of collision.',
  },
];

const commandToTest = ref('git push --force origin main');
const testResult = ref<any>(null);
const isTesting = ref(false);

const isAddRuleOpen = ref(false);
const newRuleName = ref('');
const newRulePattern = ref('');
const newRuleDesc = ref('');

function formatMode(mode: string): string {
  if (mode === 'strict') return 'Strict';
  if (mode === 'vibe') return 'Relaxed';
  return 'Balanced';
}

async function testCommand() {
  if (!commandToTest.value) return;
  isTesting.value = true;
  try {
    testResult.value = await store.evaluateGuardrail('run_command', {
      command: commandToTest.value,
    });
  } finally {
    isTesting.value = false;
  }
}

async function submitAddRule() {
  if (!newRuleName.value || !newRulePattern.value) return;
  const ok = await store.addGuardrailRule({
    name: newRuleName.value,
    pattern: newRulePattern.value,
    description: newRuleDesc.value,
    is_critical: true,
  });
  if (ok) {
    newRuleName.value = '';
    newRulePattern.value = '';
    newRuleDesc.value = '';
    isAddRuleOpen.value = false;
  }
}
</script>
