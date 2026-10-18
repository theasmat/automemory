<template>
  <div class="p-8 text-center text-text-muted text-sm">Redirecting to Project brain...</div>
</template>

<script setup lang="ts">
navigateTo('/brain', { replace: true, redirectCode: 301 });
</script>
