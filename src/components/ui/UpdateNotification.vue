<template>
  <Teleport to="body">
    <transition name="update-slide">
      <div v-if="visible" class="update-notification" :class="state" role="status" aria-live="polite">
        <!-- 检查中 -->
        <template v-if="state === 'checking'">
          <div class="update-icon spinning" aria-hidden="true">&#x21bb;</div>
          <span class="update-text">正在检查更新...</span>
        </template>

        <!-- 已是最新版本 -->
        <template v-else-if="state === 'up-to-date'">
          <div class="update-icon" aria-hidden="true">&#x2714;</div>
          <span class="update-text">已是最新版本</span>
        </template>

        <!-- 发现新版本 -->
        <template v-else-if="state === 'available'">
          <div class="update-icon" aria-hidden="true">&#x2b06;</div>
          <div class="update-info">
            <div class="update-title">新版本 v{{ updateVersion }} 可用</div>
            <div class="update-desc">当前 v{{ currentVersion }} · 可随时更新</div>
          </div>
          <button class="update-btn ignore" @click="ignoreUpdate">忽略此版本</button>
          <button class="update-btn primary" @click="installUpdate">立即更新</button>
          <button class="update-close" aria-label="稍后提醒" title="稍后提醒" @click="dismiss">&#x2715;</button>
        </template>

        <!-- 下载中 -->
        <template v-else-if="state === 'downloading'">
          <div class="update-icon spinning" aria-hidden="true">&#x21bb;</div>
          <div class="update-info">
            <div class="update-title">正在下载 v{{ updateVersion }}</div>
            <div
              class="update-progress"
              role="progressbar"
              aria-label="更新下载进度"
              aria-valuemin="0"
              aria-valuemax="100"
              :aria-valuenow="downloadPercent"
            >
              <span :style="{ width: `${downloadPercent}%` }"></span>
            </div>
            <div class="update-desc">{{ downloadPercent > 0 ? `${downloadPercent}%` : '正在连接下载服务器...' }}</div>
          </div>
        </template>

        <!-- 安装与重启 -->
        <template v-else-if="state === 'installing' || state === 'restarting'">
          <div class="update-icon spinning" aria-hidden="true">&#x21bb;</div>
          <div class="update-info">
            <div class="update-title">{{ state === 'installing' ? '正在安装更新' : '正在重新启动' }}</div>
            <div class="update-desc">请勿关闭 Woo</div>
          </div>
        </template>

        <!-- 网络错误 -->
        <template v-else-if="state === 'error'">
          <div class="update-icon" aria-hidden="true">&#x26a0;</div>
          <div class="update-info">
            <div class="update-title">{{ errorTitle }}</div>
            <div class="update-desc">{{ errorMessage || '无法连接 GitHub' }}</div>
          </div>
          <button class="update-btn" @click="check(false)">重试</button>
          <button class="update-close" aria-label="关闭更新提示" title="关闭" @click="dismiss">&#x2715;</button>
        </template>
      </div>
    </transition>
  </Teleport>
</template>

<script setup lang="ts">
import { onBeforeUnmount, ref } from 'vue'
import {
  checkForAppUpdate,
  clearPendingAppUpdate,
  installPendingAppUpdate,
  relaunchAfterUpdate,
} from '../../services/appUpdater'
import { log } from '../../services/logger'

type UpdateState =
  | 'idle'
  | 'checking'
  | 'up-to-date'
  | 'available'
  | 'downloading'
  | 'installing'
  | 'restarting'
  | 'error'

const visible = ref(false)
const state = ref<UpdateState>('idle')
const updateVersion = ref('')
const currentVersion = ref('')
const downloadPercent = ref(0)
const errorTitle = ref('检查更新失败')
const errorMessage = ref('')
const IGNORED_VERSION_KEY = 'woo-ignored-update-version'

let autoDismissTimer: ReturnType<typeof setTimeout> | null = null
let checkPromise: Promise<void> | null = null
let manualCheckRequested = false

function describeError(error: unknown, fallback: string): string {
  if (error instanceof Error && error.message) return error.message
  if (typeof error === 'string' && error.trim()) return error.trim()
  return fallback
}

function dismiss() {
  if (autoDismissTimer) clearTimeout(autoDismissTimer)
  visible.value = false
  state.value = 'idle'
  void clearPendingAppUpdate()
}

function ignoreUpdate() {
  if (updateVersion.value) {
    localStorage.setItem(IGNORED_VERSION_KEY, updateVersion.value)
  }
  dismiss()
}

async function runCheck() {
  try {
    const update = await checkForAppUpdate()
    if (update) {
      const ignoredVersion = localStorage.getItem(IGNORED_VERSION_KEY)
      if (!manualCheckRequested && ignoredVersion === update.version) {
        state.value = 'idle'
        visible.value = false
        await clearPendingAppUpdate()
        return
      }
      if (ignoredVersion && ignoredVersion !== update.version) {
        localStorage.removeItem(IGNORED_VERSION_KEY)
      }
      updateVersion.value = update.version
      currentVersion.value = update.currentVersion
      state.value = 'available'
      visible.value = true
    } else {
      state.value = 'up-to-date'
      if (manualCheckRequested) {
        visible.value = true
        autoDismissTimer = setTimeout(() => {
          visible.value = false
          state.value = 'idle'
        }, 4000)
      } else {
        visible.value = false
      }
    }
  } catch (error: unknown) {
    const message = describeError(error, '检查更新异常')
    log.app.warn('[Updater] 检查更新失败:', message)
    if (!manualCheckRequested) {
      visible.value = false
      state.value = 'idle'
      return
    }
    errorTitle.value = '检查更新失败'
    errorMessage.value = message
    state.value = 'error'
    visible.value = true
    autoDismissTimer = setTimeout(() => {
      visible.value = false
      state.value = 'idle'
    }, 10_000)
  }
}

