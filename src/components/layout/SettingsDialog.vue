<template>
  <Teleport to="body">
    <div v-if="visible" class="settings-overlay" @click.self="$emit('close')" @keydown.esc="$emit('close')">
      <div class="settings-dialog">
        <div class="settings-header">
          <h2>{{ dialogTitle }}</h2>
          <button class="settings-close-btn" @click="$emit('close')">
            <IconClose />
          </button>
        </div>
        <div class="settings-body">
          <SyncSpaceSettings v-if="section === 'all' || section === 'sync'" />
          <MarkdownVaultSettings v-if="section === 'all' || section === 'sync'" />
          <div v-if="section === 'all' || section === 'mcp'" class="settings-section mcp-section">
            <div class="settings-section-title-row">
              <h3>MCP 服务</h3>
              <button class="settings-btn secondary-btn" :disabled="mcpBusy" @click="toggleMcp">
                {{ mcp?.enabled ? '停用' : '启用' }}
              </button>
            </div>
            <p class="settings-help">让 Claude Code、Codex、Hermes、Pi 等本机 AI 工具读写笔记；仅监听 127.0.0.1，默认只读。</p>
            <div v-if="mcp?.enabled" class="lan-box">
              <p class="settings-help">
                状态：{{ mcp.running ? '运行中' : '未运行' }} · 仅监听 127.0.0.1
              </p>
              <div class="lan-code-row">
                <span>地址</span>
                <code>{{ mcpUrl }}</code>
              </div>
              <div class="lan-code-row">
                <span>令牌</span>
                <code>{{ mcp.token ? (mcpTokenVisible ? mcp.token : '••••••••••••••••') : '未生成' }}</code>
              </div>
              <div class="lan-actions">
                <button class="settings-btn secondary-btn" @click="mcpTokenVisible = !mcpTokenVisible">
                  {{ mcpTokenVisible ? '隐藏令牌' : '显示令牌' }}
                </button>
                <button class="settings-btn secondary-btn" :disabled="!mcp.token" @click="copyMcpValue(mcp.token, 'token')">
                  {{ mcpCopied === 'token' ? '已复制' : '复制令牌' }}
                </button>
                <button class="settings-btn secondary-btn" :disabled="mcpBusy" @click="rotateMcpToken">
                  {{ mcpRotateArmed ? '确认重新生成' : '重新生成' }}
                </button>
              </div>
              <label class="webdav-enable">
                <input
                  type="checkbox"
                  :checked="mcp.allowWrites"
                  :disabled="mcpBusy"
                  @change="toggleMcpWrites"
                />
                <span>允许 AI 修改笔记</span>
              </label>
              <p v-if="mcp.allowWrites" class="lan-warning">
                开启后，任何持有令牌的本机程序都可以新建、修改、移动、软删除笔记。删除会进入回收站，可恢复。
              </p>
              <div class="settings-field mcp-port-field">
                <label>端口</label>
                <input
                  v-model.number="mcpPortInput"
                  type="number"
                  min="1024"
                  max="65535"
                  class="settings-input"
                />
                <button class="settings-btn secondary-btn" :disabled="mcpBusy" @click="saveMcpPort">应用</button>
              </div>
              <div class="settings-field">
                <label>客户端配置</label>
                <select v-model="mcpClientId" class="settings-input">
                  <option v-for="option in mcpClientOptions" :key="option.id" :value="option.id">
                    {{ option.label }}
                  </option>
                </select>
              </div>
              <p class="settings-help">{{ mcpClientHint }}</p>
              <pre class="mcp-snippet">{{ mcpSnippet }}</pre>
              <div class="lan-actions">
                <button class="settings-btn secondary-btn" @click="copyMcpValue(mcpSnippet, 'snippet')">
                  {{ mcpCopied === 'snippet' ? '已复制' : '复制配置' }}
                </button>
              </div>
              <p class="settings-help">Woo 需要保持运行；令牌等同笔记权限，请勿分享。MCP 只能访问正式文稿，不能访问草稿。</p>
              <p v-if="mcp.lastError" class="lan-error">{{ mcp.lastError }}</p>
            </div>
            <p v-else-if="mcp?.lastError" class="lan-warning">{{ mcp.lastError }}</p>
            <p v-if="mcpNotice" class="webdav-notice">{{ mcpNotice }}</p>
            <p v-if="mcpError" class="lan-error">{{ mcpError }}</p>
          </div>
          <div v-if="section === 'all' || section === 'view'" class="settings-section appearance-section">
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
          <div v-if="section === 'all' || section === 'view'" class="settings-section">
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
          <div v-if="section === 'all' || section === 'editor'" class="settings-section">
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
          <div v-if="section === 'all' || section === 'editor'" class="settings-section shortcuts-section">
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
          <div v-if="section === 'all' || section === 'about'" class="settings-section about-section">
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
        <div v-if="section === 'all' || section === 'editor'" class="settings-footer">
          <button class="settings-btn save-btn" @click="handleSave">保存</button>
        </div>
      </div>
    </div>
  </Teleport>
