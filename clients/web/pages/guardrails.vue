<template>
  <div class="p-8 text-center text-text-muted text-sm">Redirecting to Safety...</div>
</template>

<script setup lang="ts">
navigateTo('/safety', { replace: true, redirectCode: 301 });
</script>
