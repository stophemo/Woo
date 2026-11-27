<template>
  <div class="sync-space-section">
    <h3>跨设备</h3>
    <p class="sync-help">
      每台设备填写同一个 WebDAV 账号即可同步：云端没有空间会自动建立，已有空间会自动加入。
      不需要配对码，也不用记空间 ID。WebDAV 没有服务端推送，同步不是实时的。
    </p>

    <div class="sync-box">
      <h4>WebDAV 账号</h4>
      <label class="sync-field">
        <span>服务商</span>
        <select v-model="providerId" @change="applyProvider">
          <option v-for="provider in WEBDAV_PROVIDERS" :key="provider.id" :value="provider.id">
            {{ provider.label }}
          </option>
        </select>
      </label>
      <label class="sync-field">
        <span>服务器地址</span>
        <input
          v-model="endpoint"
          type="url"
          :placeholder="currentProvider.endpointPlaceholder"
          autocomplete="off"
          spellcheck="false"
        />
      </label>
      <label class="sync-field">
        <span>账户</span>
        <input v-model="username" type="text" autocomplete="username" spellcheck="false" />
      </label>
      <label class="sync-field">
        <span>应用密码</span>
        <input
          v-model="password"
          type="password"
          :placeholder="config?.account.hasPassword ? '已保存，留空不改' : 'WebDAV 应用密码'"
          autocomplete="new-password"
        />
      </label>
      <p class="sync-help">{{ currentProvider.passwordHint }}</p>
      <div class="sync-actions">
        <button class="sync-btn primary" :disabled="busy" @click="saveAccount">
          {{ busy ? '正在连接…' : '保存并同步' }}
        </button>
        <button class="sync-btn" :disabled="busy" @click="showImport = !showImport">
          从另一台设备导入
        </button>
      </div>
      <p v-if="config?.account.configured" class="sync-help">
        已保存：{{ config.account.username }} @ {{ config.account.endpoint }}
      </p>

      <div v-if="showImport" class="share-box">
        <p class="sync-help">在另一台设备点「添加设备」复制配置串，粘贴到这里。</p>
        <textarea
          v-model="importPayload"
          class="share-input"
          rows="3"
          placeholder="WOO-SYNC:1:..."
          spellcheck="false"
        ></textarea>
        <div class="sync-actions">
          <button class="sync-btn primary" :disabled="busy || !importPayload.trim()" @click="importConfig">
            导入并同步
          </button>
          <button class="sync-btn" :disabled="busy" @click="showImport = false">取消</button>
        </div>
      </div>
    </div>

    <p v-if="config?.legacyNotice" class="sync-notice">{{ config.legacyNotice }}</p>
    <p v-if="config?.notice" class="sync-notice">{{ config.notice }}</p>
    <p v-if="notice" class="sync-notice">{{ notice }}</p>
    <p v-if="error" class="sync-error">{{ error }}</p>

    <template v-if="config?.active">
      <div class="space-hero">
        <div class="space-hero-top">
          <div class="space-hero-copy">
            <span class="space-kicker">WebDAV 同步空间</span>
            <strong class="space-endpoint">{{ config.account.endpoint }}</strong>
            <span class="space-account">{{ config.account.username }}</span>
          </div>
          <span class="space-status" :class="{ syncing: syncStore.isSyncing }">
            <span class="status-dot"></span>
            {{ syncStore.isSyncing ? '同步中' : syncStore.lastSyncLabel }}
          </span>
        </div>
        <div class="space-meta-grid">
          <div><span>本机</span><code>{{ config.identity.deviceName }} · {{ platformLabel(config.identity.platform) }}</code></div>
          <div><span>待同步</span><code>{{ syncStore.pendingChanges || 0 }} 项</code></div>
        </div>
        <div class="space-actions">
          <button class="sync-btn primary" :disabled="syncStore.isSyncing" @click="syncNow">
            {{ syncStore.isSyncing ? '同步中…' : '立即同步' }}
          </button>
          <button class="sync-btn" :disabled="busy" @click="toggleShare">添加设备</button>
          <button class="sync-btn danger" :disabled="busy" @click="leave">退出空间</button>
        </div>
        <p class="space-note">设备列表来自加密心跳，只读；要阻止旧设备继续同步，请修改 WebDAV 应用密码。</p>
      </div>

      <div v-if="showShare" class="sync-box">
        <h4>添加设备</h4>
        <p class="sync-help">
          在另一台设备打开「跨设备 → 从另一台设备导入」，粘贴下面这串配置。
          默认不包含应用密码，另一台设备只需补一次密码。
        </p>
        <textarea
          v-if="sharePayload"
          :value="sharePayload"
          class="share-input"
          rows="4"
          readonly
          spellcheck="false"
          @focus="selectShareText"
        ></textarea>
        <label class="share-option">
          <input v-model="shareWithPassword" type="checkbox" @change="generateShare" />
          包含应用密码（另一台设备零输入，但配置串等于明文密码）
        </label>
        <div class="sync-actions">
          <button class="sync-btn primary" :disabled="!sharePayload" @click="copyShare">复制配置串</button>
          <button class="sync-btn" :disabled="busy" @click="generateShare">重新生成</button>
          <button class="sync-btn" :disabled="busy" @click="showShare = false">关闭</button>
        </div>
      </div>

      <div class="sync-box">
        <div class="sync-section-head">
          <h4>已加入设备</h4>
          <button class="sync-btn small" :disabled="busy" @click="loadDevices">刷新</button>
        </div>
        <div v-for="device in devices" :key="device.deviceId" class="device-row">
          <div>
            <strong>{{ device.name }}</strong>
            <span v-if="device.isCurrent" class="device-current">当前设备</span>
            <span class="device-meta">
              {{ platformLabel(device.platform) }} · {{ formatLastSeen(device.lastSeenAt) }} · v{{ device.appVersion }}
              <template v-if="isStale(device.lastSeenAt)"> · 很久未同步</template>
            </span>
          </div>
        </div>
        <div v-if="config?.active && !devices.some(device => device.isCurrent)" class="device-row">
          <div>
            <strong>{{ config.identity.deviceName }}</strong>
            <span class="device-current">当前设备</span>
            <span class="device-meta">{{ platformLabel(config.identity.platform) }} · 尚未同步</span>
          </div>
        </div>
        <p v-if="config?.active && devices.filter(device => !device.isCurrent).length === 0" class="sync-help">
          只有当前设备。
        </p>
        <p v-if="deviceError" class="sync-error">{{ deviceError }}</p>
      </div>
    </template>
  </div>
