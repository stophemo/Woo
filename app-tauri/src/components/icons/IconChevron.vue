<template>
  <svg
    xmlns="http://www.w3.org/2000/svg"
    width="14"
    height="14"
    viewBox="0 0 24 24"
    fill="none"
    stroke="currentColor"
    stroke-width="2.2"
    stroke-linecap="round"
    stroke-linejoin="round"
    class="chevron-icon"
    :class="`chevron-${direction}`"
  >
    <polyline points="6 15 12 9 18 15" />
  </svg>
</template>

<script setup lang="ts">
defineProps<{ direction?: 'up' | 'down' | 'left' | 'right' }>()
</script>

<style scoped>
.chevron-icon {
  flex-shrink: 0;
  transition: transform 0.25s ease;
}
.chevron-up    { transform: rotate(0deg); }
.chevron-right { transform: rotate(90deg); }
.chevron-down  { transform: rotate(180deg); }
.chevron-left  { transform: rotate(-90deg); }
</style>
