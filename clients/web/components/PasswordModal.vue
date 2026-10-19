<template>
  <Modal
    :is-open="isOpen"
    title="Human Approval Required"
    max-width-class="max-w-md"
    @close="cancel"
  >
    <div class="space-y-4">
      <div class="flex items-start gap-3 p-3 rounded-card bg-accent/10 border border-accent/20 text-sm text-text">
        <svg class="w-5 h-5 text-accent flex-shrink-0 mt-0.5" fill="none" viewBox="0 0 24 24" stroke="currentColor">
          <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M12 15v2m-6 4h12a2 2 0 002-2v-6a2 2 0 00-2-2H6a2 2 0 00-2 2v6a2 2 0 002 2zm10-10V7a4 4 0 00-8 0v4h8z" />
        </svg>
        <div>
          <span class="font-semibold text-text">Protected action:</span>
          <span class="text-text-muted ml-1">{{ activeRequest?.actionName || 'Safety-gated write' }}</span>
          <p class="text-xs text-text-muted mt-1">
            This action modifies agent safety rules or project knowledge. Enter your human password. It is used for this one action and is never stored.
          </p>
        </div>
      </div>

      <div>
        <label for="password-input" class="block text-xs font-medium text-text-muted mb-1.5">
          AutoMemory Password
        </label>
        <input
          id="password-input"
          ref="inputRef"
          v-model="password"
          type="password"
          class="w-full bg-surface border border-border-strong rounded-card px-3 py-2 text-sm text-text placeholder-text-muted focus:outline-none focus:ring-2 focus:ring-accent focus:ring-offset-2 focus:ring-offset-bg"
          placeholder="••••••••••••"
          autocomplete="off" data-1p-ignore data-lpignore="true" spellcheck="false"
          @keydown.enter="handleSubmit"
        />
        <p v-if="activeRequest?.errorMessage" class="text-xs text-bad mt-1.5">
          {{ activeRequest.errorMessage }}
        </p>
      </div>
    </div>

    <template #footer>
      <Button variant="ghost" size="sm" @click="cancel">
        Cancel
      </Button>
      <Button variant="primary" size="sm" :disabled="!password" @click="handleSubmit">
        Confirm Action
      </Button>
    </template>
  </Modal>
</template>

<script setup lang="ts">
import { ref, watch, nextTick } from 'vue';
import { usePasswordModal } from '~/composables/usePasswordModal';

const { isOpen, activeRequest, submit, cancel } = usePasswordModal();
const password = ref('');
const inputRef = ref<HTMLInputElement | null>(null);

watch(isOpen, (open) => {
  if (open) {
    password.value = '';
    nextTick(() => {
      inputRef.value?.focus();
    });
  }
});

function handleSubmit() {
  if (!password.value) return;
  const entered = password.value;
  password.value = ''; // the field never keeps it
  submit(entered);
}

// Closing the dialog in any way also empties the field.
watch(isOpen, (open) => {
  if (!open) password.value = '';
});
</script>
