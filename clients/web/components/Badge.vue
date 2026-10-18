<template>
  <span
    class="inline-flex items-center gap-1.5 px-2 py-0.5 rounded-pill text-xs font-medium border"
    :class="[
      variant === 'ok' ? 'bg-ok-bg text-ok border-ok/30' :
      variant === 'warn' ? 'bg-warn-bg text-warn border-warn/30' :
      variant === 'bad' ? 'bg-bad-bg text-bad border-bad/30' :
      variant === 'info' ? 'bg-info-bg text-info border-info/30' :
      variant === 'accent' ? 'bg-accent/15 text-accent border-accent/30' :
      'bg-surface-2 text-text-muted border-border'
    ]"
  >
    <span
      v-if="dot"
      class="w-1.5 h-1.5 rounded-full"
      :class="[
        variant === 'ok' ? 'bg-ok' :
        variant === 'warn' ? 'bg-warn' :
        variant === 'bad' ? 'bg-bad' :
        variant === 'info' ? 'bg-info' :
        variant === 'accent' ? 'bg-accent' :
        'bg-text-muted'
      ]"
    />
    <slot />
  </span>
</template>

<script setup lang="ts">
withDefaults(
  defineProps<{
    variant?: 'ok' | 'warn' | 'bad' | 'info' | 'accent' | 'neutral';
    dot?: boolean;
  }>(),
  {
    variant: 'neutral',
    dot: false,
  }
);
</script>
