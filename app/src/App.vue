<template>
  <div class="app-container">
    <!-- 顶部菜单栏 -->
    <TopMenu 
      :is-open="topMenuOpen"
      @toggle-left-sidebar="toggleLeftSidebar"
      @toggle-thumbnail-sidebar="toggleThumbnailSidebar"
      @toggle-right-sidebar="toggleRightSidebar"
      @open-settings="showSettings = true"
      @open-login="showLogin = true"
      @toggle-top-menu="toggleTopMenu"
    />
    
    <!-- 主内容区域 -->
    <div class="main-content">
      <!-- 左侧菜单列 -->
      <LeftSidebar :is-open="leftSidebarOpen" />
      
      <!-- 中间缩略图列 -->
      <ThumbnailColumn :is-open="thumbnailSidebarOpen" />
      
      <!-- 中央编辑区域 -->
      <EditArea />
      
      <!-- 右侧AI对话区域 -->
      <RightSidebar :is-open="rightSidebarOpen" @open-settings="showSettings = true" />
    </div>

    <!-- 设置弹窗 -->
    <SettingsDialog :visible="showSettings" @close="showSettings = false" />

    <!-- 登录弹窗 -->
    <LoginDialog :visible="showLogin" @close="handleLoginClose" @login-success="handleLoginSuccess" />
  </div>
</template>

<script setup lang="ts">
import { ref, onMounted, onBeforeUnmount } from 'vue'
import TopMenu from './components/layout/TopMenu.vue'
import LeftSidebar from './components/layout/LeftSidebar.vue'
import ThumbnailColumn from './components/layout/ThumbnailColumn.vue'
import EditArea from './components/layout/EditArea.vue'
import RightSidebar from './components/layout/RightSidebar.vue'
import SettingsDialog from './components/layout/SettingsDialog.vue'
import LoginDialog from './components/layout/LoginDialog.vue'
import { useThemeStore } from './stores/theme'
import { useAuthStore } from './stores/auth'
import { useWorkspaceStore } from './stores/workspace'

// 初始化主题（确保 data-theme 属性在应用启动时就被设置到 <html>）
useThemeStore()

const authStore = useAuthStore()
const workspaceStore = useWorkspaceStore()

// 侧边栏状态
const leftSidebarOpen = ref(true)
const thumbnailSidebarOpen = ref(true)
const rightSidebarOpen = ref(true)
const showSettings = ref(false)
const showLogin = ref(false)
const topMenuOpen = ref(true)

// 登录成功后拉取数据
async function handleLoginSuccess() {
  await authStore.fetchMe()
  await workspaceStore.bootstrap()
}

function handleLoginClose() {
  showLogin.value = false
}

// 监听 Token 失效，自动弹出登录
function onUnauthorized() {
  authStore.logout()
  workspaceStore.reset()
  showLogin.value = true
}

// 启动流程：有 Token 则拉用户+目录；否则弹出登录
async function initApp() {
  if (authStore.isLoggedIn) {
    const ok = await authStore.fetchMe()
    if (ok) {
      try {
        await workspaceStore.bootstrap()
      } catch {
        /* error 已在 store 中保存 */
      }
      return
    }
  }
  showLogin.value = true
}

// 切换顶部菜单栏
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

// 键盘快捷键处理函数
const handleKeyDown = (event: KeyboardEvent) => {
  // 快捷键 Ctrl+` 切换顶部菜单栏
  if (event.ctrlKey && event.key === '`') {
    event.preventDefault()
    toggleTopMenu()
    return
  }
  // 检查是否按下 Ctrl 键
  if (event.ctrlKey) {
    switch (event.key) {
      case '1':
        event.preventDefault(); // 阻止默认行为（如切换标签页）
        toggleLeftSidebar();
        break;
      case '2':
        event.preventDefault();
        toggleThumbnailSidebar();
        break;
      case '3':
        event.preventDefault();
        toggleRightSidebar();
        break;
    }
  }
}

// 组件挂载时添加键盘事件监听
onMounted(() => {
  window.addEventListener('keydown', handleKeyDown);
  window.addEventListener('auth:unauthorized', onUnauthorized as EventListener);
  initApp();
});

// 组件卸载前移除键盘事件监听，防止内存泄漏
onBeforeUnmount(() => {
  window.removeEventListener('keydown', handleKeyDown);
  window.removeEventListener('auth:unauthorized', onUnauthorized as EventListener);
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
</style>