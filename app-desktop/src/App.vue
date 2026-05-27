<template>
  <div class="app-container">
    <!-- 顶部菜单栏 -->
    <TopMenu
      :is-open="topMenuOpen"
      :is-fullscreen="isFullscreen"
      :is-logged-in="authStore.isLoggedIn"
      @toggle-left-sidebar="toggleLeftSidebar"
      @toggle-thumbnail-sidebar="toggleThumbnailSidebar"
      @toggle-right-sidebar="toggleRightSidebar"
      @open-settings="openSettings"
      @toggle-top-menu="toggleTopMenu"
      @toggle-status-bar="toggleStatusBar"
      @open-login="openLoginDialog"
    />
    
    <!-- 主内容区域 -->
    <div class="main-content">
      <!-- 左侧菜单列 -->
      <LeftSidebar :is-open="leftSidebarOpen" />
      
      <!-- 中间缩略图列 -->
      <ThumbnailColumn :is-open="thumbnailSidebarOpen" :active-heading="editorActiveHeading" />
      
      <!-- 中央编辑区域 -->
      <EditArea ref="editAreaRef" :is-status-bar-open="statusBarOpen" @active-heading-change="handleActiveHeadingChange" />
      
      <!-- 右侧AI对话区域 -->
      <RightSidebar :is-open="rightSidebarOpen" @open-settings="openSettings" />
    </div>

    <!-- 设置弹窗 -->
    <SettingsDialog :visible="showSettings" :mode="settingsMode" @close="showSettings = false" />

    <!-- 登录/账户弹窗 -->
    <LoginDialog
      :visible="showLoginDialog"
      @close="showLoginDialog = false"
      @login-success="onLoginSuccess"
      @logout="onLogout"
    />

    <!-- 同步提示 -->
    <transition name="toast-fade">
      <div v-if="syncToast.visible" class="sync-toast" :class="syncToast.type">
        {{ syncToast.message }}
      </div>
    </transition>

   <!-- 自动更新通知 -->
    <UpdateNotification ref="updateNotificationRef" />
  </div>
</template>

<script setup lang="ts">
import { ref, onMounted, onBeforeUnmount } from 'vue'
import type { ComponentPublicInstance } from 'vue'
import TopMenu from './components/layout/TopMenu.vue'
import LeftSidebar from './components/layout/LeftSidebar.vue'
import ThumbnailColumn from './components/layout/ThumbnailColumn.vue'
import EditArea from './components/layout/EditArea.vue'
import RightSidebar from './components/layout/RightSidebar.vue'
import SettingsDialog from './components/layout/SettingsDialog.vue'
import LoginDialog from './components/layout/LoginDialog.vue'
import UpdateNotification from './components/ui/UpdateNotification.vue'
import { useThemeStore } from './stores/theme'
import { useAuthStore } from './stores/auth'
import { useWorkspaceStore } from './stores/workspace'
import { useSyncStore } from './stores/sync'
import { isModKey } from './config/shortcutUtils'
import { useLockStore } from './stores/lock'

// 初始化主题（确保 data-theme 属性在应用启动时就被设置到 <html>）
useThemeStore()

const workspaceStore = useWorkspaceStore()
const authStore = useAuthStore()
const syncStore = useSyncStore()

const updateNotificationRef = ref<ComponentPublicInstance & { check: () => void } | null>(null)
const editAreaRef = ref<ComponentPublicInstance | null>(null)

// 编辑器滚动 → 大纲高亮联动
const editorActiveHeading = ref<number | null>(null)
function handleActiveHeadingChange(headingIndex: number | null) {
  editorActiveHeading.value = headingIndex
}

