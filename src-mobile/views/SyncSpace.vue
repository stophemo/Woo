<script setup lang="ts">
import { computed, onMounted, ref, watch } from 'vue'
import { useRouter } from 'vue-router'
import { showConfirmDialog, showToast } from 'vant'
import { useSyncStore } from '../../src/stores/sync'
import webdavSpaceApi, {
  type WebDavDeviceView,
  type WebdavSpaceConfigView,
} from '../../src/services/webdavSpaceApi'
import {
  WEBDAV_PROVIDERS,
  findProvider,
  matchProvider,
} from '../../src/config/webdavProviders'

const router = useRouter()
const syncStore = useSyncStore()

const config = ref<WebdavSpaceConfigView | null>(null)
const providerId = ref('jianguoyun')
const showProviderSheet = ref(false)
const currentProvider = computed(() => findProvider(providerId.value))
const providerActions = WEBDAV_PROVIDERS.map(provider => ({ name: provider.label, id: provider.id }))
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
  config.value = await webdavSpaceApi.getConfig()
  endpoint.value = config.value.account.endpoint || endpoint.value
  username.value = config.value.account.username || username.value
  providerId.value = matchProvider(endpoint.value).id
  if (!endpoint.value) endpoint.value = currentProvider.value.endpoint
  if (config.value.notice) notice.value = config.value.notice
  if (config.value.active) await loadDevices()
  else devices.value = []
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
}

function onProviderSelect(action: { name?: string }) {
  const matched = WEBDAV_PROVIDERS.find(provider => provider.label === action.name)
  if (matched) providerId.value = matched.id
  applyProvider()
  showProviderSheet.value = false
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
    if (config.value?.active) {
      await syncStore.refreshStatus()
      void syncStore.triggerSync()
      showToast(config.value?.notice || '已连接，正在开始同步…')
    } else {
      showToast(config.value?.notice || '账号已保存')
    }
  } catch (err: any) {
    error.value = err?.message || 'WebDAV 连接失败'
    showToast(error.value)
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
    showImport.value = false
    if (parsed.password) {
      password.value = parsed.password
      busy.value = false
      await saveAccount()
      if (config.value?.active) importPayload.value = ''
    } else {
      notice.value = '已填入地址和账户，请补充应用密码后点「保存并同步」。'
      showToast(notice.value)
    }
  } catch (err: any) {
    error.value = err?.message || '配置串解析失败'
    showToast(error.value)
  } finally {
    busy.value = false
  }
}

async function openShare() {
  showShare.value = true
  await generateShare()
}

async function generateShare() {
  try {
    sharePayload.value = await webdavSpaceApi.exportConfig(shareWithPassword.value)
  } catch (err: any) {
    sharePayload.value = ''
    showToast(err?.message || '生成配置串失败')
  }
}

async function copyShare() {
  if (!sharePayload.value) return
  try {
    await navigator.clipboard.writeText(sharePayload.value)
    showToast('配置串已复制，请在另一台设备粘贴')
  } catch {
    showToast('复制失败，请手动选择配置串')
  }
}

async function syncNow() {
  const ok = await syncStore.triggerSync()
  if (!ok) showToast(syncStore.errorMsg || '同步失败')
  else showToast('同步完成')
  await loadDevices()
}

async function leave() {
  try {
    await showConfirmDialog({
      title: '退出同步空间',
      message: '只清除本机身份，本机笔记仍保留。要阻止旧设备继续同步，请修改 WebDAV 应用密码。',
      confirmButtonText: '退出',
    })
  } catch {
    return
  }
  busy.value = true
  try {
    await webdavSpaceApi.leaveSpace()
    devices.value = []
    await loadConfig()
    await syncStore.refreshStatus()
    showToast('已退出同步空间，本机笔记未删除')
  } catch (err: any) {
    showToast(err?.message || '退出同步空间失败')
  } finally {
    busy.value = false
  }
}

onMounted(async () => {
  try {
    await loadConfig()
    await syncStore.refreshStatus()
  } catch (err: any) {
    showToast(err?.message || '读取跨设备配置失败')
  }
})

watch(() => syncStore.lastSyncTime, () => {
  if (config.value?.active) void loadDevices()
})
</script>

