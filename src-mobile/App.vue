<script setup lang="ts">
import { computed, ref, watch, onMounted } from 'vue'
import { useRouter, useRoute } from 'vue-router'
import { showToast } from 'vant'
import { listen } from '@tauri-apps/api/event'
import { useAuthStore } from '../src/stores/auth'
import { useSyncStore } from '../src/stores/sync'
import { useWorkspaceStore } from '../src/stores/workspace'
import { useLockStore } from '../src/stores/lock'
import { useThemeStore } from '../src/stores/theme'
import MobileUpdateNotice from './components/MobileUpdateNotice.vue'

const router = useRouter()
const route = useRoute()
const authStore = useAuthStore()
const syncStore = useSyncStore()
const workspaceStore = useWorkspaceStore()
const lockStore = useLockStore()
const themeStore = useThemeStore()

const vantTheme = computed<'light' | 'dark'>(() => (
  themeStore.isDarkTheme(themeStore.theme) ? 'dark' : 'light'
))

const mobileThemeVars = computed(() => {
  const palettes = {
    light: { primaryColor: '#3d8aa8', background: '#f4f7f8', background2: '#ffffff', textColor: '#24353d', textColor2: '#5d7078', borderColor: '#d1dde1' },
    dark: { primaryColor: '#7ab0e0', background: '#1b1c1e', background2: '#282a2d', textColor: '#e2dfd9', textColor2: '#a5a19b', borderColor: '#3b3d40' },
    ocean: { primaryColor: '#248ca0', background: '#edf6f7', background2: '#ffffff', textColor: '#1f3f46', textColor2: '#55747a', borderColor: '#c5dfe2' },
    forest: { primaryColor: '#4c9664', background: '#f1f6f1', background2: '#ffffff', textColor: '#293c2f', textColor2: '#607566', borderColor: '#cbdccc' },
    rose: { primaryColor: '#c76b7b', background: '#faf3f2', background2: '#ffffff', textColor: '#493537', textColor2: '#7c6264', borderColor: '#e5cfcd' },
  }
  return palettes[themeStore.theme]
})

const TAB_ROUTES = ['/', '/drafts', '/settings']
const active = ref(TAB_ROUTES.indexOf(route.path) >= 0 ? TAB_ROUTES.indexOf(route.path) : 0)
const showTabbar = computed(() => TAB_ROUTES.includes(route.path))

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
      void workspaceStore.flushPendingSave().catch(() => {})
    }
  })

  // 恢复会话后等待首次同步，再刷新当前移动端视图。
  await authStore.bootstrap()
  if (authStore.isLoggedIn) {
    const synced = await syncStore.triggerSync()
    if (!synced && syncStore.errorMsg) {
      showToast(syncStore.errorMsg)
    }
    if (route.path === '/') {
      await workspaceStore.openAllDocuments()
    } else {
      await workspaceStore.syncRefresh()
    }
  }
  try { await lockStore.bootstrap() } catch { /* ignore */ }
  try { await syncStore.refreshStatus() } catch { /* ignore */ }
})
</script>

<template>
  <van-config-provider :theme="vantTheme" :theme-vars="mobileThemeVars">
    <div class="mobile-app">
      <div class="page-content" :class="{ 'has-tabbar': showTabbar }">
        <router-view />
      </div>
      <MobileUpdateNotice />
      <van-tabbar v-if="showTabbar" v-model="active" @change="onTabChange" safe-area-inset-bottom>
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
  background: #f4f7f8;
}
.mobile-app {
  min-height: 100vh;
  background: var(--van-background);
  color: var(--van-text-color);
  transition: background-color 0.25s ease, color 0.25s ease;
}
.page-content {
  padding-bottom: env(safe-area-inset-bottom, 0px);
}
.page-content.has-tabbar {
  padding-bottom: calc(50px + env(safe-area-inset-bottom, 0px));
}
/* 暗色主题：覆盖自定义背景（Vant 组件由 van-config-provider 处理） */
html[data-theme='dark'] body {
  background: #0f0f0f;
}
html[data-theme='ocean'] body {
  background: #edf6f7;
}
html[data-theme='forest'] body {
  background: #f1f6f1;
}
html[data-theme='rose'] body {
  background: #faf3f2;
}
html[data-theme='dark'] .editor-content {
  color: #e0e0e0;
}
</style>
