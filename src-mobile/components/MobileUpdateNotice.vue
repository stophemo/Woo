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
const LAST_SUCCESSFUL_CHECK_KEY = 'woo-mobile-update-last-successful-check'
const LAST_AUTO_ATTEMPT_KEY = 'woo-mobile-update-last-auto-attempt'
const AUTO_CHECK_INTERVAL_MS = 6 * 60 * 60 * 1000
const MIN_AUTO_ATTEMPT_INTERVAL_MS = 60 * 1000
const RETRY_DELAYS_MS = [60 * 1000, 5 * 60 * 1000, 15 * 60 * 1000]
const updateInfo = ref<MobileUpdateInfo | null>(null)
const checking = ref(false)
let autoCheckTimer: ReturnType<typeof setTimeout> | null = null
let retryIndex = 0
let mounted = false

function readStoredTimestamp(key: string): number {
  const value = Number(localStorage.getItem(key))
  return Number.isFinite(value) && value > 0 ? value : 0
}

function writeStoredTimestamp(key: string, value: number) {
  localStorage.setItem(key, String(value))
}

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
    clearAutoCheckTimer()
    writeStoredTimestamp(LAST_SUCCESSFUL_CHECK_KEY, Date.now())
    retryIndex = 0
    scheduleAutomaticCheck(AUTO_CHECK_INTERVAL_MS, true)
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
    if (manual) {
      showToast(`检查更新失败：${describeError(error)}`)
    } else {
      scheduleRetry()
    }
  } finally {
    checking.value = false
  }
}

function clearAutoCheckTimer() {
  if (!autoCheckTimer) return
  clearTimeout(autoCheckTimer)
  autoCheckTimer = null
}

function scheduleAutomaticCheck(delay: number, resetRetries = false) {
  if (!mounted) return
  clearAutoCheckTimer()
  autoCheckTimer = setTimeout(() => {
    autoCheckTimer = null
    void runAutomaticCheck(resetRetries)
  }, delay)
}

function scheduleRetry() {
  const delay = RETRY_DELAYS_MS[retryIndex]
  if (delay === undefined) return
  retryIndex += 1
  scheduleAutomaticCheck(delay)
}

async function runAutomaticCheck(resetRetries = false) {
  if (document.visibilityState === 'hidden' || checking.value) return

  const now = Date.now()
  const lastSuccessfulCheck = readStoredTimestamp(LAST_SUCCESSFUL_CHECK_KEY)
  const timeSinceSuccessfulCheck = now - lastSuccessfulCheck
  if (
    lastSuccessfulCheck <= now
    && timeSinceSuccessfulCheck < AUTO_CHECK_INTERVAL_MS
  ) {
    scheduleAutomaticCheck(AUTO_CHECK_INTERVAL_MS - timeSinceSuccessfulCheck, true)
    return
  }

  const lastAttempt = readStoredTimestamp(LAST_AUTO_ATTEMPT_KEY)
  const timeSinceLastAttempt = now - lastAttempt
  if (
    lastAttempt <= now
    && timeSinceLastAttempt < MIN_AUTO_ATTEMPT_INTERVAL_MS
  ) {
    scheduleAutomaticCheck(MIN_AUTO_ATTEMPT_INTERVAL_MS - timeSinceLastAttempt, resetRetries)
    return
  }

  if (resetRetries) retryIndex = 0
  writeStoredTimestamp(LAST_AUTO_ATTEMPT_KEY, now)
  await checkUpdate(false)
}

function handleVisibilityChange() {
  if (document.visibilityState === 'visible') {
    void runAutomaticCheck(true)
  }
}

function handleOnline() {
  void runAutomaticCheck(true)
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
  mounted = true
  window.addEventListener('woo:mobile-check-update', handleManualCheck)
  window.addEventListener('online', handleOnline)
  document.addEventListener('visibilitychange', handleVisibilityChange)
  // 启动检查不阻塞应用初始化，也不会在失败时打扰用户。
  scheduleAutomaticCheck(2500, true)
})

onBeforeUnmount(() => {
  mounted = false
  window.removeEventListener('woo:mobile-check-update', handleManualCheck)
  window.removeEventListener('online', handleOnline)
  document.removeEventListener('visibilitychange', handleVisibilityChange)
  clearAutoCheckTimer()
})
</script>

<style scoped>
.mobile-update-notice {
  position: fixed;
  right: 12px;
  bottom: calc(74px + env(safe-area-inset-bottom, 0px));
  z-index: 101;
  display: flex;
  align-items: center;
  min-height: 32px;
  overflow: hidden;
  border: 1px solid var(--van-border-color);
  border-radius: 7px;
  background: color-mix(in srgb, var(--van-background-2) 95%, transparent);
  box-shadow: 0 6px 18px rgba(18, 37, 45, 0.1);
  backdrop-filter: blur(14px);
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
  min-height: 32px;
  padding: 0 9px 0 10px;
  font-size: 11px;
}

.mobile-update-dot {
  width: 6px;
  height: 6px;
  border-radius: 50%;
  background: var(--van-primary-color);
}

.mobile-update-ignore {
  width: 28px;
  min-height: 32px;
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
