<template>
  <header class="top-menu" :class="{ collapsed: !isOpen, fullscreen: isFullscreen }" :style="macMenuStyle">
    <div class="menu-left">
      <Dropdown v-for="(menu, index) in menus" :key="menu.label" :ref="el => setDropdownRef(el, index)" @open-change="handleOpenChange(index, $event)">
        <template #trigger>
          <div class="menu-item">{{ menu.label }}</div>
        </template>
        <DropdownMenu :items="menu.items" @action="handleMenuAction" />
      </Dropdown>
    </div>
    <div class="menu-right">
      <button class="window-control-btn" @click="themeStore.toggleTheme()" :title="themeStore.theme === 'light' ? '切换到夜间模式' : '切换到日间模式'">
        <IconThemeToggle :mode="themeStore.theme" />
      </button>
      <button
        class="window-control-btn"
        :class="{ 'is-active': !isOpen }"
        @click="$emit('toggle-top-menu')"
        :title="'显示/隐藏菜单栏 (' + modKey() + '↑)'"
      >
        <IconTopMenu />
      </button>
      <button class="window-control-btn" @click="$emit('toggle-status-bar')" :title="'显示/隐藏底部状态栏 (' + modKey() + '↓)'">
        <IconStatusBar />
      </button>
      <button class="window-control-btn" @click="$emit('toggle-left-sidebar')" :title="'显示/隐藏左侧菜单 (' + modKey() + '←)'">
        <IconLeftSidebar />
      </button>
      <button class="window-control-btn" @click="$emit('toggle-thumbnail-sidebar')" :title="'显示/隐藏缩略图栏 (' + modKey() + '←)'">
        <IconThumbnailSidebar />
      </button>
      <button class="window-control-btn" @click="$emit('toggle-right-sidebar')" :title="'显示/隐藏 AI 聊天 (' + modKey() + '→)'">
        <IconRightSidebar />
      </button>
      <div v-if="!isMac" class="menu-divider"></div>
      <button v-if="!isMac" class="window-control-btn minimize-btn" @click="minimizeWindow" title="最小化">
        <IconMinimize />
      </button>
      <button v-if="!isMac" class="window-control-btn maximize-btn" @click="maximizeWindow" title="最大化">
        <IconMaximize />
      </button>
      <button v-if="!isMac" class="window-control-btn close-btn" @click="closeWindow" title="关闭">
        <IconClose />
      </button>
    </div>
  </header>
</template>

<script setup lang="ts">
import { ref, type ComponentPublicInstance, computed } from 'vue'
import IconLeftSidebar from '../../components/icons/IconLeftSidebar.vue'
import IconThumbnailSidebar from '../../components/icons/IconThumbnailSidebar.vue'
import IconRightSidebar from '../../components/icons/IconRightSidebar.vue'
import IconMinimize from '../../components/icons/IconMinimize.vue'
import IconMaximize from '../../components/icons/IconMaximize.vue'
import IconClose from '../../components/icons/IconClose.vue'
import IconThemeToggle from '../../components/icons/IconThemeToggle.vue'
import IconTopMenu from '../../components/icons/IconTopMenu.vue'
import IconStatusBar from '../../components/icons/IconStatusBar.vue'
import Dropdown from '../ui/Dropdown.vue'
import DropdownMenu from '../ui/DropdownMenu.vue'
import { useThemeStore } from '../../stores/theme'
import {
  fileMenuItems,
  editMenuItems,
  aiMenuItems,
  markMenuItems,
  viewMenuItems,
  helpMenuItems
} from '../../config/menus'

const themeStore = useThemeStore()

const isMac = navigator.platform.includes('Mac')

// macOS 菜单栏左侧留空给交通灯按钮（约 76px）
const macMenuStyle = computed(() => {
  return isMac ? { paddingLeft: '76px' } : {}
})

// 返回当前平台对应的修饰键标识
function modKey(): string {
  return isMac ? '⌘' : 'Ctrl+'
}

// 递归处理菜单项中的快捷键（macOS 上将 Ctrl 替换为 ⌘）
function normalizeShortcut(shortcut?: string): string | undefined {
  if (!shortcut || !isMac) return shortcut
  return shortcut.replace(/Ctrl\+/g, '⌘')
}

