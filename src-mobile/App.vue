<script setup lang="ts">
import { ref, watch, onMounted } from 'vue'
import { useRouter, useRoute } from 'vue-router'
import { showToast } from 'vant'
import { listen } from '@tauri-apps/api/event'
import { useAuthStore } from '../src/stores/auth'
import { useSyncStore } from '../src/stores/sync'
import { useWorkspaceStore } from '../src/stores/workspace'
import { useLockStore } from '../src/stores/lock'
import { useThemeStore } from '../src/stores/theme'

const router = useRouter()
const route = useRoute()
const authStore = useAuthStore()
const syncStore = useSyncStore()
const workspaceStore = useWorkspaceStore()
const lockStore = useLockStore()
const themeStore = useThemeStore()

const TAB_ROUTES = ['/', '/drafts', '/settings']
const active = ref(TAB_ROUTES.indexOf(route.path) >= 0 ? TAB_ROUTES.indexOf(route.path) : 0)

watch(() => route.path, (path) => {
  const idx = TAB_ROUTES.indexOf(path)
  if (idx >= 0) active.value = idx
})

function onTabChange(index: number) {
  router.push(TAB_ROUTES[index])
}

onMounted(async () => {
  // Tauri 后端事件 → DOM CustomEvent 桥（移动端此前缺失，导致同步状态永不刷新）。
  // 仅搬事件桥三段，不整体调用桌面 setupTauriBridge（其耦合窗口控制/electronAPI，移动端不需要）。
  try {
    listen<unknown>('sync-status', (e) => {
      window.dispatchEvent(new CustomEvent('sync-status', { detail: e.payload }))
    })
    listen<unknown>('sync-data-changed', (e) => {
      window.dispatchEvent(new CustomEvent('sync-data-changed', { detail: e.payload }))
    })
    listen<unknown>('sync:toast', (e) => {
      window.dispatchEvent(new CustomEvent('sync:toast', { detail: e.payload }))
    })
  } catch { /* 非 Tauri 环境（纯浏览器预览）忽略 */ }

  // 先注册监听，确保首次事件到达前已就绪
  syncStore.listen()
  window.addEventListener('sync-data-changed', () => {
    void workspaceStore.syncRefresh()
  })
  window.addEventListener('sync:toast', ((e: CustomEvent) => {
    const msg = e.detail?.message
    if (msg) showToast(msg)
  }) as EventListener)

  // 移动端进程易被系统回收：切后台/隐藏时强制落库，避免未保存内容丢失
  document.addEventListener('visibilitychange', () => {
    if (document.visibilityState === 'hidden') {
      void workspaceStore.flushPendingSave()
    }
  })

  // 恢复会话 + 拉一次首屏同步状态
  await authStore.bootstrap()
  try { await lockStore.bootstrap() } catch { /* ignore */ }
  try { await syncStore.refreshStatus() } catch { /* ignore */ }
})
</script>

<template>
  <van-config-provider :theme="themeStore.theme">
    <div class="mobile-app">
      <div class="page-content">
        <router-view />
      </div>
      <van-tabbar v-model="active" @change="onTabChange" safe-area-inset-bottom>
        <van-tabbar-item icon="notes-o">笔记</van-tabbar-item>
        <van-tabbar-item icon="edit-o">草稿</van-tabbar-item>
        <van-tabbar-item icon="setting-o">设置</van-tabbar-item>
      </van-tabbar>
    </div>
  </van-config-provider>
</template>

<style>
body {
  margin: 0;
  padding: 0;
  background: #f7f8fa;
}
.mobile-app {
  min-height: 100vh;
  background: #f7f8fa;
}
.page-content {
  padding-bottom: 50px;
}
/* 暗色主题：覆盖自定义背景（Vant 组件由 van-config-provider 处理） */
html[data-theme='dark'] body,
html[data-theme='dark'] .mobile-app {
  background: #0f0f0f;
}
html[data-theme='dark'] .editor-content {
  color: #e0e0e0;
}
</style>
