<template>
  <Teleport to="body">
    <div v-if="visible" class="settings-overlay" @click.self="$emit('close')" @keydown.esc="$emit('close')">
      <div class="settings-dialog">
        <div class="settings-header">
          <h2>设置</h2>
          <button class="settings-close-btn" @click="$emit('close')">
            <IconClose />
          </button>
        </div>
        <div class="settings-body">
          <div class="settings-section webdav-section">
            <h3>WebDAV 同步</h3>
            <div class="settings-field">
              <label>地址</label>
              <input v-model="webdavForm.url" type="url" class="settings-input" placeholder="https://dav.example.com/dav" autocomplete="off" spellcheck="false" />
            </div>
            <div class="settings-field">
              <label>用户名</label>
              <input v-model="webdavForm.username" type="text" class="settings-input" placeholder="WebDAV 用户名" autocomplete="username" spellcheck="false" />
            </div>
            <div class="settings-field">
              <label>密码</label>
              <input
                v-model="webdavForm.password"
                type="password"
                class="settings-input"
                :placeholder="webdavPasswordHint"
                autocomplete="new-password"
                @input="webdavPasswordTouched = true"
              />
            </div>
            <div class="settings-field">
              <label>同步目录</label>
              <input v-model="webdavForm.folder" type="text" class="settings-input" placeholder="Woo" autocomplete="off" spellcheck="false" />
            </div>
            <label class="webdav-enable">
              <input v-model="webdavForm.enabled" type="checkbox" />
              <span>启用 WebDAV 同步</span>
            </label>
            <div class="lan-actions">
              <button class="settings-btn secondary-btn" :disabled="webdavBusy" @click="testWebdav">
                {{ webdavBusy ? '测试中…' : '测试连接' }}
              </button>
              <button class="settings-btn save-btn" :disabled="webdavBusy" @click="saveWebdav">保存 WebDAV</button>
            </div>
            <p v-if="webdavNotice" class="webdav-notice">{{ webdavNotice }}</p>
            <p v-if="webdavError" class="lan-error">{{ webdavError }}</p>
            <p v-if="webdav?.active" class="settings-help">状态：已启用 · {{ syncStore.lastSyncLabel }}</p>
            <p class="settings-help">与局域网同步二选一；启用 WebDAV 会自动停用局域网。</p>
          </div>
          <div class="settings-section lan-section">
            <div class="settings-section-title-row">
              <h3>局域网同步</h3>
              <button class="settings-btn secondary-btn" :disabled="lanBusy" @click="toggleLan">
                {{ lan?.enabled ? '停用' : '启用' }}
              </button>
            </div>
            <p class="settings-help">手机和电脑连同一个 Wi-Fi，直接同步到这台电脑；不经过互联网，也不需要服务器。</p>
            <div v-if="lan?.enabled" class="lan-box">
              <div class="lan-pair-box">
                <div class="lan-pair-copy">
                  <span>手机配对码</span>
                  <strong>{{ lan.pairCode || '------' }}</strong>
                </div>
                <p class="settings-help">手机打开「我的 → WebDAV 同步」→「电脑配对」，输入这 6 位数字即可自动连接并双向同步。</p>
              </div>
              <div v-if="lan.address" class="lan-code-row"><span>地址</span><code>{{ lan.address }}</code></div>
              <div v-else class="lan-warning">未找到局域网地址，请确认电脑已连接 Wi-Fi。</div>
              <div class="lan-code-row"><span>用户名</span><code>woo</code></div>
              <div class="lan-code-row"><span>局域网密码</span><code>{{ lan.token }}</code></div>
              <div class="lan-code-row"><span>目录</span><code>{{ lan.folder }}</code></div>
              <p class="settings-help">上面是手动连接信息；配对码只是发现码，局域网密码用于 WebDAV 认证。</p>
              <div class="lan-actions">
                <button class="settings-btn secondary-btn" @click="copyLan">{{ lanCopied ? '已复制' : '复制连接信息' }}</button>
                <button class="settings-btn secondary-btn" :disabled="syncStore.isSyncing" @click="syncNow">
                  {{ syncStore.isSyncing ? '同步中…' : '立即同步' }}
                </button>
              </div>
              <p v-if="lan.active" class="settings-help">
                状态：运行中 · {{ syncStore.lastSyncLabel }}<span v-if="syncStore.pendingChanges > 0"> · 待同步 {{ syncStore.pendingChanges }}</span>
              </p>
              <p v-else class="lan-warning">当前账号与这组局域网配置不匹配，请停用后重新启用。</p>
            </div>
            <p v-if="lanError" class="lan-error">{{ lanError }}</p>
          </div>
          <div class="settings-section appearance-section">
            <h3>外观</h3>
            <div class="theme-grid" role="radiogroup" aria-label="选择主题">
              <button
                v-for="option in themeStore.themeOptions"
                :key="option.id"
                type="button"
                class="theme-option"
                :class="{ selected: themeStore.theme === option.id }"
                role="radio"
                :aria-checked="themeStore.theme === option.id"
                @click="themeStore.theme = option.id"
              >
                <span class="theme-swatch" aria-hidden="true">
                  <span v-for="color in option.colors" :key="color" :style="{ backgroundColor: color }"></span>
                </span>
                <span class="theme-option-copy">
                  <strong>{{ option.label }}</strong>
                  <small>{{ option.description }}</small>
                </span>
                <span v-if="themeStore.theme === option.id" class="theme-selected-mark" aria-hidden="true">✓</span>
              </button>
            </div>
          </div>
          <div class="settings-section">
            <h3>编辑字体</h3>
            <div class="font-grid">
              <button
                v-for="option in editorFontOptions"
                :key="option.id"
                type="button"
                class="font-option"
                :class="{ selected: editorFontId === option.id }"
                @click="setEditorFontId(option.id)"
              >
                <span class="font-preview" :style="{ fontFamily: option.stack }">{{ option.preview }}</span>
                <span class="font-copy">
                  <strong>{{ option.label }}</strong>
                  <small>{{ option.hint }}</small>
                </span>
              </button>
            </div>
            <p class="settings-help">只影响正文，不改变应用界面。</p>
          </div>
          <div class="settings-section">
            <h3>静态资源链接</h3>
            <div class="settings-field">
              <label>Provider</label>
              <input type="text" v-model="assetProviderInput" placeholder="如 qiniu / s3 / r2 / aliyun-oss" class="settings-input" />
            </div>
            <div class="settings-field">
              <label>Base URL</label>
              <input type="text" v-model="assetBaseUrlInput" placeholder="https://cdn.example.com" class="settings-input" />
            </div>
            <div class="settings-field">
              <label>Path Prefix (可选)</label>
              <input type="text" v-model="assetPathPrefixInput" placeholder="uploads/images" class="settings-input" />
            </div>
            <p class="settings-help">编辑区插入图片时，若输入相对路径将自动拼接为 `baseUrl/pathPrefix/relativePath`。</p>
          </div>
          <div class="settings-section shortcuts-section">
            <div class="settings-section-title-row">
              <h3>编辑器快捷键</h3>
              <button class="shortcut-reset-all" type="button" @click="resetAllShortcuts">恢复默认</button>
            </div>
            <div class="shortcut-list">
              <div v-for="entry in shortcutEntries" :key="entry.action" class="shortcut-row">
                <span class="shortcut-label">{{ entry.label }}</span>
                <button
                  type="button"
                  class="shortcut-input"
                  :class="{ recording: recordingAction === entry.action }"
                  @click="startRecording(entry.action)"
                  @keydown="captureShortcut($event, entry.action)"
                  @keydown.esc="cancelRecording"
                >
                  {{ recordingAction === entry.action ? '请按下组合键' : shortcutForDisplay(entry.shortcut) }}
                </button>
                <button
                  v-if="entry.shortcut !== entry.defaultShortcut"
                  type="button"
                  class="shortcut-reset"
                  title="恢复此项默认快捷键"
                  @click="resetShortcut(entry.action)"
                >↺</button>
              </div>
            </div>
            <p v-if="shortcutError" class="shortcut-error">{{ shortcutError }}</p>
          </div>
          <div class="settings-section about-section">
            <h3>关于</h3>
            <div class="about-row">
              <div>
                <div class="about-name">Woo 无我笔记</div>
                <div class="about-version">版本 {{ appVersion ? `v${appVersion}` : '读取中...' }}</div>
              </div>
              <button class="settings-btn secondary-btn" @click="emit('check-update')">检查更新</button>
            </div>
          </div>
        </div>
        <div class="settings-footer">
          <button class="settings-btn save-btn" @click="handleSave">保存</button>
        </div>
      </div>
    </div>
  </Teleport>
