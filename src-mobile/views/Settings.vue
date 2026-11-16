<script setup lang="ts">
import { ref, reactive, computed, onMounted, onBeforeUnmount } from 'vue'
import { useRouter } from 'vue-router'
import { showToast } from 'vant'
import { getVersion } from '@tauri-apps/api/app'
import { useSyncStore } from '../../src/stores/sync'
import { useLockStore } from '../../src/stores/lock'
import { useThemeStore } from '../../src/stores/theme'
import { editorFontId, editorFontOptions } from '../../src/config/editorFonts'
import {
  DEFAULT_EDITOR_SHORTCUTS,
  editorShortcutEntries,
  resetEditorShortcuts,
  setEditorShortcut,
  shortcutConflicts,
  shortcutForDisplay,
  shortcutFromKeyboardEvent,
  type EditorShortcutAction,
} from '../../src/config/editorShortcuts'

const router = useRouter()
const appVersion = ref(__APP_VERSION__)
const syncStore = useSyncStore()
const lockStore = useLockStore()
const themeStore = useThemeStore()
const currentFontLabel = computed(() => editorFontOptions.find(option => option.id === editorFontId.value)?.label ?? '文楷')

const shortcutEntries = computed(() => editorShortcutEntries())
const showShortcutSettings = ref(false)
const recordingAction = ref<EditorShortcutAction | null>(null)
const shortcutError = ref('')

onMounted(async () => {
  window.addEventListener('keydown', captureShortcut, true)
  void syncStore.refreshStatus()
  try {
    appVersion.value = await getVersion()
  } catch {
    // 浏览器预览环境没有 Tauri API，继续显示构建时版本。
  }
})

onBeforeUnmount(() => {
  window.removeEventListener('keydown', captureShortcut, true)
})

function startRecording(action: EditorShortcutAction) {
  recordingAction.value = action
  shortcutError.value = ''
}

function cancelRecording() {
  recordingAction.value = null
}

function closeShortcutSettings() {
  showShortcutSettings.value = false
  cancelRecording()
}

function captureShortcut(event: KeyboardEvent) {
  const action = recordingAction.value
  if (!action) return
  event.preventDefault()
  event.stopPropagation()
  if (event.key === 'Escape') {
    cancelRecording()
    return
  }
  const shortcut = shortcutFromKeyboardEvent(event)
  if (!shortcut) return
  const conflict = shortcutConflicts(action, shortcut)
  if (conflict) {
    shortcutError.value = `与“${shortcutEntries.value.find(item => item.action === conflict)?.label || conflict}”冲突`
    return
  }
  setEditorShortcut(action, shortcut)
  shortcutError.value = ''
  recordingAction.value = null
}

function resetShortcut(action: EditorShortcutAction) {
  setEditorShortcut(action, DEFAULT_EDITOR_SHORTCUTS[action])
  shortcutError.value = ''
}

function resetAllShortcuts() {
  resetEditorShortcuts()
  shortcutError.value = ''
  recordingAction.value = null
}

async function onSyncNow() {
  if (syncStore.backend !== 'webdav' && syncStore.backend !== 'lan') {
    showToast('请先配置 WebDAV 同步')
    return
  }
  const ok = await syncStore.triggerSync()
  showToast(ok ? '同步完成' : (syncStore.errorMsg || '同步失败'))
}

function onCheckUpdate() {
  showToast('正在检查更新…')
  window.dispatchEvent(new CustomEvent('woo:mobile-check-update'))
}

// ------- 密码锁 -------
const showPwdDialog = ref(false)
const pwd = reactive({ value: '', confirm: '' })

function openPwdDialog() {
  pwd.value = ''
  pwd.confirm = ''
  showPwdDialog.value = true
}

// 返回 false 阻止对话框关闭（校验不通过时保持打开）
async function beforePwdClose(action: string) {
  if (action !== 'confirm') return true
  if (pwd.value.length < 4) {
    showToast('密码至少 4 位')
    return false
  }
  if (pwd.value !== pwd.confirm) {
    showToast('两次输入不一致')
    return false
  }
  try {
    const existed = lockStore.hasPassword
    await lockStore.setPassword(pwd.value)
    showToast(existed ? '密码已更新' : '密码锁已设置')
    return true
  } catch (e: any) {
    showToast(e?.message || '设置失败')
    return false
  }
}
</script>