</template>

<script setup lang="ts">
import { computed, onMounted, ref, watch } from 'vue'
import { useSyncStore } from '../../stores/sync'
import webdavSpaceApi, {
  type WebDavDeviceView,
  type WebdavSpaceConfigView,
} from '../../services/webdavSpaceApi'
import {
  WEBDAV_PROVIDERS,
  findProvider,
  matchProvider,
} from '../../config/webdavProviders'

const syncStore = useSyncStore()
const config = ref<WebdavSpaceConfigView | null>(null)
const providerId = ref('jianguoyun')
const currentProvider = computed(() => findProvider(providerId.value))
const endpoint = ref('')
const username = ref('')
const password = ref('')
const busy = ref(false)
const notice = ref('')
const error = ref('')
const devices = ref<WebDavDeviceView[]>([])
const deviceError = ref('')
const showImport = ref(false)
const importPayload = ref('')
const showShare = ref(false)
const sharePayload = ref('')
const shareWithPassword = ref(false)

function platformLabel(platform: string) {
  return ({ macos: 'macOS', windows: 'Windows', linux: 'Linux', android: 'Android', ios: 'iOS' } as Record<string, string>)[platform] || platform
}

function formatLastSeen(ms: number) {
  try {
    return new Date(ms).toLocaleString('zh-CN', {
      month: '2-digit',
      day: '2-digit',
      hour: '2-digit',
      minute: '2-digit',
    })
  } catch {
    return '未知时间'
  }
}

function isStale(lastSeenAt: number) {
  return Date.now() - lastSeenAt > 30 * 24 * 60 * 60 * 1000
}

async function loadConfig() {
  try {
    config.value = await webdavSpaceApi.getConfig()
    endpoint.value = config.value.account.endpoint || endpoint.value
    username.value = config.value.account.username || username.value
    providerId.value = matchProvider(endpoint.value).id
    if (!endpoint.value) endpoint.value = currentProvider.value.endpoint
    if (config.value.notice) notice.value = config.value.notice
    if (config.value.active) await loadDevices()
    else devices.value = []
  } catch (err: any) {
    error.value = err?.message || '读取跨设备配置失败'
  }
}

async function loadDevices() {
  if (!config.value?.active) {
    devices.value = []
    return
  }
  deviceError.value = ''
  try {
    devices.value = await webdavSpaceApi.listDevices()
  } catch (err: any) {
    deviceError.value = err?.message || '读取设备列表失败'
  }
}

