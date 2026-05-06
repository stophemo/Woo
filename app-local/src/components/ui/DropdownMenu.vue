<template>
  <div :class="['dropdown-menu-container', { 'is-submenu': isSubmenu }]">
    <template v-for="(item, index) in items" :key="index">
      <!-- 普通菜单项 -->
      <DropdownItem 
        v-if="item.type === 'item'" 
        @click="$emit('action', item.action || '')"
      >
        <span v-if="!item.isHtml">{{ item.label }}</span>
        <span v-else v-html="item.label"></span>
      </DropdownItem>
      
      <!-- 分隔线 -->
      <DropdownDivider 
        v-else-if="item.type === 'divider'" 
      />
      
      <!-- 子菜单 -->
      <div 
        v-else-if="item.type === 'submenu'"
        class="submenu-item"
        @mouseenter="activeSubmenu = index"
        @mouseleave="activeSubmenu = null"
      >
        <div class="submenu-trigger">
          <span>{{ item.label }}</span>
          <span class="submenu-arrow">›</span>
        </div>
        <transition name="submenu">
          <DropdownMenu
            v-if="activeSubmenu === index"
            :items="item.children || []"
            :is-submenu="true"
            @action="$emit('action', $event)"
          />
        </transition>
      </div>
    </template>
  </div>
</template>

<script setup lang="ts">
import { ref } from 'vue'
import type { MenuItem } from '../../config/menus'
import DropdownItem from './DropdownItem.vue'
import DropdownDivider from './DropdownDivider.vue'

interface Props {
  items: MenuItem[]
  isSubmenu?: boolean  // 是否为子菜单（影响定位样式）
}

withDefaults(defineProps<Props>(), {
  isSubmenu: false
})

const activeSubmenu = ref<number | null>(null)

defineEmits<{
  action: [type: string]
}>()
</script>

<style scoped>
.dropdown-menu-container {
  background-color: var(--bg-elevated);
  border-radius: 6px;
  box-shadow: var(--shadow-dropdown);
  padding: 6px 0;
  min-width: 180px;
}

/* 子菜单定位 */
.dropdown-menu-container.is-submenu {
  position: absolute;
  left: 100%;
  top: 0;
  z-index: 1001;
}

.submenu-item {
  position: relative;
}

.submenu-trigger {
  display: flex;
  justify-content: space-between;
  align-items: center;
  padding: 8px 16px;
  cursor: pointer;
  font-size: 13px;
  color: var(--text-primary);
  transition: background-color 0.2s;
}

.submenu-trigger:hover {
  background-color: var(--bg-hover);
}

.submenu-arrow {
  font-size: 16px;
  color: var(--text-muted);
}

.submenu-enter-active,
.submenu-leave-active {
  transition: opacity 0.2s;
}

.submenu-enter-from,
.submenu-leave-to {
  opacity: 0;
}
</style>