</template>

<script setup lang="ts">
import { computed, onMounted, ref, watch } from 'vue'
import type { SettingsSection } from '../../config/menus'
import IconClose from '../icons/IconClose.vue'
import SyncSpaceSettings from '../settings/SyncSpaceSettings.vue'
import MarkdownVaultSettings from '../settings/MarkdownVaultSettings.vue'
import { getAssetLinkSettings, saveAssetLinkSettings } from '../../services/assetLink'
import { useThemeStore } from '../../stores/theme'
import { editorFontId, editorFontOptions, setEditorFontId } from '../../config/editorFonts'
import {
  getMcpConfig,
  regenerateMcpToken,
  setMcpAllowWrites,
  setMcpEnabled,
  setMcpPort,
  type McpView,
} from '../../services/mcpApi'
import {
  buildMcpSnippet,
  mcpClientLocation,
  mcpClientOptions,
  type McpClientId,
} from '../../config/mcpClients'
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
  section?: SettingsSection
}

const props = withDefaults(defineProps<Props>(), { section: 'all' })
const emit = defineEmits<{ close: []; 'check-update': [] }>()
const themeStore = useThemeStore()

const dialogTitle = computed(() => {
  switch (props.section) {
    case 'sync': return '跨设备'
    case 'editor': return '编辑设置'
    case 'view': return '外观与字体'
    case 'mcp': return 'MCP 服务'
    case 'about': return '关于 Woo'
    default: return '设置'
  }
})

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

/* ========== MCP 服务 ========== */
const mcp = ref<McpView | null>(null)
const mcpBusy = ref(false)
const mcpError = ref('')
const mcpNotice = ref('')
const mcpTokenVisible = ref(false)
const mcpPortInput = ref(47823)
const mcpClientId = ref<McpClientId>('claude-code')
const mcpCopied = ref('')
const mcpRotateArmed = ref(false)

const mcpUrl = computed(() =>
  mcp.value?.url ?? (mcp.value ? `http://127.0.0.1:${mcp.value.port}/mcp` : ''),
)
const mcpSnippet = computed(() =>
  mcp.value ? buildMcpSnippet(mcpClientId.value, mcpUrl.value, mcp.value.token) : '',
)
const mcpClientHint = computed(() => mcpClientLocation(mcpClientId.value))

async function loadMcp() {
  try {
    mcp.value = await getMcpConfig()
    mcpPortInput.value = mcp.value.port
    mcpError.value = ''
    mcpNotice.value = ''
    mcpRotateArmed.value = false
  } catch (error: any) {
    mcpError.value = error?.message || '读取 MCP 配置失败'
  }
}