<template>
  <div class="sync-page">
    <van-nav-bar title="跨设备" left-arrow fixed placeholder @click-left="router.back()" />

    <p class="hint">
      每台设备填写同一个 WebDAV 账号即可同步：云端没有空间会自动建立，已有空间会自动加入。
      不需要配对码，也不用记空间 ID。WebDAV 没有服务端推送，同步不是实时的。
    </p>

    <van-cell-group inset title="WebDAV 账号">
      <van-cell title="服务商" :value="currentProvider.label" is-link @click="showProviderSheet = true" />
      <van-field
        v-model="endpoint"
        label="服务器地址"
        :placeholder="currentProvider.endpointPlaceholder"
        clearable
      />
      <van-field v-model="username" label="账户" placeholder="WebDAV 账户" clearable />
      <van-field
        v-model="password"
        label="应用密码"
        type="password"
        :placeholder="config?.account.hasPassword ? '已保存，留空不改' : 'WebDAV 应用密码'"
        clearable
      />
      <van-cell :title="busy ? '正在连接…' : '保存并同步'" is-link @click="saveAccount" />
      <van-cell title="从另一台设备导入" is-link @click="showImport = true" />
      <div class="hint-block">{{ currentProvider.passwordHint }}</div>
    </van-cell-group>

    <p v-if="config?.legacyNotice" class="hint">{{ config.legacyNotice }}</p>
    <p v-if="config?.notice" class="hint">{{ config.notice }}</p>
    <p v-if="notice" class="hint">{{ notice }}</p>
    <p v-if="error" class="hint error">{{ error }}</p>

    <template v-if="config?.active">
      <section class="space-hero-mobile">
        <div class="space-hero-mobile-top">
          <div class="space-hero-copy">
            <span class="space-kicker">WebDAV 同步空间</span>
            <strong class="space-endpoint">{{ config.account.endpoint }}</strong>
            <span class="space-account">{{ config.account.username }}</span>
          </div>
          <span class="space-status" :class="{ syncing: syncStore.isSyncing }">
            <i class="status-dot"></i>
            {{ syncStore.isSyncing ? '同步中' : syncStore.lastSyncLabel }}
          </span>
        </div>
        <div class="space-meta-mobile">
          <div><span>本机</span><b>{{ config.identity.deviceName }} · {{ platformLabel(config.identity.platform) }}</b></div>
          <div><span>待同步</span><b>{{ syncStore.pendingChanges || 0 }} 项</b></div>
        </div>
        <div class="space-actions-mobile">
          <van-button size="small" type="primary" :loading="syncStore.isSyncing" @click="syncNow">
            立即同步
          </van-button>
          <van-button size="small" plain @click="openShare">添加设备</van-button>
          <van-button size="small" plain type="danger" @click="leave">退出空间</van-button>
        </div>
        <p class="hint">设备列表来自加密心跳，只读；要阻止旧设备继续同步，请修改 WebDAV 应用密码。</p>
      </section>

      <van-cell-group inset title="已加入设备">
        <van-cell
          v-for="device in devices"
          :key="device.deviceId"
          :title="device.name"
          :label="`${platformLabel(device.platform)} · ${formatLastSeen(device.lastSeenAt)} · v${device.appVersion}${isStale(device.lastSeenAt) ? ' · 很久未同步' : ''}`"
          :value="device.isCurrent ? '当前设备' : ''"
        />
        <van-cell
          v-if="!devices.some(device => device.isCurrent)"
          :title="config.identity.deviceName"
          :label="`${platformLabel(config.identity.platform)} · 尚未同步`"
          value="当前设备"
        />
        <van-cell v-if="deviceError" :title="deviceError" />
        <van-cell
          v-if="devices.filter(device => !device.isCurrent).length === 0"
          title="只有当前设备"
        />
      </van-cell-group>
    </template>

    <van-action-sheet
      v-model:show="showProviderSheet"
      :actions="providerActions"
      cancel-text="取消"
      @select="onProviderSelect"
    />

    <van-popup v-model:show="showImport" position="bottom" round>
      <div class="share-panel">
        <h3>从另一台设备导入</h3>
        <p class="hint">在另一台设备点「添加设备」复制配置串，粘贴到这里。</p>
        <van-field
          v-model="importPayload"
          type="textarea"
          rows="3"
          autosize
          placeholder="WOO-SYNC:1:..."
        />
        <van-button block type="primary" :disabled="busy || !importPayload.trim()" @click="importConfig">
          导入并同步
        </van-button>
        <van-button block plain @click="showImport = false">取消</van-button>
      </div>
    </van-popup>

    <van-popup v-model:show="showShare" position="bottom" round>
      <div class="share-panel">
        <h3>添加设备</h3>
        <p class="hint">
          在另一台设备打开「跨设备 → 从另一台设备导入」，粘贴下面这串配置。
          默认不包含应用密码，另一台设备只需补一次密码。
        </p>
        <van-field
          v-if="sharePayload"
          v-model="sharePayload"
          type="textarea"
          rows="4"
          autosize
          readonly
        />
        <van-checkbox v-model="shareWithPassword" shape="square" @change="generateShare">
          包含应用密码（另一台设备零输入，但配置串等于明文密码）
        </van-checkbox>
        <van-button block type="primary" :disabled="!sharePayload" @click="copyShare">复制配置串</van-button>
        <van-button block plain @click="showShare = false">关闭</van-button>
      </div>
    </van-popup>
  </div>
