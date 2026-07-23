<template>
  <header ref="menuBarEl" class="top-menu" :class="{ collapsed: !isOpen, 'is-mobile': isMobile }" :style="macMenuStyle" data-tauri-drag-region>
    <div class="menu-left">
      <!-- 移动端汉堡菜单按钮 -->
      <button v-if="isMobile" class="window-control-btn hamburger-btn" @click="$emit('toggle-left-sidebar')" title="打开目录">
        <IconHamburger :size="20" />
      </button>
      <!-- 桌面端下拉菜单 -->
      <template v-else>
        <Dropdown v-for="(menu, index) in menus" :key="menu.label" :ref="el => setDropdownRef(el, index)" @open-change="handleOpenChange(index, $event)">
          <template #trigger>
            <div class="menu-item">{{ menu.label }}</div>
          </template>
          <DropdownMenu :items="menu.items" @action="handleMenuAction" />
        </Dropdown>
      </template>
    </div>
    <!-- 桌面端完整按钮组 -->
    <div v-show="!compact && !isMobile" class="menu-right">
      <button class="window-control-btn" @click="themeStore.toggleTheme()" :title="themeToggleTitle">
        <IconThemeToggle :mode="themeIconMode" />
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
      <button class="window-control-btn" @click="handleSync" :title="syncTitle" :disabled="syncing">
        <svg width="16" height="16" viewBox="0 0 16 16" fill="none" stroke="currentColor" stroke-width="1.5" :class="{ 'spin': syncing }">
          <path d="M2 8a6 6 0 0 1 10.47-4M14 8a6 6 0 0 1-10.47 4"/>
          <path d="M13.5 1.5V4.5H10.5"/>
          <path d="M2.5 14.5V11.5H5.5"/>
        </svg>
      </button>
      <button class="window-control-btn" :title="isLoggedIn ? '账户信息' : '登录账户'" @click="$emit('open-login')">
        <svg width="16" height="16" viewBox="0 0 16 16" fill="none" stroke="currentColor" stroke-width="1.5">
          <circle cx="8" cy="5" r="2.5"/>
          <path d="M2 14c0-3 2.7-5 6-5s6 2 6 5"/>
        </svg>
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
    <!-- 移动端精简按钮组 -->
    <div v-if="isMobile" class="menu-right mobile-menu-right">
      <button class="window-control-btn" @click="$emit('toggle-document-drawer')" title="文稿列表">
        <IconThumbnailSidebar />
      </button>
      <button class="window-control-btn" @click="themeStore.toggleTheme()" :title="themeToggleTitle">
        <IconThemeToggle :mode="themeIconMode" />
      </button>
      <button class="window-control-btn" @click="$emit('open-settings')" title="设置">
        <IconSettings />
      </button>
      <button class="window-control-btn" @click="handleSync" :title="syncTitle" :disabled="syncing">
        <svg width="16" height="16" viewBox="0 0 16 16" fill="none" stroke="currentColor" stroke-width="1.5" :class="{ 'spin': syncing }">
          <path d="M2 8a6 6 0 0 1 10.47-4M14 8a6 6 0 0 1-10.47 4"/>
          <path d="M13.5 1.5V4.5H10.5"/>
          <path d="M2.5 14.5V11.5H5.5"/>
        </svg>
      </button>
      <button class="window-control-btn" :title="isLoggedIn ? '账户信息' : '登录账户'" @click="$emit('open-login')">
        <svg width="16" height="16" viewBox="0 0 16 16" fill="none" stroke="currentColor" stroke-width="1.5">
          <circle cx="8" cy="5" r="2.5"/>
          <path d="M2 14c0-3 2.7-5 6-5s6 2 6 5"/>
        </svg>
      </button>
    </div>
  </header>
</template>

<script setup lang="ts">
import { ref, type ComponentPublicInstance, computed, onMounted, onBeforeUnmount } from 'vue'
import IconLeftSidebar from '../../components/icons/IconLeftSidebar.vue'
import IconThumbnailSidebar from '../../components/icons/IconThumbnailSidebar.vue'
import IconSettings from '../../components/icons/IconSettings.vue'
import IconMinimize from '../../components/icons/IconMinimize.vue'
import IconMaximize from '../../components/icons/IconMaximize.vue'
import IconClose from '../../components/icons/IconClose.vue'
import IconThemeToggle from '../../components/icons/IconThemeToggle.vue'
import IconTopMenu from '../../components/icons/IconTopMenu.vue'
import { log } from '../../services/logger'
import IconStatusBar from '../../components/icons/IconStatusBar.vue'
import IconHamburger from '../../components/icons/IconHamburger.vue'
import Dropdown from '../ui/Dropdown.vue'
import DropdownMenu from '../ui/DropdownMenu.vue'
import { useThemeStore } from '../../stores/theme'
import { useSyncStore } from '../../stores/sync'
import { useWorkspaceStore } from '../../stores/workspace'
import { exportDocumentAs, type ExportFormat } from '../../services/exportDocument'
import {
  fileMenuItems,
  editMenuItems,
  markMenuItems,
  viewMenuItems,
  helpMenuItems
} from '../../config/menus'

