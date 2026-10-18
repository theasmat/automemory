<template>
  <div class="w-full">
    <div v-if="label" class="text-xs font-medium text-text-muted mb-1">{{ label }}</div>
    <div class="flex items-center justify-between gap-2 bg-surface-2 rounded-xl px-3 py-2 text-[13px] font-mono">
      <code class="text-text select-all overflow-x-auto whitespace-nowrap">{{ command }}</code>
      <div class="flex items-center gap-1.5 flex-shrink-0">
        <button
          v-if="isSafeCommand"
          type="button"
          class="text-xs font-sans font-medium text-accent hover:underline px-2 py-0.5 rounded bg-accent/10 border border-accent/20 transition-colors flex items-center gap-1"
          :disabled="isRunning"
          title="Run this action directly in the dashboard"
          @click="runCommand"
        >
          <span v-if="ran" class="text-ok">✓ Done</span>
          <span v-else-if="isRunning" class="animate-pulse">Running...</span>
          <span v-else>Run for me</span>
        </button>

        <button
          type="button"
          class="text-text-muted hover:text-accent p-1 transition-colors flex-shrink-0"
          :title="copied ? 'Copied!' : 'Copy to clipboard'"
          :aria-label="copied ? 'Copied' : 'Copy command'"
          @click="copy"
        >
          <span v-if="copied" class="text-ok text-xs font-sans font-medium">Copied!</span>
          <svg v-else class="w-3.5 h-3.5" fill="none" viewBox="0 0 24 24" stroke="currentColor">
            <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M8 16H6a2 2 0 01-2-2V6a2 2 0 012-2h8a2 2 0 012 2v2m-6 12h8a2 2 0 002-2v-8a2 2 0 00-2-2h-8a2 2 0 00-2 2v8a2 2 0 002 2z" />
          </svg>
        </button>
      </div>
    </div>
  </div>
</template>

<script setup lang="ts">
import { ref, computed } from 'vue';
import { useAutoMemoryStore } from '~/stores/automemory';
import { useToast } from '~/composables/useToast';

const props = defineProps<{
  command: string;
  label?: string;
  allowRun?: boolean;
}>();

const store = useAutoMemoryStore();
const { success, error } = useToast();
const copied = ref(false);
const isRunning = ref(false);
const ran = ref(false);

const isSafeCommand = computed(() => {
  if (props.allowRun !== undefined) return props.allowRun;
  const cmd = props.command.toLowerCase();
  return cmd.includes('sync') || cmd.includes('export-rules');
});

async function copy() {
  try {
    await navigator.clipboard.writeText(props.command);
    copied.value = true;
    setTimeout(() => {
      copied.value = false;
    }, 2000);
  } catch {
    // fallback
  }
}

async function runCommand() {
  isRunning.value = true;
  try {
    const cmd = props.command.toLowerCase();
    if (cmd.includes('sync')) {
      const ok = await store.triggerSync();
      if (ok) {
        ran.value = true;
      }
    } else if (cmd.includes('export-rules')) {
      // Refresh context and rules
      await store.fetchContextPreview();
      success('Refreshed shared instruction rules preview');
      ran.value = true;
    } else {
      await copy();
      success(`Copied: ${props.command}`);
      ran.value = true;
    }
    setTimeout(() => {
      ran.value = false;
    }, 2500);
  } catch (err: any) {
    error(err?.message || 'Action failed');
  } finally {
    isRunning.value = false;
  }
}
</script>