/** 切换服务商时填入对应预设；没有预设的服务商清空，避免沿用上一个供应商的地址。 */
function applyProvider() {
  endpoint.value = currentProvider.value.endpoint
  error.value = ''
}

async function saveAccount() {
  if (busy.value) return
  busy.value = true
  error.value = ''
  notice.value = ''
  try {
    await webdavSpaceApi.connectAccount(endpoint.value, username.value, password.value || undefined)
    password.value = ''
    await loadConfig()
    notice.value = config.value?.notice || (config.value?.active ? '已连接，正在开始同步…' : '账号已保存')
    if (config.value?.active) {
      await syncStore.refreshStatus()
      void syncStore.triggerSync()
    }
  } catch (err: any) {
    error.value = err?.message || 'WebDAV 连接失败'
  } finally {
    busy.value = false
  }
}

async function importConfig() {
  if (busy.value) return
  const payload = importPayload.value.trim()
  if (!payload) return
  busy.value = true
  error.value = ''
  notice.value = ''
  try {
    const parsed = await webdavSpaceApi.parseConfig(payload)
    endpoint.value = parsed.endpoint
    username.value = parsed.username
    providerId.value = matchProvider(parsed.endpoint).id
    if (parsed.password) {
      password.value = parsed.password
      showImport.value = false
      busy.value = false
      await saveAccount()
      if (config.value?.active) importPayload.value = ''
    } else {
      showImport.value = false
      notice.value = '已填入地址和账户，请补充应用密码后点「保存并同步」。'
    }
  } catch (err: any) {
    error.value = err?.message || '配置串解析失败'
  } finally {
    busy.value = false
  }
}

async function toggleShare() {
  showShare.value = !showShare.value
  if (showShare.value) await generateShare()
}

async function generateShare() {
  try {
    sharePayload.value = await webdavSpaceApi.exportConfig(shareWithPassword.value)
  } catch (err: any) {
    sharePayload.value = ''
    error.value = err?.message || '生成配置串失败'
  }
}

function selectShareText(event: Event) {
  const target = event.target as HTMLTextAreaElement
  target.select()
}

async function copyShare() {
  if (!sharePayload.value) return
  try {
    await navigator.clipboard.writeText(sharePayload.value)
    notice.value = '配置串已复制，请在另一台设备粘贴。'
  } catch {
    error.value = '复制失败，请手动选择配置串。'
  }
}

async function syncNow() {
  error.value = ''
  const ok = await syncStore.triggerSync()
  if (!ok) error.value = syncStore.errorMsg || '同步失败'
  else notice.value = '同步完成。'
  await loadDevices()
}

async function leave() {
  if (busy.value) return
  if (!window.confirm('退出空间只清除本机身份，本机笔记仍保留。要阻止旧设备继续同步，请修改 WebDAV 应用密码。确定退出吗？')) return
  busy.value = true
  error.value = ''
  try {
    await webdavSpaceApi.leaveSpace()
    devices.value = []
    await loadConfig()
    await syncStore.refreshStatus()
    notice.value = '已退出同步空间，本机笔记未删除。'
  } catch (err: any) {
    error.value = err?.message || '退出同步空间失败'
  } finally {
    busy.value = false
  }
}

onMounted(async () => {
  await loadConfig()
  await syncStore.refreshStatus()
})

watch(() => syncStore.lastSyncTime, () => {
  if (config.value?.active) void loadDevices()
})
</script>