const themeStore = useThemeStore()
const syncStore = useSyncStore()
const workspaceStore = useWorkspaceStore()

const themeIconMode = computed<'light' | 'dark'>(() => (
  themeStore.isDarkTheme(themeStore.theme) ? 'dark' : 'light'
))
const themeToggleTitle = computed(() => (
  themeStore.isDarkTheme(themeStore.theme) ? '切换到云白主题' : '切换到深夜主题'
))

import { isMac, shortcutDisplay } from '../../config/shortcutUtils'

// macOS：窗口模式留 80px 给交通灯按钮（垂直居中由 titleBarOverlay 保证）
// 全屏时交通灯隐藏，菜单项左对齐；移动端也不需要左侧内边距
const macMenuStyle = computed(() => {
  if (!isMac || props.isFullscreen || props.isMobile) return {}
  return { paddingLeft: '80px' }
})

// 返回当前平台对应的修饰键标识
function modKey(): string {
  return isMac ? '⌘' : 'Ctrl+'
}

// 递归处理菜单项中的快捷键（使用 shortcutUtils 平台感知转换）
function normalizeShortcut(shortcut?: string): string | undefined {
  if (!shortcut) return undefined
  return shortcutDisplay(shortcut)
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
  isLoggedIn?: boolean
  isMobile?: boolean
}
const props = withDefaults(defineProps<Props>(), {
  isOpen: true,
  isFullscreen: false,
  isLoggedIn: false,
  isMobile: false
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
  'toggle-document-drawer': []
  'open-settings': []
  'toggle-top-menu': []
  'toggle-status-bar': []
  'open-login': []
}>()

const syncTitle = computed(() => {
  if (syncing.value) return '同步中...'
  return `同步数据 (${syncStore.lastSyncLabel})`
})

const syncing = ref(false)

async function handleSync() {
  if (syncing.value || syncStore.isSyncing) return
  syncing.value = true
  try {
    const ok = await syncStore.triggerSync()
    window.dispatchEvent(new CustomEvent('sync:toast', {
      detail: {
        message: ok ? '同步完成' : syncStore.errorMsg || '同步失败，请重试',
        type: ok ? 'success' : 'error'
      }
    }))
  } catch (e: any) {
    window.dispatchEvent(new CustomEvent('sync:toast', {
      detail: { message: e?.message || '同步异常', type: 'error' }
    }))
  } finally {
    syncing.value = false
  }
}

const menuBarEl = ref<HTMLElement | null>(null)
const compact = ref(false)
let resizeObserver: ResizeObserver | null = null

// 右侧按钮区域总宽度 ≈ 8 个按钮 × 34px + 间隔 + macOS左边距
const COMPACT_THRESHOLD = 900

onMounted(() => {
  syncStore.listen()

  if (menuBarEl.value) {
    resizeObserver = new ResizeObserver(([entry]) => {
      compact.value = entry.contentRect.width < COMPACT_THRESHOLD
    })
    resizeObserver.observe(menuBarEl.value)
  }
})

onBeforeUnmount(() => {
  resizeObserver?.disconnect()
})

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
  log.app.info('Menu action:', action)
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
    emit('open-settings')
    return
  }
  // 导出（作用于当前打开的文稿）
  if (action.startsWith('export-')) {
    const formatMap: Record<string, ExportFormat> = {
      'export-markdown': 'markdown',
      'export-txt': 'txt',
      'export-image': 'image',
    }
    const format = formatMap[action]
    if (format) exportDocumentAs(workspaceStore.currentDocument, format)
    return
  }
  // 编辑器命令（如 'link'）通过自定义事件传递给 EditArea
  if (['link', 'image', 'table', 'hr'].includes(action)) {
    window.dispatchEvent(new CustomEvent('woo-editor-command', { detail: { command: action } }))
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
  padding-top: var(--safe-area-top);
  height: var(--top-menu-height);
  background-color: var(--bg-toolbar);
  border-bottom: 1px solid var(--border-primary);
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

.menu-left {
  display: flex;
  gap: 4px;
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
}

.mobile-menu-right {
  gap: 2px;
}

.top-menu.is-mobile {
  padding: var(--safe-area-top) 8px 0 8px;
}

.top-menu.is-mobile .menu-left {
  align-items: center;
}

.top-menu.is-mobile .window-control-btn {
  padding: 6px;
}

.top-menu.is-mobile .window-control-btn svg {
  width: 18px;
  height: 18px;
}

.hamburger-btn {
  padding: 6px;
  margin-left: -4px;
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

@keyframes spin {
  from { transform: rotate(0deg); }
  to { transform: rotate(360deg); }
}

.spin {
  animation: spin 1s linear infinite;
}
</style>