<template>
  <div class="settings-page">
    <header class="w-head">
      <div class="w-head__row">
        <div class="w-head__titles">
          <span class="w-head__kicker">无我笔记</span>
          <h1 class="w-head__title">我的</h1>
        </div>
      </div>
      <p class="w-head__meta">Woo v{{ appVersion }}</p>
    </header>

    <!-- WebDAV 同步 -->
    <van-cell-group inset title="WebDAV 同步">
      <van-cell
        icon="exchange"
        title="同步设置"
        :label="syncStore.backend === 'webdav' || syncStore.backend === 'lan' ? '已连接，自动增量同步' : '支持坚果云、NAS、Nextcloud 或局域网'"
        :value="syncStore.backend === 'webdav' || syncStore.backend === 'lan' ? '生效中' : '未启用'"
        is-link
        @click="router.push('/webdav')"
      />
      <van-cell icon="exchange" title="同步状态">
        <template #value>
          <span>{{ syncStore.lastSyncLabel }}</span>
          <van-tag v-if="syncStore.pendingChanges > 0" type="warning" class="pending-tag">
            待同步 {{ syncStore.pendingChanges }}
          </van-tag>
        </template>
      </van-cell>
      <van-cell
        icon="replay"
        title="立即同步"
        is-link
        :label="syncStore.backend === 'off' ? '配置 WebDAV 后可用' : ''"
        @click="onSyncNow"
      >
        <template #value>
          <van-loading v-if="syncStore.isSyncing" size="16" />
        </template>
      </van-cell>
    </van-cell-group>

    <!-- 数据 -->
    <van-cell-group inset title="数据">
      <van-cell title="回收站" is-link icon="delete-o" @click="router.push('/trash')" />
    </van-cell-group>

    <!-- 编辑 -->
    <van-cell-group inset title="编辑">
      <van-cell
        icon="edit"
        title="编辑字体"
        :value="currentFontLabel"
        is-link
        @click="router.push('/font')"
      />
      <van-cell
        icon="edit"
        title="编辑器快捷键"
        value="可自定义"
        is-link
        @click="showShortcutSettings = true"
      />
    </van-cell-group>

    <!-- 安全 -->
    <van-cell-group inset title="安全">
      <van-cell
        icon="lock"
        :title="lockStore.hasPassword ? '修改密码锁' : '设置密码锁'"
        is-link
        :label="lockStore.hasPassword ? '已启用，可在笔记列表滑动加锁' : '设置后可对笔记加锁隐藏'"
        @click="openPwdDialog"
      />
    </van-cell-group>

    <!-- 外观 -->
    <van-cell-group inset title="外观">
      <div class="mobile-theme-grid" role="radiogroup" aria-label="选择主题">
        <button
          v-for="option in themeStore.themeOptions"
          :key="option.id"
          type="button"
          class="mobile-theme-option"
          :class="{ selected: themeStore.theme === option.id }"
          role="radio"
          :aria-checked="themeStore.theme === option.id"
          @click="themeStore.theme = option.id"
        >
          <span class="mobile-theme-swatch" aria-hidden="true">
            <span v-for="color in option.colors" :key="color" :style="{ backgroundColor: color }"></span>
          </span>
          <span class="mobile-theme-name">{{ option.label }}</span>
          <van-icon v-if="themeStore.theme === option.id" name="success" class="theme-check" aria-hidden="true" />
        </button>
      </div>
    </van-cell-group>

    <!-- 关于 -->
    <van-cell-group inset title="关于">
      <van-cell icon="info-o" title="版本" :value="`v${appVersion}`" />
      <van-cell icon="upgrade" title="检查更新" is-link @click="onCheckUpdate" />
      <van-cell icon="link-o" title="项目主页" value="GitHub" is-link @click="showToast('打开链接')" />
    </van-cell-group>

    <!-- 密码锁设置对话框 -->
    <van-dialog
      v-model:show="showPwdDialog"
      :title="lockStore.hasPassword ? '修改密码锁' : '设置密码锁'"
      show-cancel-button
      :before-close="beforePwdClose"
    >
      <div class="pwd-fields">
        <van-field v-model="pwd.value" type="password" label="密码" placeholder="至少 4 位" />
        <van-field v-model="pwd.confirm" type="password" label="确认" placeholder="再次输入" />
      </div>
    </van-dialog>

    <!-- 移动端快捷键设置（连接实体键盘后录入组合键） -->
    <van-popup v-model:show="showShortcutSettings" position="bottom" round :style="{ maxHeight: '82vh' }" @closed="cancelRecording">
      <div class="shortcut-sheet">
        <div class="shortcut-sheet-header">
          <h3>编辑器快捷键</h3>
          <button type="button" class="shortcut-sheet-close" title="关闭" aria-label="关闭" @click="closeShortcutSettings">
            <van-icon name="cross" />
          </button>
        </div>
        <p class="shortcut-sheet-hint">连接实体键盘后，点击一项并按下组合键即可修改。</p>
        <div class="shortcut-sheet-list">
          <div v-for="entry in shortcutEntries" :key="entry.action" class="shortcut-sheet-row">
            <span class="shortcut-sheet-label">{{ entry.label }}</span>
            <button
              type="button"
              class="shortcut-sheet-value"
              :class="{ recording: recordingAction === entry.action }"
              @click="startRecording(entry.action)"
            >
              {{ recordingAction === entry.action ? '请按组合键' : shortcutForDisplay(entry.shortcut) }}
            </button>
            <button
              v-if="entry.shortcut !== entry.defaultShortcut"
              type="button"
              class="shortcut-sheet-reset"
              title="恢复默认快捷键"
              aria-label="恢复默认快捷键"
              @click="resetShortcut(entry.action)"
            >
              <van-icon name="replay" />
            </button>
          </div>
        </div>
        <p v-if="shortcutError" class="shortcut-sheet-error">{{ shortcutError }}</p>
        <van-button block plain type="primary" class="shortcut-reset-all" @click="resetAllShortcuts">恢复全部默认</van-button>
      </div>
    </van-popup>
  </div>
