<template>
  <svg 
    :viewBox="viewBox" 
    :width="size" 
    :height="size" 
    fill="none" 
    stroke="currentColor" 
    :stroke-width="strokeWidth"
  >
    <line x1="5" y1="12" x2="19" y2="12"/>
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