</template>

<script setup lang="ts">
import { computed, onMounted, reactive, ref, watch } from 'vue'
import IconClose from '../icons/IconClose.vue'
import { getAssetLinkSettings, saveAssetLinkSettings } from '../../services/assetLink'
import { useThemeStore } from '../../stores/theme'
import { editorFontId, editorFontOptions, setEditorFontId } from '../../config/editorFonts'
import { useSyncStore } from '../../stores/sync'
import { invoke } from '../../services/api'
import {
  DEFAULT_EDITOR_SHORTCUTS,
  editorShortcutEntries,
  resetEditorShortcuts,
  setEditorShortcut,
  shortcutConflicts,
  shortcutForDisplay,
  shortcutFromKeyboardEvent,
  type EditorShortcutAction,
} from '../../config/editorShortcuts'

interface Props {
  visible: boolean
  appVersion: string
}

const props = defineProps<Props>()
const emit = defineEmits<{ close: []; 'check-update': [] }>()
const themeStore = useThemeStore()

const shortcutEntries = computed(() => editorShortcutEntries())
const recordingAction = ref<EditorShortcutAction | null>(null)
const shortcutError = ref('')

function startRecording(action: EditorShortcutAction) {
  shortcutError.value = ''
  recordingAction.value = action
}

function cancelRecording() {
  recordingAction.value = null
}