async function toggleMcp() {
  if (mcpBusy.value) return
  mcpBusy.value = true
  mcpError.value = ''
  mcpNotice.value = ''
  try {
    const nextEnabled = !mcp.value?.enabled
    mcp.value = await setMcpEnabled(nextEnabled)
    mcpPortInput.value = mcp.value.port
    mcpNotice.value = nextEnabled ? 'MCP 服务已启用' : 'MCP 服务已停用'
  } catch (error: any) {
    const message = error?.message || 'MCP 服务操作失败'
    await loadMcp()
    mcpError.value = message
  } finally {
    mcpBusy.value = false
  }
}

async function toggleMcpWrites() {
  if (mcpBusy.value || !mcp.value) return
  mcpBusy.value = true
  mcpError.value = ''
  mcpNotice.value = ''
  try {
    mcp.value = await setMcpAllowWrites(!mcp.value.allowWrites)
    mcpNotice.value = mcp.value.allowWrites ? '已允许 AI 修改笔记' : '已切换为只读'
  } catch (error: any) {
    const message = error?.message || '更新写入权限失败'
    await loadMcp()
    mcpError.value = message
  } finally {
    mcpBusy.value = false
  }
}

async function saveMcpPort() {
  if (mcpBusy.value) return
  const port = Number(mcpPortInput.value)
  if (!Number.isInteger(port) || port < 1024 || port > 65535) {
    mcpError.value = '端口必须在 1024-65535 之间'
    return
  }
  mcpBusy.value = true
  mcpError.value = ''
  mcpNotice.value = ''
  try {
    mcp.value = await setMcpPort(port)
    mcpPortInput.value = mcp.value.port
    mcpNotice.value = `端口已设置为 ${mcp.value.port}，请更新客户端配置`
  } catch (error: any) {
    const message = error?.message || '设置端口失败'
    await loadMcp()
    mcpError.value = message
  } finally {
    mcpBusy.value = false
  }
}

async function rotateMcpToken() {
  if (mcpBusy.value || !mcp.value) return
  if (!mcpRotateArmed.value) {
    mcpRotateArmed.value = true
    mcpNotice.value = '再次点击「确认重新生成」将轮换令牌，所有客户端都需要更新配置'
    return
  }
  mcpRotateArmed.value = false
  mcpBusy.value = true
  mcpError.value = ''
  mcpNotice.value = ''
  try {
    mcp.value = await regenerateMcpToken()
    mcpTokenVisible.value = true
    mcpNotice.value = '令牌已重新生成，请更新所有客户端配置'
  } catch (error: any) {
    mcpError.value = error?.message || '生成令牌失败'
  } finally {
    mcpBusy.value = false
  }
}

async function copyMcpValue(value: string, key: string) {
  if (!value) return
  try {
    await navigator.clipboard.writeText(value)
    mcpCopied.value = key
    setTimeout(() => {
      if (mcpCopied.value === key) mcpCopied.value = ''
    }, 1600)
  } catch {
    mcpError.value = '复制失败，请手动选择文本'
  }
}

watch(() => props.visible, (visible) => {
  if (visible) {
    void loadMcp()
  }
})
onMounted(() => {
  void loadMcp()
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
.mcp-section .settings-help { margin: 6px 0 0; line-height: 1.6; }
.mcp-port-field { flex-direction: row; align-items: center; gap: 8px; }
.mcp-port-field label { flex: 0 0 52px; }
.mcp-port-field .settings-input { max-width: 120px; }
.mcp-snippet { margin: 8px 0 0; padding: 10px; border: 1px solid var(--border-secondary); border-radius: 6px; background: var(--bg-primary); color: var(--text-primary); font: 11px/1.6 var(--font-mono, ui-monospace, SFMono-Regular, Menlo, monospace); white-space: pre-wrap; word-break: break-all; user-select: all; max-height: 220px; overflow: auto; }
.webdav-enable { display: flex; align-items: center; gap: 8px; margin: 10px 0 2px; color: var(--text-primary); font-size: 12.5px; cursor: pointer; }
.webdav-notice { margin: 8px 0 0; color: var(--accent); font-size: 12px; line-height: 1.6; }
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
