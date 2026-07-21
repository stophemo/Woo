<template>
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
          <div class="update-title">发现新版本 v{{ updateVersion }}</div>
          <div class="update-desc">当前 v{{ currentVersion }}，更新完成后将自动重启</div>
        </div>
        <button class="update-btn primary" @click="installUpdate">立即更新</button>
        <button class="update-close" aria-label="关闭更新提示" title="关闭" @click="dismiss">&#x2715;</button>
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
          <div class="update-title">检查更新失败</div>
          <div class="update-desc">{{ errorMessage || '无法连接 GitHub' }}</div>
        </div>
        <button class="update-btn" @click="check(false)">重试</button>
        <button class="update-close" aria-label="关闭更新提示" title="关闭" @click="dismiss">&#x2715;</button>
      </template>
    </div>
  </transition>
</template>

<script setup lang="ts">
import { onBeforeUnmount, ref } from 'vue'
import {
  checkForAppUpdate,
  clearPendingAppUpdate,
  installPendingAppUpdate,
  relaunchAfterUpdate,
} from '../../services/appUpdater'

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
const errorMessage = ref('')

let autoDismissTimer: ReturnType<typeof setTimeout> | null = null

function dismiss() {
  if (autoDismissTimer) clearTimeout(autoDismissTimer)
  visible.value = false
  void clearPendingAppUpdate()
}

async function check(silent = false) {
  if (state.value === 'downloading' || state.value === 'installing' || state.value === 'restarting') return
  if (autoDismissTimer) clearTimeout(autoDismissTimer)
  errorMessage.value = ''
  state.value = 'checking'
  visible.value = !silent

  try {
    const update = await checkForAppUpdate()
    if (update) {
      updateVersion.value = update.version
      currentVersion.value = update.currentVersion
      state.value = 'available'
      visible.value = true
    } else {
      state.value = 'up-to-date'
      if (!silent) {
        visible.value = true
        autoDismissTimer = setTimeout(() => { visible.value = false }, 2000)
      }
    }
  } catch (error: unknown) {
    if (silent) {
      visible.value = false
      state.value = 'idle'
      return
    }
    errorMessage.value = error instanceof Error ? error.message : '检查更新异常'
    state.value = 'error'
    visible.value = true
  }
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
    errorMessage.value = error instanceof Error ? error.message : '安装更新失败'
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
  bottom: 20px;
  right: 20px;
  z-index: 10000;
  display: flex;
  align-items: center;
  gap: 10px;
  padding: 12px 16px;
  background: var(--bg-elevated, #ffffff);
  border: 1px solid var(--border-primary, #d8d4cd);
  border-radius: 10px;
  box-shadow: var(--shadow-dropdown, 0 4px 12px rgba(0, 0, 0, 0.1));
  font-size: 13px;
  width: min(360px, calc(100vw - 32px));
  max-width: calc(100vw - 32px);
  backdrop-filter: blur(8px);
}

.update-icon {
  font-size: 18px;
  width: 28px;
  height: 28px;
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
  padding: 6px 14px;
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
</style>