// 处理来自 macOS 原生菜单的动作
function handleMenuAction(action: string) {
  switch (action) {
    case 'enter-fullscreen':
      isFullscreen.value = true
      // 全屏时自动收起顶部菜单栏（类似 Cmd+⬆ 效果），退出时恢复
      topMenuOpenBeforeFullscreen = topMenuOpen.value
      topMenuOpen.value = false
      break
    case 'leave-fullscreen':
      isFullscreen.value = false
      // 恢复全屏前的菜单栏状态
      topMenuOpen.value = topMenuOpenBeforeFullscreen
      break
    case 'settings':
      openSettings('file')
      break
    case 'ai-settings':
      openSettings('ai')
      break
    case 'new-document':
      workspaceStore.createNewDocument()
      break
    case 'new-folder':
      workspaceStore.createRootFolder()
      break
    case 'version-history':
      // 切换到第一个文档的版本历史
      break
    case 'toggle-left':
      toggleLeftSidebar()
      break
    case 'toggle-thumbnail':
      toggleThumbnailSidebar()
      break
    case 'toggle-right':
    case 'open-chat':
      toggleRightSidebar()
      break
    case 'theme':
      useThemeStore().toggleTheme()
      break
    case 'check-update':
      updateNotificationRef.value?.check()
      break
    case 'docs':
      break
    case 'github':
      if (window.electronAPI) {
        window.electronAPI.openExternalLink('https://github.com/stophemo/Woo')
      }
      break
    default:
      console.log('[MenuAction] 未处理的动作:', action)
  }
}

// 侧边栏状态
const leftSidebarOpen = ref(true)
const thumbnailSidebarOpen = ref(true)
const rightSidebarOpen = ref(true)
const showSettings = ref(false)
const showLoginDialog = ref(false)
const settingsMode = ref<'file' | 'ai'>('file')
const topMenuOpen = ref(true)
const statusBarOpen = ref(true)
const isFullscreen = ref(false)
let topMenuOpenBeforeFullscreen = true

function openSettings(mode: 'file' | 'ai') {
  settingsMode.value = mode
  showSettings.value = true
}

function openLoginDialog() {
  showLoginDialog.value = true
}

function onLoginSuccess() {
  showLoginDialog.value = false
  // 登录成功后重新拉取云端锁密码
  useLockStore().bootstrap()
  // 重新加载工作区数据
  workspaceStore.bootstrap()
}

function onLogout() {
  // 登出后重置工作区
  workspaceStore.reset()
}

// 启动流程：认证恢复 + 工作区加载
async function initApp() {
  // 1. 恢复认证 session
  await authStore.bootstrap()

  // 1.5 初始化加锁状态
  await useLockStore().bootstrap()

  // 2. 启动同步监听和提示
  syncStore.listen()
  setupSyncToast()
  setupSyncDataRefresh()
  try {
    await syncStore.refreshStatus()
  } catch { /* ignore */ }

  // 3. 加载工作区数据
  try {
    await workspaceStore.bootstrap()
  } catch {
    /* error 已在 store 中保存 */
  }
}

const syncToast = ref({ visible: false, message: '', type: 'success' })
let toastTimer: ReturnType<typeof setTimeout> | null = null

function showToast(message: string, type: 'success' | 'error' = 'success') {
  if (toastTimer) clearTimeout(toastTimer)
  syncToast.value = { visible: true, message, type }
  toastTimer = setTimeout(() => {
    syncToast.value.visible = false
  }, 3000)
}

// 注册全局同步反馈事件
function setupSyncToast() {
  window.addEventListener('sync:toast', ((event: CustomEvent) => {
    const { message, type } = event.detail
    showToast(message, type || 'success')
  }) as EventListener)
}

// 同步完成后增量刷新工作区数据（diff 对比，不重置状态）
function setupSyncDataRefresh() {
  window.addEventListener('sync-data-changed', async () => {
    await workspaceStore.syncRefresh()
  })
}
const toggleTopMenu = () => {
  topMenuOpen.value = !topMenuOpen.value
}

// 切换左侧侧边栏
const toggleLeftSidebar = () => {
  leftSidebarOpen.value = !leftSidebarOpen.value
}

// 切换缩略图栏
const toggleThumbnailSidebar = () => {
  thumbnailSidebarOpen.value = !thumbnailSidebarOpen.value
}

// 切换右侧侧边栏
const toggleRightSidebar = () => {
  rightSidebarOpen.value = !rightSidebarOpen.value
}

