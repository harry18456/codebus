<!-- Synthetic UI component — noise (off-route). -->
<template>
  <div class="event-card">
    <h3>{{ title }}</h3>
    <p>{{ description }}</p>
  </div>
</template>

<script setup lang="ts">
defineProps<{ title: string; description: string }>()
</script>

<style scoped>
.event-card { padding: 8px; border: 1px solid #ddd; }
</style>
