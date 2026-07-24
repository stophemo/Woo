<template>
  <transition name="mobile-update-fade">
    <aside v-if="updateInfo" class="mobile-update-notice" role="status" aria-live="polite">
      <button
        type="button"
        class="mobile-update-link"
        :aria-label="`下载 Woo v${updateInfo.version}`"
        @click="downloadUpdate"
      >
        <span class="mobile-update-dot" aria-hidden="true"></span>
        <span>新版本 v{{ updateInfo.version }}</span>
      </button>
      <button
        type="button"
        class="mobile-update-ignore"
        :aria-label="`忽略 v${updateInfo.version}`"
        @click="ignoreUpdate"
      >×</button>
    </aside>
  </transition>
</template>

<script setup lang="ts">
import { onBeforeUnmount, onMounted, ref } from 'vue'
import { showToast } from 'vant'
import {
  checkForMobileAppUpdate,
  openMobileUpdateDownload,
  type MobileUpdateInfo,
} from '../../src/services/mobileUpdater'

const IGNORED_VERSION_KEY = 'woo-ignored-update-version'
const updateInfo = ref<MobileUpdateInfo | null>(null)
const checking = ref(false)
let autoCheckTimer: ReturnType<typeof setTimeout> | null = null

function describeError(error: unknown): string {
  if (error instanceof Error && error.message) return error.message
  if (typeof error === 'string' && error.trim()) return error.trim()
  return '请稍后重试'
}

async function checkUpdate(manual: boolean) {
  if (checking.value) {
    if (manual) showToast('正在检查更新…')
    return
  }

  checking.value = true
  try {
    const update = await checkForMobileAppUpdate()
    if (!update) {
      updateInfo.value = null
      if (manual) showToast('已是最新版本')
      return
    }

    const ignoredVersion = localStorage.getItem(IGNORED_VERSION_KEY)
    if (!manual && ignoredVersion === update.version) return
    if (ignoredVersion && ignoredVersion !== update.version) {
      localStorage.removeItem(IGNORED_VERSION_KEY)
    }

    updateInfo.value = update
    if (manual) showToast(`发现 v${update.version}，已显示在右下角`)
  } catch (error: unknown) {
    if (manual) showToast(`检查更新失败：${describeError(error)}`)
  } finally {
    checking.value = false
  }
}

function handleManualCheck() {
  void checkUpdate(true)
}

function ignoreUpdate() {
  if (updateInfo.value) {
    localStorage.setItem(IGNORED_VERSION_KEY, updateInfo.value.version)
  }
  updateInfo.value = null
}

async function downloadUpdate() {
  if (!updateInfo.value) return
  try {
    await openMobileUpdateDownload(updateInfo.value.downloadUrl)
  } catch (error: unknown) {
    showToast(`无法打开下载链接：${describeError(error)}`)
  }
}

onMounted(() => {
  window.addEventListener('woo:mobile-check-update', handleManualCheck)
  // 启动检查不阻塞应用初始化，也不会在失败时打扰用户。
  autoCheckTimer = setTimeout(() => { void checkUpdate(false) }, 2500)
})

onBeforeUnmount(() => {
  window.removeEventListener('woo:mobile-check-update', handleManualCheck)
  if (autoCheckTimer) clearTimeout(autoCheckTimer)
})
</script>

<style scoped>
.mobile-update-notice {
  position: fixed;
  right: 8px;
  bottom: calc(76px + env(safe-area-inset-bottom, 0px));
  z-index: 101;
  display: flex;
  align-items: center;
  min-height: 28px;
  overflow: hidden;
  border: 1px solid var(--van-border-color);
  border-radius: 14px;
  background: color-mix(in srgb, var(--van-background-2) 92%, transparent);
  box-shadow: 0 2px 8px rgba(0, 0, 0, 0.1);
  opacity: 0.9;
  backdrop-filter: blur(8px);
}

.mobile-update-link,
.mobile-update-ignore {
  border: 0;
  background: transparent;
  color: var(--van-text-color-2);
}

.mobile-update-link {
  display: flex;
  align-items: center;
  gap: 5px;
  min-height: 28px;
  padding: 0 8px 0 9px;
  font-size: 11px;
}

.mobile-update-dot {
  width: 5px;
  height: 5px;
  border-radius: 50%;
  background: var(--van-primary-color);
}

.mobile-update-ignore {
  width: 26px;
  min-height: 28px;
  padding: 0;
  color: var(--van-text-color-3, var(--van-text-color-2));
  font-size: 15px;
}

.mobile-update-fade-enter-active,
.mobile-update-fade-leave-active {
  transition: opacity 0.2s ease, transform 0.2s ease;
}

.mobile-update-fade-enter-from,
.mobile-update-fade-leave-to {
  opacity: 0;
  transform: translateY(6px);
}
</style>
