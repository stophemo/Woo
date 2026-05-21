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
    <circle cx="12" cy="8" r="4" />
    <path d="M4 20c0-4 4-6 8-6s8 2 8 6" />
  </svg>
</template>

<script setup lang="ts">
interface Props {
  size?: number
  strokeWidth?: number
}

withDefaults(defineProps<Props>(), {
  size: 18,
  strokeWidth: 2
})

const viewBox = '0 0 24 24'
</script>