// 切换底部状态栏
const toggleStatusBar = () => {
  statusBarOpen.value = !statusBarOpen.value
}

// 左侧菜单栏 + 缩略图栏 四步循环：
// (左开,缩开) -> 收起缩略图 -> (左开,缩关) -> 收起左侧 -> (左关,缩关) -> 展开缩略图 -> (左关,缩开) -> 展开左侧 -> (左开,缩开)
const cycleLeftPanels = () => {
  if (leftSidebarOpen.value && thumbnailSidebarOpen.value) {
    thumbnailSidebarOpen.value = false
  } else if (leftSidebarOpen.value && !thumbnailSidebarOpen.value) {
    leftSidebarOpen.value = false
  } else if (!leftSidebarOpen.value && !thumbnailSidebarOpen.value) {
    thumbnailSidebarOpen.value = true
  } else {
    leftSidebarOpen.value = true
  }
}

// handleKeyDown 中直接使用从 shortcutUtils 导入的 isModKey

// 键盘快捷键处理函数
const handleKeyDown = (event: KeyboardEvent) => {
  // ESC 退出全屏
  if (event.key === 'Escape' && isFullscreen.value) {
    if (window.electronAPI && window.electronAPI.setFullscreen) {
      window.electronAPI.setFullscreen(false)
    }
    isFullscreen.value = false
    return
  }

  if (!isModKey(event)) return

  switch (event.key) {
    // 应用级快捷键（跨平台：Mac ⌘ / Win Ctrl）
    case 'n':
    case 'N':
      event.preventDefault()
      workspaceStore.createNewDocument()
      break
    case ',':
      event.preventDefault()
      openSettings('file')
      break
    case 'f':
    case 'F':
      event.preventDefault()
      // 触发编辑器的查找功能（Tiptap 内置）
      document.dispatchEvent(new KeyboardEvent('keydown', {
        key: 'f', code: 'KeyF', ctrlKey: true, bubbles: true
      }))
      break
    case 'h':
    case 'H':
      event.preventDefault()
      // 查找替换（Tiptap 查找替换扩展）
      break
    // 侧边栏切换
    case 'ArrowUp':
      event.preventDefault()
      toggleTopMenu()
      break
    case 'ArrowDown':
      event.preventDefault()
      toggleStatusBar()
      break
    case 'ArrowLeft':
      event.preventDefault()
      cycleLeftPanels()
      break
    case 'ArrowRight':
      event.preventDefault()
      toggleRightSidebar()
      break
  }
}

// 组件挂载时添加键盘事件监听
onMounted(() => {
  window.addEventListener('keydown', handleKeyDown);
  // 注册 macOS 原生菜单动作监听
  if (window.electronAPI && window.electronAPI.onMenuAction) {
    window.electronAPI.onMenuAction(handleMenuAction)
  }
  initApp();
});

// 组件卸载前移除键盘事件监听和菜单监听，防止内存泄漏
onBeforeUnmount(() => {
  window.removeEventListener('keydown', handleKeyDown);
  if (window.electronAPI && window.electronAPI.removeMenuActionListener) {
    window.electronAPI.removeMenuActionListener()
  }
});
</script>

<style scoped>
.app-container {
  width: 100vw;
  height: 100vh;
  display: flex;
  flex-direction: column;
  overflow: hidden;
}

.main-content {
  flex: 1;
  display: flex;
  overflow: hidden;
}

.sync-toast {
  position: fixed;
  top: 52px;
  left: 50%;
  transform: translateX(-50%);
  padding: 8px 20px;
  border-radius: 8px;
  font-size: 13px;
  z-index: 9999;
  pointer-events: none;
  box-shadow: 0 4px 12px rgba(0,0,0,0.15);
}
.sync-toast.success {
  background: #10b981;
  color: #fff;
}
.sync-toast.error {
  background: #ef4444;
  color: #fff;
}

.toast-fade-enter-active,
.toast-fade-leave-active {
  transition: all 0.3s ease;
}
.toast-fade-enter-from,
.toast-fade-leave-to {
  opacity: 0;
  transform: translateX(-50%) translateY(-10px);
}
</style>