function captureShortcut(event: KeyboardEvent, action: EditorShortcutAction) {
  if (recordingAction.value !== action) return
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
}

/* ========== WebDAV 同步 ========== */
interface WebdavView {
  enabled: boolean
  active: boolean
  url: string
  username: string
  folder: string
  hasPassword: boolean
  configured: boolean
}

const webdav = ref<WebdavView | null>(null)
const webdavForm = reactive({ enabled: false, url: '', username: '', password: '', folder: 'Woo' })
const webdavPasswordTouched = ref(false)
const webdavBusy = ref(false)
const webdavError = ref('')
const webdavNotice = ref('')

const webdavPasswordHint = computed(() =>
  webdav.value?.hasPassword ? '已保存，留空不改' : 'WebDAV 密码 / 应用密码',
)

async function loadWebdav() {
  try {
    const config = await invoke<WebdavView>('webdav:getConfig')
    webdav.value = config
    webdavForm.enabled = config.enabled
    webdavForm.url = config.url
    webdavForm.username = config.username
    webdavForm.folder = config.folder || 'Woo'
    webdavForm.password = ''
    webdavPasswordTouched.value = false
  } catch (error: any) {
    webdavError.value = error?.message || ''
  }
}

function webdavPayload() {
  return {
    url: webdavForm.url.trim(),
    username: webdavForm.username.trim(),
    password: webdavPasswordTouched.value ? webdavForm.password : null,
    folder: webdavForm.folder.trim() || 'Woo',
  }
}

async function testWebdav() {
  if (webdavBusy.value) return
  webdavBusy.value = true
  webdavError.value = ''
  webdavNotice.value = ''
  try {
    webdavNotice.value = await invoke<string>('webdav:test', webdavPayload())
  } catch (error: any) {
    webdavError.value = error?.message || '连接失败'
  } finally {
    webdavBusy.value = false
  }
}

async function saveWebdav() {
  if (webdavBusy.value) return
  if (!webdavForm.url.trim()) {
    webdavError.value = '请填写 WebDAV 地址'
    return
  }
  webdavBusy.value = true
  webdavError.value = ''
  webdavNotice.value = ''
  try {
    // 与局域网同步互斥：启用 WebDAV 时先停掉局域网。
    if (webdavForm.enabled && lan.value?.enabled) {
      await invoke('lanSync:setEnabled', { enabled: false })
      lan.value = await invoke<LanView>('lanSync:getConfig')
    }
    const config = await invoke<WebdavView>('webdav:saveConfig', {
      enabled: webdavForm.enabled,
      ...webdavPayload(),
    })
    webdav.value = config
    webdavForm.enabled = config.enabled
    webdavForm.password = ''
    webdavPasswordTouched.value = false
    await syncStore.refreshStatus()
    if (config.enabled) void syncStore.triggerSync()
    webdavNotice.value = config.enabled ? '已启用 WebDAV 同步' : '已保存'
  } catch (error: any) {
    webdavError.value = error?.message || '保存失败'
  } finally {
    webdavBusy.value = false
  }
}