<style scoped>
.sync-space-section { display: flex; flex-direction: column; gap: 12px; }
.sync-space-section h3 { margin: 0; font-size: 15px; }
.sync-space-section h4 { margin: 0 0 8px; font-size: 13px; }
.sync-help { margin: 0; color: var(--text-secondary, #8a8a8a); font-size: 12px; line-height: 1.6; }
.sync-box { border: 1px solid var(--border-color, #e5e5e5); border-radius: 12px; padding: 14px; display: flex; flex-direction: column; gap: 10px; background: color-mix(in srgb, var(--bg-secondary, #fff) 72%, transparent); }
.space-hero { display: flex; flex-direction: column; gap: 14px; padding: 16px; border: 1px solid var(--border-color, #e5e5e5); border-radius: 14px; background: linear-gradient(145deg, color-mix(in srgb, var(--accent-color, #2f6fed) 7%, var(--bg-elevated, #fff)), var(--bg-elevated, #fff) 58%); box-shadow: 0 8px 24px color-mix(in srgb, #000 6%, transparent); }
.space-hero-top { display: flex; align-items: flex-start; justify-content: space-between; gap: 14px; }
.space-hero-copy { display: flex; min-width: 0; flex-direction: column; gap: 4px; }
.space-kicker { color: var(--text-secondary, #8a8a8a); font-size: 11px; font-weight: 600; letter-spacing: .08em; text-transform: uppercase; }
.space-endpoint { color: var(--text-primary); font-family: ui-monospace, SFMono-Regular, Menlo, monospace; font-size: 13px; line-height: 1.4; overflow-wrap: anywhere; }
.space-account { color: var(--text-secondary, #8a8a8a); font-size: 11px; }
.space-status { display: inline-flex; flex: 0 0 auto; align-items: center; gap: 6px; padding: 5px 9px; border: 1px solid color-mix(in srgb, var(--accent-color, #2f6fed) 22%, transparent); border-radius: 999px; background: color-mix(in srgb, var(--accent-color, #2f6fed) 8%, transparent); color: var(--text-secondary, #8a8a8a); font-size: 11px; }
.space-status .status-dot { width: 6px; height: 6px; border-radius: 50%; background: var(--accent-color, #2f6fed); }
.space-status.syncing .status-dot { animation: pulse 1.2s ease-in-out infinite; }
.space-meta-grid { display: grid; grid-template-columns: repeat(2, minmax(0, 1fr)); gap: 8px 16px; }
.space-meta-grid > div { display: flex; min-width: 0; flex-direction: column; gap: 2px; }
.space-meta-grid span { color: var(--text-secondary, #8a8a8a); font-size: 10px; }
.space-meta-grid code { overflow: hidden; color: var(--text-primary); font-size: 11px; text-overflow: ellipsis; white-space: nowrap; }
.space-actions { display: flex; flex-wrap: wrap; gap: 7px; padding-top: 2px; }
.space-note { margin: 0; color: var(--text-secondary, #8a8a8a); font-size: 11px; line-height: 1.5; }
@keyframes pulse { 0%, 100% { opacity: 1; } 50% { opacity: .3; } }
.sync-actions { display: flex; gap: 8px; flex-wrap: wrap; }
.sync-btn { border: 1px solid var(--border-color, #d9d9d9); background: var(--bg-secondary, #fff); color: inherit; border-radius: 8px; padding: 7px 12px; font-size: 12px; cursor: pointer; }
.sync-btn.primary { background: var(--accent-color, #2f6fed); border-color: transparent; color: #fff; }
.sync-btn.danger { color: #c0392b; }
.sync-btn.small { padding: 4px 8px; font-size: 11px; }
.sync-btn:disabled { opacity: .55; cursor: not-allowed; }
.sync-field { display: flex; flex-direction: column; gap: 6px; font-size: 12px; }
.sync-field > span { color: var(--text-secondary, #8a8a8a); }
.sync-field input, .sync-field select { width: 100%; box-sizing: border-box; padding: 8px 10px; border: 1px solid var(--border-color, #d9d9d9); border-radius: 8px; background: var(--bg-primary, #fff); color: inherit; font: inherit; }
.sync-section-head { display: flex; align-items: center; justify-content: space-between; }
.sync-notice { margin: 0; color: #1d7a46; font-size: 12px; }
.sync-error { margin: 0; color: #c0392b; font-size: 12px; }
.share-box { display: flex; flex-direction: column; gap: 8px; padding-top: 4px; border-top: 1px solid var(--border-color, #eee); }
.share-input { width: 100%; box-sizing: border-box; padding: 8px 10px; border: 1px solid var(--border-color, #d9d9d9); border-radius: 8px; background: var(--bg-primary, #fff); color: inherit; font-family: ui-monospace, SFMono-Regular, Menlo, monospace; font-size: 11px; line-height: 1.5; resize: vertical; overflow-wrap: anywhere; }
.share-option { display: flex; align-items: flex-start; gap: 6px; color: var(--text-secondary, #8a8a8a); font-size: 12px; line-height: 1.5; }
.device-row { padding: 7px 0; border-top: 1px solid var(--border-color, #eee); font-size: 12px; }
.device-row:first-of-type { border-top: 0; }
.device-row strong { font-weight: 600; }
.device-current { margin-left: 7px; color: var(--accent-color, #2f6fed); font-size: 11px; }
.device-meta { display: block; margin-top: 3px; color: var(--text-secondary, #8a8a8a); font-size: 11px; }
@media (max-width: 640px) {
  .space-meta-grid { grid-template-columns: minmax(0, 1fr); }
}
</style>
