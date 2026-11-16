<script setup lang="ts">
import { computed, ref, watch, onMounted } from 'vue'
import { useRouter, useRoute } from 'vue-router'
import { showToast } from 'vant'
import { listen } from '@tauri-apps/api/event'
import { useSyncStore } from '../src/stores/sync'
import { useWorkspaceStore } from '../src/stores/workspace'
import { useLockStore } from '../src/stores/lock'
import { useThemeStore, type ThemeMode } from '../src/stores/theme'
import MobileUpdateNotice from './components/MobileUpdateNotice.vue'
import MIcon from './components/MIcon.vue'

const router = useRouter()
const route = useRoute()
const syncStore = useSyncStore()
const workspaceStore = useWorkspaceStore()
const lockStore = useLockStore()
const themeStore = useThemeStore()

const vantTheme = computed<'light' | 'dark'>(() => (
  themeStore.isDarkTheme(themeStore.theme) ? 'dark' : 'light'
))

interface MobileThemePalette {
  [key: string]: string
  primaryColor: string
  background: string
  background2: string
  textColor: string
  textColor2: string
  borderColor: string
}

const MOBILE_THEME_PALETTES: Record<ThemeMode, MobileThemePalette> = {
  light: { primaryColor: '#3d8aa8', background: '#f4f7f8', background2: '#ffffff', textColor: '#24353d', textColor2: '#5d7078', borderColor: '#d1dde1' },
  paper: { primaryColor: '#4597b7', background: '#f2f6f7', background2: '#fcfdfd', textColor: '#203139', textColor2: '#526a74', borderColor: '#cdd9dd' },
  ocean: { primaryColor: '#248ca0', background: '#edf6f7', background2: '#ffffff', textColor: '#1f3f46', textColor2: '#55747a', borderColor: '#c5dfe2' },
  forest: { primaryColor: '#4c9664', background: '#f1f6f1', background2: '#ffffff', textColor: '#293c2f', textColor2: '#607566', borderColor: '#cbdccc' },
  rose: { primaryColor: '#c76b7b', background: '#faf3f2', background2: '#ffffff', textColor: '#493537', textColor2: '#7c6264', borderColor: '#e5cfcd' },
  dark: { primaryColor: '#7ab0e0', background: '#1b1c1e', background2: '#282a2d', textColor: '#e2dfd9', textColor2: '#a5a19b', borderColor: '#3b3d40' },
  ink: { primaryColor: '#df7657', background: '#151b19', background2: '#23302c', textColor: '#e6ece7', textColor2: '#abb9b1', borderColor: '#3a4a44' },
  graphite: { primaryColor: '#d6aa4f', background: '#17191a', background2: '#292c2d', textColor: '#e7e5df', textColor2: '#b5b1a7', borderColor: '#414647' },
  deepsea: { primaryColor: '#50b99f', background: '#0f181b', background2: '#203036', textColor: '#e2ecec', textColor2: '#a6b9b9', borderColor: '#375058' },
}

const mobileThemeVars = computed(() => MOBILE_THEME_PALETTES[themeStore.theme])

const TAB_ROUTES = ['/', '/drafts', '/settings']
const active = ref(TAB_ROUTES.indexOf(route.path) >= 0 ? TAB_ROUTES.indexOf(route.path) : 0)
const showTabbar = computed(() => TAB_ROUTES.includes(route.path))
const appReady = ref(false)

async function startSyncIfConfigured() {
  await syncStore.refreshStatus()
  if (syncStore.backend !== 'webdav' && syncStore.backend !== 'lan') return
  const synced = await syncStore.triggerSync()
  if (!synced && syncStore.errorMsg) showToast(syncStore.errorMsg)
}

watch(() => route.path, (path) => {
  const idx = TAB_ROUTES.indexOf(path)
  if (idx >= 0) active.value = idx
})

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

  // 单用户数据模式：直接打开本地 woo.db，不存在账号切换。
  appReady.value = true
  void lockStore.bootstrap().catch(() => {})
  // WebDAV/局域网启用时后台补一轮同步，不阻塞本地文稿。
  void startSyncIfConfigured()
})
</script>

<template>
  <van-config-provider :theme="vantTheme" :theme-vars="mobileThemeVars" theme-vars-scope="global">
    <div class="mobile-app">
      <div class="page-content" :class="{ 'has-tabbar': showTabbar }">
        <router-view v-if="appReady" />
      </div>
      <MobileUpdateNotice />
      <van-tabbar v-if="appReady && showTabbar" v-model="active" class="mobile-tabbar">
        <van-tabbar-item icon="notes-o" @click="router.push('/')">文稿</van-tabbar-item>
        <van-tabbar-item @click="router.push('/drafts')">
          <template #icon><span class="van-tabbar-item__icon"><MIcon name="pencil" :size="19" /></span></template>
          草稿
        </van-tabbar-item>
        <van-tabbar-item icon="user-o" @click="router.push('/settings')">我的</van-tabbar-item>
      </van-tabbar>
    </div>
  </van-config-provider>
</template>
