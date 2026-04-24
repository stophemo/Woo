<template>
  <header class="top-menu">
    <div class="menu-left">
      <Dropdown v-for="(menu, index) in menus" :key="menu.label" :ref="el => setDropdownRef(el, index)" @open-change="handleOpenChange(index, $event)">
        <template #trigger>
          <div class="menu-item">{{ menu.label }}</div>
        </template>
        <DropdownMenu :items="menu.items" @action="handleMenuAction" />
      </Dropdown>
    </div>
    <div class="menu-right">
      <button class="window-control-btn" @click="$emit('toggle-left-sidebar')" title="收起左侧菜单">
        <IconLeftSidebar />
      </button>
      <button class="window-control-btn" @click="$emit('toggle-thumbnail-sidebar')" title="收起缩略图栏">
        <IconThumbnailSidebar />
      </button>
      <button class="window-control-btn" @click="$emit('toggle-right-sidebar')" title="收起AI聊天">
        <IconRightSidebar />
      </button>
      <button class="window-control-btn minimize-btn" @click="minimizeWindow" title="最小化">
        <IconMinimize />
      </button>
      <button class="window-control-btn maximize-btn" @click="maximizeWindow" title="最大化">
        <IconMaximize />
      </button>
      <button class="window-control-btn close-btn" @click="closeWindow" title="关闭">
        <IconClose />
      </button>
    </div>
  </header>
</template>

<script setup lang="ts">
import { ref, type ComponentPublicInstance } from 'vue'
import IconLeftSidebar from '../../components/icons/IconLeftSidebar.vue'
import IconThumbnailSidebar from '../../components/icons/IconThumbnailSidebar.vue'
import IconRightSidebar from '../../components/icons/IconRightSidebar.vue'
import IconMinimize from '../../components/icons/IconMinimize.vue'
import IconMaximize from '../../components/icons/IconMaximize.vue'
import IconClose from '../../components/icons/IconClose.vue'
import Dropdown from '../ui/Dropdown.vue'
import DropdownMenu from '../ui/DropdownMenu.vue'
import {
  fileMenuItems,
  editMenuItems,
  aiMenuItems,
  markMenuItems,
  viewMenuItems,
  helpMenuItems
} from '../../config/menus'

// 菜单配置数据
class TopMenu {
  static menus = [
    { label: '文件', items: fileMenuItems },
    { label: '编辑', items: editMenuItems },
    { label: 'AI', items: aiMenuItems },
    { label: '标记', items: markMenuItems },
    { label: '查看', items: viewMenuItems },
    { label: '帮助', items: helpMenuItems }
  ]
}

const menus = ref(TopMenu.menus)
const dropdownRefs = ref<Map<number, ComponentPublicInstance>>(new Map())
const activeMenuIndex = ref<number | null>(null)

// 设置 Dropdown 组件引用
const setDropdownRef = (el: any, index: number) => {
  if (el && el.close) {
    dropdownRefs.value.set(index, el)
  }
}

// 处理 Dropdown 打开/关闭事件，确保同时只有一个下拉菜单打开
const handleOpenChange = (index: number, isOpen: boolean) => {
  if (isOpen) {
    // 如果有其他菜单打开，关闭它们
    if (activeMenuIndex.value !== null && activeMenuIndex.value !== index) {
      const prevDropdown = dropdownRefs.value.get(activeMenuIndex.value)
      if (prevDropdown) {
        ;(prevDropdown as any).close()
      }
    }
    activeMenuIndex.value = index
  } else {
    // 如果当前活动菜单关闭了，清空活动索引
    if (activeMenuIndex.value === index) {
      activeMenuIndex.value = null
    }
  }
}

defineEmits<{
  'toggle-left-sidebar': []
  'toggle-thumbnail-sidebar': []
  'toggle-right-sidebar': []
}>()

// 窗口控制
const minimizeWindow = () => {
  if (window.electronAPI) {
    window.electronAPI.minimize()
  }
}

const maximizeWindow = () => {
  if (window.electronAPI) {
    window.electronAPI.maximize()
  }
}

const closeWindow = () => {
  if (window.electronAPI) {
    window.electronAPI.close()
  }
}

// 菜单操作处理
const handleMenuAction = (action: string) => {
  console.log('Menu action:', action)
  // 处理 GitHub 链接
  if (action === 'github') {
    if (window.electronAPI) {
      window.electronAPI.openExternalLink('https://github.com/stophemo/Non-ego-Notes')
    }
    return
  }
  // 这里可以根据不同的 action 执行不同的操作
  // 例如：
  // if (action === 'new-document') { ... }
}
</script>

<style scoped>
.top-menu {
  display: flex;
  justify-content: space-between;
  align-items: center;
  padding: 0 16px;
  height: 40px;
  background-color: #f5f5f5;
  border-bottom: 1px solid #e0e0e0;
  -webkit-app-region: drag;
}

.menu-left {
  display: flex;
  gap: 4px;
  -webkit-app-region: no-drag;
}

.menu-item {
  padding: 6px 12px;
  cursor: pointer;
  border-radius: 4px;
  font-size: 14px;
  color: #333;
}

.menu-item:hover {
  background-color: #e5e5e5;
}

.menu-right {
  display: flex;
  gap: 4px;
  -webkit-app-region: no-drag;
}

.window-control-btn {
  background: transparent;
  border: none;
  cursor: pointer;
  padding: 8px;
  border-radius: 4px;
  display: flex;
  align-items: center;
  justify-content: center;
  color: #666;
  transition: all 0.2s;
}

.window-control-btn:hover {
  background-color: #e5e5e5;
  color: #333;
}

.window-control-btn.close-btn:hover {
  background-color: #e81123;
  color: white;
}

.window-control-btn svg {
  width: 18px;
  height: 18px;
}
</style>
