<template>
  <svg 
    :viewBox="viewBox" 
    :width="size" 
    :height="size" 
    fill="none" 
    stroke="currentColor" 
    :stroke-width="strokeWidth"
  >
    <rect x="3" y="3" width="7" height="18" rx="1" opacity="0.3"/>
    <rect x="14" y="3" width="7" height="18" rx="1" opacity="0.3"/>
    <rect x="8.5" y="3" width="7" height="18" rx="1"/>
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
