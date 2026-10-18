<template>
  <Modal
    :is-open="isOpen"
    title="Keyboard Shortcuts"
    max-width-class="max-w-md"
    @close="close"
  >
    <div class="space-y-4 text-sm">
      <div class="space-y-2">
        <h4 class="font-semibold text-text text-xs text-text-muted">Global Navigation</h4>
        <div class="grid grid-cols-2 gap-2 text-text">
          <div class="flex items-center justify-between p-2 rounded bg-surface-2">
            <span>Home</span>
            <kbd class="font-mono text-text-muted text-xs">g h</kbd>
          </div>
          <div class="flex items-center justify-between p-2 rounded bg-surface-2">
            <span>Project brain</span>
            <kbd class="font-mono text-text-muted text-xs">g b</kbd>
          </div>
          <div class="flex items-center justify-between p-2 rounded bg-surface-2">
            <span>Agents &amp; locks</span>
            <kbd class="font-mono text-text-muted text-xs">g a</kbd>
          </div>
          <div class="flex items-center justify-between p-2 rounded bg-surface-2">
            <span>Inbox</span>
            <kbd class="font-mono text-text-muted text-xs">g i</kbd>
          </div>
          <div class="flex items-center justify-between p-2 rounded bg-surface-2">
            <span>Sync &amp; devices</span>
            <kbd class="font-mono text-text-muted text-xs">g s</kbd>
          </div>
          <div class="flex items-center justify-between p-2 rounded bg-surface-2">
            <span>Command palette</span>
            <kbd class="font-mono text-text-muted text-xs">⌘K</kbd>
          </div>
        </div>
      </div>

      <div class="space-y-2">
        <h4 class="font-semibold text-text text-xs text-text-muted">Actions &amp; Inbox</h4>
        <div class="space-y-1.5 text-text">
          <div class="flex items-center justify-between p-2 rounded bg-surface-2">
            <span>Add note to Project brain</span>
            <kbd class="font-mono text-text-muted text-xs">n</kbd>
          </div>
          <div class="flex items-center justify-between p-2 rounded bg-surface-2">
            <span>Navigate items in Inbox</span>
            <kbd class="font-mono text-text-muted text-xs">J / K</kbd>
          </div>
          <div class="flex items-center justify-between p-2 rounded bg-surface-2">
            <span>Approve selected suggestion</span>
            <kbd class="font-mono text-text-muted text-xs">A</kbd>
          </div>
          <div class="flex items-center justify-between p-2 rounded bg-surface-2">
            <span>Reject selected suggestion</span>
            <kbd class="font-mono text-text-muted text-xs">R</kbd>
          </div>
        </div>
      </div>
    </div>
  </Modal>
</template>

<script setup lang="ts">
import { ref, onMounted, onUnmounted } from 'vue';

const isOpen = ref(false);

function open() {
  isOpen.value = true;
}

function close() {
  isOpen.value = false;
}

function handleKeydown(_e: KeyboardEvent) {
  // Handled by HelpDrawer (? opens HelpDrawer with shortcuts)
}

onMounted(() => {
  // ShortcutHelp can still be opened via open() programmatically
});

onUnmounted(() => {
});

defineExpose({ open, close });
</script>