</template>

<style scoped>
.settings-page {
  min-height: 100vh;
  min-height: 100dvh;
  padding-bottom: 24px;
  background: var(--van-background);
}

.settings-page :deep(.van-cell__left-icon) {
  margin-right: 12px;
  color: var(--van-primary-color);
  font-size: 18px;
}

.settings-page :deep(.van-cell__title) {
  color: var(--van-text-color);
  font-size: 14px;
}

.settings-page :deep(.van-cell__value) {
  max-width: 52%;
  color: var(--van-text-color-2);
  font-size: 12px;
}

.pending-tag {
  margin-left: 8px;
}

.mobile-theme-grid {
  display: grid;
  grid-template-columns: repeat(2, minmax(0, 1fr));
  gap: 9px;
  padding: 13px;
}

.mobile-theme-option {
  position: relative;
  display: flex;
  min-width: 0;
  align-items: center;
  gap: 9px;
  min-height: 54px;
  padding: 8px 10px;
  border: 1px solid var(--van-border-color);
  border-radius: 7px;
  background: color-mix(in srgb, var(--van-background) 58%, var(--van-background-2));
  color: var(--van-text-color);
  font-size: 12px;
  text-align: left;
}

.mobile-theme-option.selected {
  border-color: var(--van-primary-color);
  background: color-mix(in srgb, var(--van-primary-color) 7%, var(--van-background-2));
  box-shadow: 0 0 0 1px color-mix(in srgb, var(--van-primary-color) 12%, transparent);
}

.mobile-theme-swatch {
  display: flex;
  width: 36px;
  height: 34px;
  flex-shrink: 0;
  overflow: hidden;
  border: 1px solid var(--van-border-color);
  border-radius: 5px;
}

.mobile-theme-swatch span {
  flex: 1;
}

.mobile-theme-name {
  overflow: hidden;
  font-weight: 600;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.theme-check {
  margin-left: auto;
  color: var(--van-primary-color);
  font-size: 14px;
}

.pwd-fields {
  padding: 8px 0;
}

.shortcut-sheet {
  max-height: 82vh;
  overflow-y: auto;
  padding: 14px 14px calc(18px + env(safe-area-inset-bottom, 0px));
}

.shortcut-sheet-header {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 12px;
}

.shortcut-sheet-header h3 {
  margin: 0;
  color: var(--van-text-color);
  font-size: 17px;
}

.shortcut-sheet-close {
  display: inline-flex;
  width: 32px;
  height: 32px;
  align-items: center;
  justify-content: center;
  padding: 0;
  border: 0;
  border-radius: 6px;
  background: transparent;
  color: var(--van-text-color-2);
}

.shortcut-sheet-hint {
  margin: 7px 0 12px;
  color: var(--van-text-color-3, var(--van-text-color-2));
  font-size: 12px;
  line-height: 1.5;
}

.shortcut-sheet-list {
  overflow: hidden;
  border: 1px solid var(--van-border-color);
  border-radius: 7px;
}

.shortcut-sheet-row {
  display: flex;
  align-items: center;
  gap: 7px;
  min-height: 44px;
  padding: 5px 8px;
  border-bottom: 1px solid var(--van-border-color);
}

.shortcut-sheet-row:last-child { border-bottom: 0; }

.shortcut-sheet-label {
  min-width: 0;
  flex: 1;
  overflow: hidden;
  color: var(--van-text-color);
  font-size: 13px;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.shortcut-sheet-value {
  min-width: 72px;
  min-height: 30px;
  padding: 4px 7px;
  border: 1px solid var(--van-border-color);
  border-radius: 5px;
  background: var(--van-background);
  color: var(--van-text-color-2);
  font-size: 11px;
  white-space: nowrap;
}

.shortcut-sheet-value.recording {
  border-color: var(--van-primary-color);
  color: var(--van-primary-color);
}

.shortcut-sheet-reset {
  display: inline-flex;
  width: 28px;
  height: 30px;
  align-items: center;
  justify-content: center;
  padding: 0;
  border: 0;
  background: transparent;
  color: var(--van-text-color-2);
}

.shortcut-sheet-error {
  margin: 9px 0 0;
  color: var(--van-danger-color);
  font-size: 12px;
}

.shortcut-reset-all {
  margin-top: 13px;
}
</style>
