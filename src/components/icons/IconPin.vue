<template>
  <svg
    :viewBox="viewBox"
    :width="size"
    :height="size"
    fill="none"
    stroke="currentColor"
    :stroke-width="strokeWidth"
    stroke-linecap="round"
    stroke-linejoin="round"
  >
    <path d="M9 3h6l-1 6 3.5 3.5V14H6.5v-1.5L10 9 9 3Z"/>
    <path d="M12 14v7"/>
  </svg>
</template>

<script setup lang="ts">
interface Props {
  size?: number
  strokeWidth?: number
}

withDefaults(defineProps<Props>(), {
  size: 18,
  strokeWidth: 1.6
})

const viewBox = '0 0 24 24'
</script>