</template>

<style scoped>
.sync-page { padding: 12px 0 32px; display: flex; flex-direction: column; gap: 12px; }
.hint { margin: 0 16px; color: var(--van-text-color-2); font-size: 12px; line-height: 1.6; }
.hint.error { color: var(--van-danger-color, #c0392b); }
.hint-block { padding: 8px 16px 12px; color: var(--van-text-color-2); font-size: 11px; line-height: 1.6; }
.space-hero-mobile { display: flex; flex-direction: column; gap: 12px; margin: 0 16px; padding: 16px; border: 1px solid color-mix(in srgb, var(--van-primary-color) 14%, var(--van-border-color)); border-radius: 16px; background: linear-gradient(150deg, color-mix(in srgb, var(--van-primary-color) 8%, var(--van-background-2)), var(--van-background-2) 62%); }
.space-hero-mobile-top { display: flex; align-items: flex-start; justify-content: space-between; gap: 12px; }
.space-hero-copy { display: flex; min-width: 0; flex-direction: column; gap: 4px; }
.space-kicker { color: var(--van-text-color-2); font-size: 10px; font-weight: 600; letter-spacing: .1em; }
.space-endpoint { color: var(--van-text-color); font-family: ui-monospace, SFMono-Regular, Menlo, monospace; font-size: 13px; line-height: 1.4; overflow-wrap: anywhere; }
.space-account { color: var(--van-text-color-2); font-size: 11px; }
.space-status { display: inline-flex; flex: 0 0 auto; align-items: center; gap: 5px; padding: 4px 8px; border: 1px solid color-mix(in srgb, var(--van-primary-color) 24%, transparent); border-radius: 999px; background: color-mix(in srgb, var(--van-primary-color) 8%, transparent); color: var(--van-text-color-2); font-size: 10px; }
.space-status .status-dot { width: 6px; height: 6px; border-radius: 50%; background: var(--van-primary-color); }
.space-status.syncing .status-dot { animation: pulse 1.2s ease-in-out infinite; }
.space-meta-mobile { display: flex; flex-direction: column; gap: 6px; }
.space-meta-mobile > div { display: flex; gap: 8px; font-size: 11px; }
.space-meta-mobile span { flex: 0 0 auto; color: var(--van-text-color-2); }
.space-meta-mobile b { min-width: 0; color: var(--van-text-color); font-weight: 500; overflow-wrap: anywhere; }
.space-actions-mobile { display: flex; flex-wrap: wrap; gap: 8px; }
.share-panel { display: flex; flex-direction: column; gap: 12px; padding: 20px 16px calc(20px + env(safe-area-inset-bottom)); }
.share-panel h3 { margin: 0; font-size: 16px; }
.share-panel .hint { margin: 0; }
@keyframes pulse { 0%, 100% { opacity: 1; } 50% { opacity: .3; } }
</style>
