<template>
  <div class="p-8 text-center text-text-muted text-sm">Redirecting to Home...</div>
</template>

<script setup lang="ts">
navigateTo('/', { replace: true, redirectCode: 301 });
</script>