function normalizeItems(items: import('../../config/menus').MenuItem[]): import('../../config/menus').MenuItem[] {
  return items.map(item => {
    if (item.type === 'item') {
      return { ...item, shortcut: normalizeShortcut(item.shortcut) }
    }
    if (item.type === 'submenu' && item.children) {
      return { ...item, children: normalizeItems(item.children) }
    }
    return item
  })
}

// 菜单配置数据
class TopMenu {
  static menus = [
    { label: '文件', items: normalizeItems(fileMenuItems) },
    { label: '编辑', items: normalizeItems(editMenuItems) },
    { label: 'AI', items: normalizeItems(aiMenuItems) },
    { label: '标记', items: normalizeItems(markMenuItems) },
    { label: '查看', items: normalizeItems(viewMenuItems) },
    { label: '帮助', items: normalizeItems(helpMenuItems) }
  ]
}

const menus = ref(TopMenu.menus)
const dropdownRefs = ref<Map<number, ComponentPublicInstance>>(new Map())
const activeMenuIndex = ref<number | null>(null)

interface Props {
  isOpen?: boolean
  isFullscreen?: boolean
}
withDefaults(defineProps<Props>(), {
  isOpen: true,
  isFullscreen: false
})

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

const emit = defineEmits<{
  'toggle-left-sidebar': []
  'toggle-thumbnail-sidebar': []
  'toggle-right-sidebar': []
  'open-settings': [mode: 'file' | 'ai']
  'toggle-top-menu': []
  'toggle-status-bar': []
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
  // 关闭所有下拉菜单（点击菜单项后自动收起）
  for (const [, dropdown] of dropdownRefs.value) {
    if (dropdown && (dropdown as any).close) {
      ;(dropdown as any).close()
    }
  }
  if (action === 'github') {
    if (window.electronAPI) {
      window.electronAPI.openExternalLink('https://github.com/stophemo/Woo')
    }
    return
  }
  if (action === 'settings') {
    emit('open-settings', 'file')
    return
  }
  if (action === 'ai-settings') {
    emit('open-settings', 'ai')
    return
  }
  if (action === 'open-chat') {
    emit('toggle-right-sidebar')
    return
  }
}
</script>

<style scoped>
.top-menu {
  display: flex;
  justify-content: space-between;
  align-items: center;
  padding: 0 16px;
  height: 40px;
  background-color: var(--bg-toolbar);
  border-bottom: 1px solid var(--border-primary);
  -webkit-app-region: drag;
  transition: var(--theme-transition), height 0.25s ease, transform 0.25s ease, padding 0.25s ease, border-bottom-color 0.25s ease, opacity 0.2s ease;
  transform-origin: top;
}

.top-menu.collapsed {
  height: 0;
  padding-top: 0;
  padding-bottom: 0;
  border-bottom-color: transparent;
  transform: translateY(-100%);
  opacity: 0;
  pointer-events: none;
  overflow: hidden;
}

/* 全屏模式下隐藏左侧菜单目录，由 macOS 系统菜单栏接管；
   同时右侧功能按键保持右对齐 */
.top-menu.fullscreen {
  justify-content: flex-end;
}
.top-menu.fullscreen .menu-left {
  display: none;
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
  color: var(--text-primary);
}

.menu-item:hover {
  background-color: var(--bg-hover);
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
  color: var(--text-secondary);
  transition: all 0.2s;
}

.window-control-btn:hover {
  background-color: var(--bg-hover);
  color: var(--text-primary);
}

.window-control-btn.is-active {
  background-color: var(--bg-active, var(--bg-hover));
  color: var(--accent, var(--text-primary));
}

.window-control-btn.close-btn:hover {
  background-color: var(--close-hover-bg);
  color: var(--close-hover-color);
}

.window-control-btn svg {
  width: 18px;
  height: 18px;
}

.menu-divider {
  width: 1px;
  height: 16px;
  background-color: var(--border-primary);
  margin: 0 2px;
  align-self: center;
}
</style>