function check(silent = false): Promise<void> {
  // A delayed startup check must not overwrite a completed manual result.
  if (silent && state.value !== 'idle') return Promise.resolve()
  if (state.value === 'downloading' || state.value === 'installing' || state.value === 'restarting') {
    return Promise.resolve()
  }
  if (autoDismissTimer) clearTimeout(autoDismissTimer)
  if (!silent) {
    manualCheckRequested = true
    visible.value = true
  }
  if (checkPromise) return checkPromise

  manualCheckRequested = !silent
  errorMessage.value = ''
  state.value = 'checking'
  visible.value = !silent
  checkPromise = runCheck().finally(() => {
    checkPromise = null
    manualCheckRequested = false
  })
  return checkPromise
}

async function installUpdate() {
  state.value = 'downloading'
  downloadPercent.value = 0
  try {
    await installPendingAppUpdate((progress) => {
      if (progress.downloadFinished) {
        state.value = 'installing'
        downloadPercent.value = 100
        return
      }
      if (progress.totalBytes && progress.totalBytes > 0) {
        downloadPercent.value = Math.min(
          99,
          Math.round((progress.downloadedBytes / progress.totalBytes) * 100)
        )
      }
    })
    state.value = 'restarting'
    await relaunchAfterUpdate()
  } catch (error: unknown) {
    errorTitle.value = '安装更新失败'
    errorMessage.value = describeError(error, '安装更新失败')
    state.value = 'error'
  }
}

defineExpose({ check })

onBeforeUnmount(() => {
  if (autoDismissTimer) clearTimeout(autoDismissTimer)
  void clearPendingAppUpdate()
})
</script>

<style scoped>
.update-notification {
  position: fixed;
  bottom: 38px;
  right: 14px;
  z-index: 10000;
  display: flex;
  align-items: center;
  gap: 8px;
  padding: 9px 11px;
  background: var(--bg-elevated, #ffffff);
  border: 1px solid var(--border-primary, #d8d4cd);
  border-radius: 8px;
  box-shadow: var(--shadow-dropdown, 0 4px 12px rgba(0, 0, 0, 0.1));
  font-size: 13px;
  width: min(340px, calc(100vw - 28px));
  max-width: calc(100vw - 32px);
  backdrop-filter: blur(8px);
}

.update-notification.available {
  width: auto;
  min-width: min(390px, calc(100vw - 28px));
  max-width: min(430px, calc(100vw - 28px));
}

.update-icon {
  font-size: 15px;
  width: 20px;
  height: 20px;
  display: flex;
  align-items: center;
  justify-content: center;
  flex-shrink: 0;
}

.spinning {
  animation: spin 1s linear infinite;
}

@keyframes spin {
  from { transform: rotate(0deg); }
  to { transform: rotate(360deg); }
}

.update-notification.error .update-icon {
  color: #ef4444;
}

.update-notification.up-to-date .update-icon {
  color: #10b981;
}

.update-info {
  flex: 1;
  min-width: 0;
}

.update-progress {
  width: 180px;
  height: 4px;
  margin: 6px 0 4px;
  overflow: hidden;
  background: var(--bg-hover, #e6e3de);
  border-radius: 2px;
}

.update-progress span {
  display: block;
  height: 100%;
  background: var(--accent, #5a9acf);
  transition: width 0.2s ease;
}

.update-title {
  font-weight: 600;
  color: var(--text-primary, #37342f);
  margin-bottom: 2px;
}

.update-desc {
  font-size: 12px;
  color: var(--text-muted, #9a9690);
  overflow-wrap: anywhere;
}

.update-btn {
  padding: 5px 9px;
  border: 1px solid var(--accent, #5a9acf);
  background: transparent;
  color: var(--accent, #5a9acf);
  border-radius: 6px;
  cursor: pointer;
  font-size: 12px;
  font-weight: 500;
  white-space: nowrap;
  transition: all 0.15s ease;
}

.update-btn:hover {
  background: var(--accent-light, rgba(90, 154, 207, 0.12));
}

.update-btn.ignore {
  border-color: transparent;
  color: var(--text-secondary, #68645e);
}

.update-btn.ignore:hover {
  background: var(--bg-hover, #e6e3de);
  color: var(--text-primary, #37342f);
}

.update-btn.primary {
  color: #ffffff;
  background: var(--accent, #5a9acf);
}

.update-btn.primary:hover {
  background: var(--accent-hover, #437fad);
}

.update-close {
  width: 22px;
  height: 22px;
  display: flex;
  align-items: center;
  justify-content: center;
  border: none;
  background: transparent;
  color: var(--text-muted, #9a9690);
  border-radius: 4px;
  cursor: pointer;
  font-size: 12px;
  flex-shrink: 0;
  transition: all 0.15s ease;
}

.update-close:hover {
  background: var(--bg-hover, #e6e3de);
  color: var(--text-primary, #37342f);
}

/* 过渡动画 */
.update-slide-enter-active {
  transition: all 0.3s ease;
}

.update-slide-leave-active {
  transition: all 0.2s ease;
}

.update-slide-enter-from {
  opacity: 0;
  transform: translateY(20px);
}

.update-slide-leave-to {
  opacity: 0;
  transform: translateY(20px);
}

@media (max-width: 520px) {
  .update-notification.available {
    min-width: 0;
    flex-wrap: wrap;
  }

  .update-notification.available .update-info {
    flex: 1 1 calc(100% - 48px);
  }

  .update-notification.available .update-btn {
    flex: 1 1 auto;
  }
}
</style>
