<template>
  <div class="dropdown-item" @click="$emit('click')">
    <slot></slot>
  </div>
</template>

<script setup lang="ts">
defineEmits<{
  click: []
}>()
</script>

<style scoped>
.dropdown-item {
  padding: 8px 16px;
  cursor: pointer;
  font-size: 13px;
  color: var(--text-primary);
  transition: background-color 0.2s;
}

.dropdown-item:hover {
  background-color: var(--bg-hover);
}
</style>
