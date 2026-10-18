<template>
  <button
    :type="type || 'button'"
    :aria-label="ariaLabel"
    :title="title || ariaLabel"
    :disabled="disabled"
    class="inline-flex items-center justify-center rounded-card text-text-muted hover:text-text hover:bg-surface-2 transition-colors disabled:opacity-40 disabled:cursor-not-allowed"
    :class="[
      size === 'sm' ? 'w-7 h-7 text-xs' : size === 'lg' ? 'w-10 h-10 text-base' : 'w-8 h-8 text-sm',
      border ? 'border border-border' : '',
      customClass || ''
    ]"
    @click="$emit('click', $event)"
  >
    <slot />
  </button>
</template>

<script setup lang="ts">
withDefaults(
  defineProps<{
    ariaLabel: string;
    title?: string;
    type?: 'button' | 'submit' | 'reset';
    size?: 'sm' | 'md' | 'lg';
    border?: boolean;
    disabled?: boolean;
    customClass?: string;
  }>(),
  {
    type: 'button',
    size: 'md',
    border: false,
    disabled: false,
  }
);

defineEmits<{
  (e: 'click', event: MouseEvent): void;
}>();
</script>
