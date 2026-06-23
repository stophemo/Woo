<template>
  <svg 
    :viewBox="viewBox" 
    :width="size" 
    :height="size" 
    fill="none" 
    stroke="currentColor" 
    :stroke-width="strokeWidth"
  >
    <rect x="5" y="5" width="14" height="14" rx="2"/>
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