/* ========== 局域网同步 ========== */
interface LanView {
  enabled: boolean
  active: boolean
  running: boolean
  port: number
  token: string
  pairCode: string
  folder: string
  address: string | null
}

const syncStore = useSyncStore()
const lan = ref<LanView | null>(null)
const lanBusy = ref(false)
const lanCopied = ref(false)
const lanError = ref('')

async function loadLan() {
  try {
    lan.value = await invoke<LanView>('lanSync:getConfig')
  } catch (error: any) {
    lanError.value = error?.message || ''
  }
}

async function toggleLan() {
  if (lanBusy.value) return
  lanBusy.value = true
  lanError.value = ''
  try {
    const nextEnabled = !lan.value?.enabled
    // 与 WebDAV 同步互斥：启用局域网时先停掉云端 WebDAV。
    if (nextEnabled && webdav.value?.enabled) {
      await invoke('webdav:saveConfig', {
        enabled: false,
        url: webdavForm.url.trim(),
        username: webdavForm.username.trim(),
        password: null,
        folder: webdavForm.folder.trim() || 'Woo',
      })
      webdav.value = await invoke<WebdavView>('webdav:getConfig')
      webdavForm.enabled = false
    }
    lan.value = await invoke<LanView>('lanSync:setEnabled', { enabled: nextEnabled })
    await syncStore.refreshStatus()
    if (lan.value?.enabled) {
      // 服务刚起来，桌面库先同步一次；手机随后拉取。
      void syncStore.triggerSync()
    }
  } catch (error: any) {
    lanError.value = error?.message || '局域网同步操作失败'
  } finally {
    lanBusy.value = false
  }
}

async function copyLan() {
  if (!lan.value) return
  const text = `配对码：${lan.value.pairCode || ''}\n地址：${lan.value.address || ''}\n用户名：woo\n局域网密码：${lan.value.token}\n目录：${lan.value.folder}`
  try {
    await navigator.clipboard.writeText(text)
    lanCopied.value = true
    setTimeout(() => { lanCopied.value = false }, 1600)
  } catch {
    lanError.value = '复制失败，请手动记录上面的信息'
  }
}

async function syncNow() {
  const ok = await syncStore.triggerSync()
  if (!ok) lanError.value = syncStore.errorMsg || '同步失败'
}

watch(() => props.visible, (visible) => {
  if (visible) {
    void loadWebdav()
    void loadLan()
  }
})
onMounted(() => {
  void loadWebdav()
  void loadLan()
})

/* ========== 资产链接配置状态 ========== */
const assetProviderInput = ref('custom')
const assetBaseUrlInput = ref('')
const assetPathPrefixInput = ref('')
function loadSettings() {
  const asset = getAssetLinkSettings()
  assetProviderInput.value = asset.provider
  assetBaseUrlInput.value = asset.baseUrl
  assetPathPrefixInput.value = asset.pathPrefix
}

loadSettings()

function handleSave() {
  saveAssetLinkSettings({
    provider: assetProviderInput.value.trim() || 'custom',
    baseUrl: assetBaseUrlInput.value.trim(),
    pathPrefix: assetPathPrefixInput.value.trim()
  })
  emit('close')
}
</script>

