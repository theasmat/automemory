<template>
  <button
    :type="type || 'button'"
    :disabled="disabled || loading"
    class="inline-flex items-center justify-center gap-2 rounded-pill font-medium transition-colors focus-visible:outline-none disabled:opacity-50 disabled:cursor-not-allowed text-sm"
    :class="[
      sizeClasses,
      variantClasses,
      customClass || ''
    ]"
    @click="$emit('click', $event)"
  >
    <svg
      v-if="loading"
      class="animate-spin -ml-0.5 h-3.5 w-3.5 text-current"
      fill="none"
      viewBox="0 0 24 24"
    >
      <circle class="opacity-25" cx="12" cy="12" r="10" stroke="currentColor" stroke-width="4" />
      <path class="opacity-75" fill="currentColor" d="M4 12a8 8 0 018-8v8H4z" />
    </svg>
    <slot name="icon" />
    <slot />
    <svg v-if="locked" class="h-3 w-3 opacity-70" viewBox="0 0 20 20" fill="currentColor" role="img" aria-label="Asks for your password">
      <title>Asks for your password</title>
      <path fill-rule="evenodd" d="M10 2a4 4 0 00-4 4v2H5a1 1 0 00-1 1v7a1 1 0 001 1h10a1 1 0 001-1V9a1 1 0 00-1-1h-1V6a4 4 0 00-4-4zm2 6V6a2 2 0 10-4 0v2h4z" clip-rule="evenodd" />
    </svg>
  </button>
</template>

<script setup lang="ts">
import { computed, watch } from 'vue';
import { usePolicy } from '~/composables/usePolicy';

const props = withDefaults(
  defineProps<{
    variant?: 'primary' | 'secondary' | 'ghost' | 'danger';
    size?: 'sm' | 'md' | 'lg';
    type?: 'button' | 'submit' | 'reset';
    disabled?: boolean;
    loading?: boolean;
    customClass?: string;
    /** The API path this button calls; a lock shows when the server's policy says it asks for the password. */
    lock?: string;
  }>(),
  {
    variant: 'secondary',
    size: 'md',
    type: 'button',
    disabled: false,
    loading: false,
  }
);

const { rules, tierOf } = usePolicy();
const locked = computed(() => !!props.lock && tierOf(props.lock) === 'protected');
// A path that is not in the policy table is a bug in the page: say so loudly (the browser smoke test fails on it).
watch(rules, () => {
  if (props.lock && rules.value.length && tierOf(props.lock) === null) console.error(`Button lock="${props.lock}" is not in /api/policy`);
}, { immediate: true });

defineEmits<{
  (e: 'click', event: MouseEvent): void;
}>();

const sizeClasses = computed(() => {
  switch (props.size) {
    case 'sm':
      return 'px-3 py-1.5 text-xs';
    case 'lg':
      return 'px-5 py-2.5 text-base';
    default:
      return 'px-4 py-2 text-sm';
  }
});

const variantClasses = computed(() => {
  switch (props.variant) {
    case 'primary':
      return 'bg-accent-solid text-accent-fg hover:opacity-90 font-semibold';
    case 'danger':
      return 'bg-bad-bg text-bad hover:opacity-80 font-medium';
    case 'ghost':
      return 'bg-transparent text-text-muted hover:text-text hover:bg-surface-2';
    default:
      return 'bg-surface-2 text-text hover:bg-border/80';
  }
});
</script>
