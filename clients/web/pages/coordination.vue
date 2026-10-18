<template>
  <div class="p-8 text-center text-text-muted text-sm">Redirecting to Agents & locks...</div>
</template>

<script setup lang="ts">
navigateTo('/agents', { replace: true, redirectCode: 301 });
</script>
