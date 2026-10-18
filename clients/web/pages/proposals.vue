<template>
  <div class="p-8 text-center text-text-muted text-sm">Redirecting to Inbox...</div>
</template>

<script setup lang="ts">
navigateTo('/inbox', { replace: true, redirectCode: 301 });
</script>
