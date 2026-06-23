<template>
  <div class="dropdown" @click.stop>
    <div class="dropdown-trigger" @click="handleTriggerClick">
      <slot name="trigger"></slot>
    </div>
    <transition name="dropdown">
      <div v-if="isOpen" class="dropdown-menu">
        <slot></slot>
      </div>
    </transition>
  </div>
</template>

<script setup lang="ts">
import { ref, onMounted, onUnmounted } from 'vue'

const isOpen = ref(false)
const emit = defineEmits<{
  'open-change': [value: boolean]
}>()

const toggle = () => {
  isOpen.value = !isOpen.value
  emit('open-change', isOpen.value)
}

const handleTriggerClick = () => {
  toggle()
}

const close = () => {
  isOpen.value = false
  emit('open-change', false)
}

// 点击外部关闭下拉菜单
const handleClickOutside = (event: MouseEvent) => {
  const target = event.target as HTMLElement
  if (!target.closest('.dropdown')) {
    isOpen.value = false
  }
}

onMounted(() => {
  document.addEventListener('click', handleClickOutside)
})

onUnmounted(() => {
  document.removeEventListener('click', handleClickOutside)
})

defineExpose({ close, toggle })
</script>

<style scoped>
.dropdown {
  position: relative;
}

.dropdown-trigger {
  cursor: pointer;
}

.dropdown-menu {
  position: absolute;
  top: 100%;
  left: 0;
  margin-top: 4px;
  background-color: var(--bg-elevated);
  border-radius: 6px;
  box-shadow: var(--shadow-dropdown);
  padding: 6px 0;
  min-width: 180px;
  z-index: 1000;
}

.dropdown-enter-active,
.dropdown-leave-active {
  transition: opacity 0.2s, transform 0.2s;
}

.dropdown-enter-from,
.dropdown-leave-to {
  opacity: 0;
  transform: translateY(-8px);
}
</style>
