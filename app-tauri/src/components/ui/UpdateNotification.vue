<template>
  <transition name="update-slide">
    <div v-if="visible" class="update-notification" :class="state">
      <!-- 检查中 -->
      <template v-if="state === 'checking'">
        <div class="update-icon spinning">&#x21bb;</div>
        <span class="update-text">正在检查更新...</span>
      </template>

      <!-- 已是最新版本 -->
      <template v-else-if="state === 'up-to-date'">
        <div class="update-icon">&#x2714;</div>
        <span class="update-text">已是最新版本</span>
      </template>

      <!-- 发现新版本 -->
      <template v-else-if="state === 'available'">
        <div class="update-icon">&#x2b06;</div>
        <div class="update-info">
          <div class="update-title">发现新版本 v{{ updateVersion }}</div>
          <div class="update-desc">浏览器打开后，请下载对应安装包</div>
        </div>
        <button class="update-btn" @click="goDownload">前往下载</button>
        <button class="update-close" @click="dismiss">&#x2715;</button>
      </template>

      <!-- 网络错误 -->
      <template v-else-if="state === 'error'">
        <div class="update-icon">&#x26a0;</div>
        <div class="update-info">
          <div class="update-title">检查更新失败</div>
          <div class="update-desc">{{ errorMessage || '无法连接 GitHub' }}</div>
        </div>
        <button class="update-btn" @click="check">重试</button>
        <button class="update-close" @click="dismiss">&#x2715;</button>
      </template>
    </div>
  </transition>
</template>

<script setup lang="ts">
import { ref } from 'vue'

type UpdateState = 'idle' | 'checking' | 'up-to-date' | 'available' | 'error'

const visible = ref(false)
const state = ref<UpdateState>('idle')
const updateVersion = ref('')
const downloadUrl = ref('')
const errorMessage = ref('')

let autoDismissTimer: ReturnType<typeof setTimeout> | null = null

function dismiss() {
  if (autoDismissTimer) clearTimeout(autoDismissTimer)
  visible.value = false
}

async function check() {
  if (!window.electronAPI) return

  errorMessage.value = ''
  state.value = 'checking'
  visible.value = true

  try {
    const result = await window.electronAPI.checkForUpdates()

    if (result.error) {
      errorMessage.value = result.error
      state.value = 'error'
      return
    }

    if (result.hasUpdate && result.version && result.downloadUrl) {
      updateVersion.value = result.version
      downloadUrl.value = result.downloadUrl
      state.value = 'available'
    } else {
      state.value = 'up-to-date'
      autoDismissTimer = setTimeout(() => { visible.value = false }, 2000)
    }
  } catch (e: any) {
    errorMessage.value = e?.message || '检查更新异常'
    state.value = 'error'
  }
}

function goDownload() {
  if (downloadUrl.value && window.electronAPI) {
    window.electronAPI.openExternalLink(downloadUrl.value)
  }
  visible.value = false
}

defineExpose({ check })
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
  max-width: 360px;
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

.update-title {
  font-weight: 600;
  color: var(--text-primary, #37342f);
  margin-bottom: 2px;
}

.update-desc {
  font-size: 12px;
  color: var(--text-muted, #9a9690);
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