<style scoped>
.settings-overlay { position: fixed; top: 0; left: 0; right: 0; bottom: 0; background-color: rgba(0, 0, 0, 0.4); z-index: 9999; display: flex; align-items: center; justify-content: center; }
.settings-dialog { display: flex; flex-direction: column; background-color: var(--bg-surface); border: 1px solid var(--border-primary); border-radius: 8px; width: 520px; max-height: 80vh; overflow: hidden; box-shadow: 0 8px 32px rgba(0, 0, 0, 0.2); }
.settings-header { display: flex; flex: 0 0 auto; align-items: center; justify-content: space-between; padding: 16px 20px; border-bottom: 1px solid var(--border-secondary); background: var(--bg-surface); }
.settings-header h2 { font-size: 16px; font-weight: 600; color: var(--text-primary); margin: 0; }
.settings-close-btn { background: transparent; border: none; cursor: pointer; padding: 4px; border-radius: 4px; display: flex; align-items: center; color: var(--text-secondary); transition: all 0.2s; }
.settings-close-btn:hover { background-color: var(--bg-hover); color: var(--text-primary); }
.settings-body { flex: 1 1 auto; min-height: 0; overflow-y: auto; padding: 20px; }
.settings-section { margin-bottom: 18px; }
.settings-section h3 { font-size: 14px; font-weight: 600; color: var(--text-primary); margin: 0 0 12px 0; }
.theme-grid { display: grid; grid-template-columns: repeat(2, minmax(0, 1fr)); gap: 8px; }
.theme-option { position: relative; display: flex; align-items: center; gap: 10px; min-width: 0; padding: 10px; border: 1px solid var(--border-primary); border-radius: 6px; background: var(--bg-elevated); color: var(--text-primary); text-align: left; cursor: pointer; transition: border-color 0.2s, background-color 0.2s, box-shadow 0.2s; }
.theme-option:hover { background: var(--bg-hover); }
.theme-option.selected { border-color: var(--accent); box-shadow: 0 0 0 2px var(--accent-light); }
.theme-swatch { display: flex; width: 34px; height: 34px; flex-shrink: 0; overflow: hidden; border: 1px solid var(--border-primary); border-radius: 5px; }
.theme-swatch span { flex: 1; }
.theme-option-copy { display: flex; min-width: 0; flex-direction: column; gap: 2px; }
.theme-option-copy strong { font-size: 13px; font-weight: 600; }
.theme-option-copy small { overflow: hidden; color: var(--text-muted); font-size: 11px; text-overflow: ellipsis; white-space: nowrap; }
.theme-selected-mark { position: absolute; top: 6px; right: 8px; color: var(--accent); font-size: 14px; font-weight: 700; }
.settings-field { display: flex; flex-direction: column; gap: 8px; margin-bottom: 12px; }
.settings-field label { font-size: 13px; color: var(--text-secondary); }
.settings-input { flex: 1; padding: 8px 12px; border: 1px solid var(--border-primary); border-radius: 4px; font-size: 13px; background-color: var(--bg-elevated); color: var(--text-primary); outline: none; transition: border-color 0.2s; }
.settings-input:focus { border-color: var(--accent); }
.settings-help { font-size: 12px; color: var(--text-muted); margin: 0; }
.settings-help a { color: var(--accent); text-decoration: none; }
.settings-help a:hover { text-decoration: underline; }
.settings-section-title-row { display: flex; align-items: center; justify-content: space-between; gap: 12px; }
.shortcut-reset-all { border: 1px solid var(--border-primary); border-radius: 4px; background: transparent; color: var(--text-secondary); padding: 4px 8px; cursor: pointer; font-size: 12px; }
.shortcut-reset-all:hover { background: var(--bg-hover); color: var(--text-primary); }
.shortcut-list { border: 1px solid var(--border-primary); border-radius: 5px; overflow: hidden; }
.shortcut-row { display: grid; grid-template-columns: minmax(0, 1fr) auto 28px; align-items: center; gap: 8px; min-height: 38px; padding: 5px 8px 5px 12px; border-bottom: 1px solid var(--border-secondary); }
.shortcut-row:last-child { border-bottom: none; }
.shortcut-label { min-width: 0; color: var(--text-secondary); font-size: 13px; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
.shortcut-input { min-width: 94px; border: 1px solid var(--border-primary); border-radius: 4px; background: var(--bg-elevated); color: var(--text-primary); padding: 5px 8px; text-align: center; font: inherit; font-size: 12px; cursor: pointer; }
.shortcut-input:hover, .shortcut-input.recording { border-color: var(--accent); color: var(--accent); }
.shortcut-reset { width: 24px; height: 24px; padding: 0; border: 0; border-radius: 4px; background: transparent; color: var(--text-muted); cursor: pointer; font-size: 16px; }
.shortcut-reset:hover { background: var(--bg-hover); color: var(--text-primary); }
.shortcut-error { margin: 8px 0 0; color: var(--danger, #d04444); font-size: 12px; }
.about-section { padding-top: 18px; border-top: 1px solid var(--border-secondary); }
.font-grid { display: grid; grid-template-columns: repeat(2, minmax(0, 1fr)); gap: 8px; }
.font-option { display: flex; align-items: center; gap: 10px; min-height: 54px; padding: 8px 12px; border: 1px solid var(--border-secondary); border-radius: 7px; background: var(--bg-primary); color: var(--text-primary); text-align: left; cursor: pointer; transition: border-color 0.15s ease, background 0.15s ease; }
.font-option:hover { background: var(--bg-hover); }
.font-option.selected { border-color: var(--accent); background: var(--accent-light); box-shadow: 0 0 0 1px color-mix(in srgb, var(--accent) 14%, transparent); }
.font-preview { flex: 0 0 auto; font-size: 17px; line-height: 1.3; white-space: nowrap; }
.font-copy { display: flex; min-width: 0; flex-direction: column; gap: 2px; font-size: 12px; }
.font-copy strong { font-weight: 600; }
.font-copy small { color: var(--text-muted); font-size: 10px; }
.lan-box { margin-top: 10px; padding: 12px; border: 1px solid var(--border-secondary); border-radius: 6px; background: var(--bg-primary); }
.lan-code-row { display: flex; align-items: baseline; gap: 12px; padding: 4px 0; font-size: 12px; color: var(--text-secondary); }
.lan-code-row span { width: 52px; flex: 0 0 52px; }
.lan-code-row code { user-select: all; font-family: var(--font-mono, ui-monospace, SFMono-Regular, Menlo, monospace); color: var(--text-primary); font-size: 12px; word-break: break-all; }
.lan-actions { display: flex; gap: 8px; margin-top: 10px; }
.lan-warning { margin: 8px 0; color: var(--warning-color, #b7791f); font-size: 12px; line-height: 1.6; }
.lan-error { margin: 8px 0 0; color: var(--danger-color, #c53030); font-size: 12px; line-height: 1.6; }
.lan-section .settings-help { margin: 6px 0 0; line-height: 1.6; }
.lan-pair-box { padding: 12px 14px; margin: 10px 0 8px; border-radius: 10px; background: var(--bg-tertiary); border: 1px solid var(--border-secondary); }
.lan-pair-copy { display: flex; align-items: baseline; justify-content: space-between; gap: 12px; }
.lan-pair-copy span { color: var(--text-secondary); font-size: 12px; }
.lan-pair-copy strong { color: var(--accent); font-family: "SFMono-Regular", "Roboto Mono", monospace; font-size: 24px; letter-spacing: 0.22em; line-height: 1.1; }
.lan-pair-box .settings-help { margin: 6px 0 0; line-height: 1.55; }
.webdav-enable { display: flex; align-items: center; gap: 8px; margin: 10px 0 2px; color: var(--text-primary); font-size: 12.5px; cursor: pointer; }
.webdav-notice { margin: 8px 0 0; color: var(--accent); font-size: 12px; line-height: 1.6; }
.webdav-section .settings-help { margin: 6px 0 0; line-height: 1.6; }
.about-row { display: flex; align-items: center; justify-content: space-between; gap: 16px; flex-wrap: wrap; }
.about-name { color: var(--text-primary); font-size: 13px; font-weight: 600; }
.about-version { margin-top: 4px; color: var(--text-muted); font-size: 12px; }
.settings-footer { display: flex; flex: 0 0 auto; justify-content: flex-end; padding: 12px 20px; border-top: 1px solid var(--border-secondary); background: var(--bg-surface); }
.settings-btn { padding: 6px 16px; border: none; border-radius: 4px; cursor: pointer; font-size: 13px; transition: all 0.2s; }
.save-btn { background-color: var(--accent); color: #ffffff; }
.save-btn:hover:not(:disabled) { background-color: var(--accent-hover); }
.secondary-btn { border: 1px solid var(--border-primary); background: var(--bg-elevated); color: var(--text-primary); }
.secondary-btn:hover { background: var(--bg-hover); }

@media (max-width: 640px) {
  .settings-overlay {
    align-items: flex-end;
    padding: 0 0 env(safe-area-inset-bottom) 0;
  }
  .settings-dialog {
    width: 100%;
    max-width: 100%;
    max-height: 90vh;
    border-radius: 12px 12px 0 0;
    border-left: none;
    border-right: none;
    border-bottom: none;
  }
  .theme-grid { grid-template-columns: 1fr; }
  .shortcut-row { grid-template-columns: minmax(0, 1fr) auto 28px; }
}
</style>
