<template>
  <div class="sync-space-section">
    <h3>同步空间</h3>
    <p class="sync-help">
      多台设备通过六位配对码加入同一个端到端加密空间。正文和图片只以密文离开本机，
      服务端只保存密文和必要的同步元数据。
    </p>

    <template v-if="config?.active">
      <div class="sync-box">
        <div class="sync-row"><span>服务地址</span><code>{{ config.endpoint }}</code></div>
        <div class="sync-row"><span>空间</span><code>{{ shortVaultId }}</code></div>
        <div class="sync-row"><span>本机</span><code>{{ config.deviceName }}</code></div>
        <div class="sync-row">
          <span>状态</span>
          <code>{{ syncStore.isSyncing ? '同步中…' : syncStore.lastSyncLabel }}</code>
        </div>
        <p v-if="syncStore.pendingChanges > 0" class="sync-help">待同步 {{ syncStore.pendingChanges }} 项</p>
        <p class="sync-help">
          请保留至少一台已配对设备并备份应用数据目录。当前版本没有恢复码，密钥丢失后服务端密文无法解密。
        </p>
        <div class="sync-actions">
          <button class="sync-btn primary" :disabled="syncStore.isSyncing" @click="syncNow">
            {{ syncStore.isSyncing ? '同步中…' : '立即同步' }}
          </button>
          <button class="sync-btn" :disabled="busy" @click="startPairing">生成六位配对码</button>
          <button class="sync-btn" :disabled="busy" @click="seedLocal">上传本机独有文稿</button>
          <button class="sync-btn danger" :disabled="busy" @click="leave">离开同步空间</button>
        </div>
      </div>

      <div v-if="pairing" class="sync-box pairing-box">
        <div class="sync-row"><span>六位配对码</span><strong class="pairing-code">{{ pairing.shortCode }}</strong></div>
        <p class="sync-help">在新设备上选择「加入已有空间」，输入这 6 位数字。配对码 {{ pairingExpiryLabel }} 前有效。</p>
        <div v-if="pairing.claimedDevice" class="pairing-claim">
          <p class="sync-help">新设备请求加入：</p>
          <div class="sync-row"><span>设备</span><code>{{ pairing.claimedDevice.name }}</code></div>
          <div class="sync-row"><span>平台</span><code>{{ platformLabel(pairing.claimedDevice.platform) }}</code></div>
          <div class="sync-row"><span>核对码</span><strong class="pairing-code">{{ pairing.verificationCode || '------' }}</strong></div>
          <p class="sync-help">请确认新设备屏幕显示相同的六位核对码，再点击确认。</p>
          <button class="sync-btn primary" :disabled="busy" @click="confirmPairing">确认新设备加入</button>
        </div>
        <p v-else class="sync-help">等待新设备输入配对码…</p>
        <p v-if="pairing.status === 'expired' || pairing.status === 'canceled'" class="sync-error">配对码已过期，请重新生成。</p>
      </div>

      <div class="sync-box">
        <div class="sync-section-head">
          <h4>已加入设备</h4>
          <button class="sync-btn small" :disabled="busy" @click="loadDevices">刷新</button>
        </div>
        <div v-for="device in devices" :key="device.id" class="device-row">
          <div>
            <strong>{{ device.name }}</strong>
            <span class="device-meta">{{ platformLabel(device.platform) }}<template v-if="device.isCurrent"> · 当前设备</template></span>
          </div>
          <button
            v-if="!device.isCurrent"
            class="sync-btn small danger"
            :disabled="busy"
            @click="revoke(device)"
          >撤销</button>
        </div>
        <p v-if="devices.length === 0" class="sync-help">尚未读取设备列表。</p>
        <p class="sync-help">每个空间最多 4 台设备；换机时先撤销旧设备。</p>
      </div>
    </template>

    <template v-else>
      <div class="sync-box">
        <label class="sync-field">
          <span>同步服务地址</span>
          <input v-model="endpoint" type="url" placeholder="https://woo-sync.example.workers.dev" autocomplete="off" spellcheck="false" />
        </label>
        <label class="sync-field">
          <span>本机名称</span>
          <input v-model="deviceName" type="text" maxlength="80" placeholder="例如 MacBook Air" autocomplete="off" />
        </label>
        <div class="sync-actions">
          <button class="sync-btn" :disabled="busy" @click="testConnection">测试连接</button>
        </div>
      </div>

      <div class="sync-box">
        <h4>加入已有空间</h4>
        <p class="sync-help">在已加入的设备上点击「生成六位配对码」，然后在这里输入。</p>
        <label class="sync-field">
          <span>六位配对码</span>
          <input
            v-model="joinCode"
            type="text"
            inputmode="numeric"
            maxlength="6"
            placeholder="000000"
            class="pairing-input"
            autocomplete="one-time-code"
          />
        </label>
        <div class="sync-actions">
          <button class="sync-btn primary" :disabled="busy || joinCode.length !== 6" @click="joinVault">加入空间</button>
        </div>
        <p v-if="joinStatus" class="sync-help">{{ joinStatus }}</p>
        <p v-if="joinVerification" class="sync-row"><span>核对码</span><strong class="pairing-code">{{ joinVerification }}</strong></p>
      </div>

      <div class="sync-box">
        <h4>创建新空间</h4>
        <p class="sync-help">第一台设备创建空间；创建邀请码由部署同步服务时设置。</p>
        <label class="sync-field">
          <span>创建邀请码</span>
          <input v-model="inviteCode" type="password" placeholder="部署时设置的邀请码" autocomplete="off" />
        </label>
        <div class="sync-actions">
          <button class="sync-btn primary" :disabled="busy" @click="createVault">创建空间</button>
        </div>
        <p class="sync-help">
          如果这台设备之前使用 WebDAV，建议先在下方「旧版同步」中完成一次同步，
          确认本机数据是最新的，再创建空间。
        </p>
      </div>
    </template>

    <p v-if="notice" class="sync-notice">{{ notice }}</p>
    <p v-if="error" class="sync-error">{{ error }}</p>
  </div>
</template>

<script setup lang="ts">
import { computed, onBeforeUnmount, onMounted, ref } from 'vue'
import { useSyncStore } from '../../stores/sync'
import vaultSyncApi, {
  type PairingCodeView,
  type PairingStatusView,
  type VaultDeviceView,
  type VaultSyncConfigView,
} from '../../services/vaultSyncApi'

const syncStore = useSyncStore()
const config = ref<VaultSyncConfigView | null>(null)
const endpoint = ref('')
const deviceName = ref('')
const inviteCode = ref('')
const joinCode = ref('')
const joinStatus = ref('')
const joinVerification = ref('')
const notice = ref('')
const error = ref('')
const busy = ref(false)
const devices = ref<VaultDeviceView[]>([])
const pairing = ref<(PairingCodeView & Partial<PairingStatusView>) | null>(null)

let pairingTimer: ReturnType<typeof setInterval> | null = null
let joinTimer: ReturnType<typeof setInterval> | null = null
let joinPairingId = ''

const shortVaultId = computed(() => {
  const id = config.value?.vaultId || ''
  return id ? `${id.slice(0, 8)}…` : ''
})

const pairingExpiryLabel = computed(() => {
  if (!pairing.value) return ''
  return new Date(pairing.value.expiresAt).toLocaleTimeString('zh-CN', {
    hour: '2-digit',
    minute: '2-digit',
  })
})

function stopPairingTimer() {
  if (pairingTimer) clearInterval(pairingTimer)
  pairingTimer = null
}

function stopJoinTimer() {
  if (joinTimer) clearInterval(joinTimer)
  joinTimer = null
}

async function loadConfig() {
  try {
    config.value = await vaultSyncApi.getConfig()
    if (!deviceName.value) deviceName.value = config.value.deviceName || 'Woo 设备'
    if (!endpoint.value) endpoint.value = config.value.endpoint || ''
  } catch (err: any) {
    error.value = err?.message || '读取同步配置失败'
  }
}

async function loadDevices() {
  if (!config.value?.active) return
  try {
    devices.value = await vaultSyncApi.listDevices()
  } catch (err: any) {
    error.value = err?.message || '读取设备列表失败'
  }
}

async function testConnection() {
  if (busy.value) return
  busy.value = true
  error.value = ''
  notice.value = ''
  try {
    notice.value = await vaultSyncApi.test(endpoint.value)
  } catch (err: any) {
    error.value = err?.message || '连接失败'
  } finally {
    busy.value = false
  }
}

async function createVault() {
  if (busy.value) return
  busy.value = true
  error.value = ''
  notice.value = ''
  try {
    config.value = await vaultSyncApi.createVault(endpoint.value, inviteCode.value, deviceName.value)
    inviteCode.value = ''
    notice.value = '同步空间已创建，正在上传本机数据…'
    await syncStore.refreshStatus()
    void syncStore.triggerSync()
    await loadDevices()
  } catch (err: any) {
    error.value = err?.message || '创建同步空间失败'
  } finally {
    busy.value = false
  }
}

async function startPairing() {
  if (busy.value) return
  busy.value = true
  error.value = ''
  notice.value = ''
  try {
    const code = await vaultSyncApi.startPairing()
    pairing.value = { ...code, status: 'open', claimedDevice: null, verificationCode: null }
    stopPairingTimer()
    pairingTimer = setInterval(pollPairing, 2000)
  } catch (err: any) {
    error.value = err?.message || '生成配对码失败'
  } finally {
    busy.value = false
  }
}

async function pollPairing() {
  if (!pairing.value) return
  try {
    const status = await vaultSyncApi.pairingStatus(pairing.value.pairingId)
    pairing.value = { ...pairing.value, ...status }
    if (status.status === 'expired' || status.status === 'canceled' || status.status === 'confirmed') {
      stopPairingTimer()
    }
  } catch (err: any) {
    error.value = err?.message || '查询配对状态失败'
    stopPairingTimer()
  }
}

async function confirmPairing() {
  if (!pairing.value || busy.value) return
  busy.value = true
  error.value = ''
  try {
    await vaultSyncApi.confirmPairing(pairing.value.pairingId)
    pairing.value = { ...pairing.value, status: 'confirmed' }
    stopPairingTimer()
    notice.value = '新设备已加入同步空间。'
    await loadDevices()
  } catch (err: any) {
    error.value = err?.message || '确认新设备失败'
  } finally {
    busy.value = false
  }
}

async function joinVault() {
  if (busy.value) return
  busy.value = true
  error.value = ''
  notice.value = ''
  joinStatus.value = '正在认领配对码…'
  try {
    const start = await vaultSyncApi.beginJoin(endpoint.value, joinCode.value, deviceName.value)
    joinPairingId = start.pairingId
    joinVerification.value = start.verificationCode
    joinStatus.value = '等待已有设备确认…'
    stopJoinTimer()
    joinTimer = setInterval(pollJoin, 2000)
  } catch (err: any) {
    error.value = err?.message || '加入同步空间失败'
    joinStatus.value = ''
    busy.value = false
  }
}

async function pollJoin() {
  if (!joinPairingId) return
  try {
    const result = await vaultSyncApi.pollJoin(joinPairingId)
    if (result.configured) {
      stopJoinTimer()
      joinPairingId = ''
      joinCode.value = ''
      joinStatus.value = '已加入同步空间，正在拉取远端数据…'
      notice.value = '加入成功。'
      await loadConfig()
      await syncStore.refreshStatus()
      void syncStore.triggerSync()
      await loadDevices()
      busy.value = false
      return
    }
    if (result.status === 'expired' || result.status === 'canceled') {
      stopJoinTimer()
      joinPairingId = ''
      joinStatus.value = '配对码已过期，请在已有设备上重新生成。'
      busy.value = false
    }
  } catch (err: any) {
    stopJoinTimer()
    joinPairingId = ''
    error.value = err?.message || '加入同步空间失败'
    busy.value = false
  }
}

async function syncNow() {
  error.value = ''
  const ok = await syncStore.triggerSync()
  if (!ok) error.value = syncStore.errorMsg || '同步失败'
  else notice.value = '同步完成。'
  await loadDevices()
}

async function seedLocal() {
  if (busy.value) return
  if (!window.confirm('只会上传远端空间中不存在的本机文稿，不会覆盖已有内容。继续吗？')) return
  busy.value = true
  error.value = ''
  notice.value = ''
  try {
    const count = await vaultSyncApi.seedLocal()
    notice.value = count > 0
      ? `已加入 ${count} 项本机独有内容，正在上传…`
      : '没有需要上传的本机独有文稿。'
    if (count > 0) void syncStore.triggerSync()
  } catch (err: any) {
    error.value = err?.message || '登记本机文稿失败'
  } finally {
    busy.value = false
  }
}

async function revoke(device: VaultDeviceView) {
  if (busy.value) return
  if (!window.confirm(`撤销「${device.name}」后，它需要重新配对才能继续同步。确定撤销吗？`)) return
  busy.value = true
  error.value = ''
  try {
    await vaultSyncApi.revokeDevice(device.id)
    notice.value = `已撤销 ${device.name}。`
    await loadDevices()
  } catch (err: any) {
    error.value = err?.message || '撤销设备失败'
  } finally {
    busy.value = false
  }
}

async function leave() {
  if (busy.value) return
  if (!window.confirm('离开同步空间后，本机笔记仍会保留，但不再与其他设备同步。确定离开吗？')) return
  busy.value = true
  error.value = ''
  try {
    await vaultSyncApi.leave()
    config.value = await vaultSyncApi.getConfig()
    devices.value = []
    pairing.value = null
    stopPairingTimer()
    stopJoinTimer()
    notice.value = '已离开同步空间，本机笔记未删除。'
    await syncStore.refreshStatus()
  } catch (err: any) {
    error.value = err?.message || '离开同步空间失败'
  } finally {
    busy.value = false
  }
}

function platformLabel(platform: string) {
  return ({ macos: 'macOS', windows: 'Windows', linux: 'Linux', android: 'Android', ios: 'iOS' } as Record<string, string>)[platform] || platform
}

onMounted(async () => {
  await loadConfig()
  await syncStore.refreshStatus()
  await loadDevices()
})

onBeforeUnmount(() => {
  stopPairingTimer()
  stopJoinTimer()
})
</script>

<style scoped>
.sync-space-section { display: flex; flex-direction: column; gap: 12px; }
.sync-space-section h3 { margin: 0; font-size: 15px; }
.sync-space-section h4 { margin: 0 0 8px; font-size: 13px; }
.sync-help { margin: 0; color: var(--text-secondary, #8a8a8a); font-size: 12px; line-height: 1.6; }
.sync-box { border: 1px solid var(--border-color, #e5e5e5); border-radius: 10px; padding: 12px; display: flex; flex-direction: column; gap: 10px; }
.sync-row { display: flex; gap: 10px; align-items: baseline; font-size: 12px; }
.sync-row > span { color: var(--text-secondary, #8a8a8a); min-width: 64px; }
.sync-row code { overflow-wrap: anywhere; }
.sync-row strong { font-size: 16px; letter-spacing: 1px; }
.sync-actions { display: flex; gap: 8px; flex-wrap: wrap; }
.sync-btn { border: 1px solid var(--border-color, #d9d9d9); background: var(--bg-secondary, #fff); color: inherit; border-radius: 8px; padding: 7px 12px; font-size: 12px; cursor: pointer; }
.sync-btn.primary { background: var(--accent-color, #2f6fed); border-color: transparent; color: #fff; }
.sync-btn.danger { color: #c0392b; }
.sync-btn.small { padding: 4px 8px; font-size: 11px; }
.sync-btn:disabled { opacity: .55; cursor: not-allowed; }
.sync-field { display: flex; flex-direction: column; gap: 6px; font-size: 12px; }
.sync-field > span { color: var(--text-secondary, #8a8a8a); }
.sync-field input { width: 100%; box-sizing: border-box; padding: 8px 10px; border: 1px solid var(--border-color, #d9d9d9); border-radius: 8px; background: var(--bg-primary, #fff); color: inherit; font: inherit; }
.sync-field input.pairing-input { font-size: 20px; letter-spacing: 8px; text-align: center; }
.pairing-box { border-color: var(--accent-color, #2f6fed); }
.pairing-code { font-size: 24px; letter-spacing: 6px; }
.pairing-claim { display: flex; flex-direction: column; gap: 8px; }
.sync-section-head { display: flex; align-items: center; justify-content: space-between; }
.device-row { display: flex; align-items: center; justify-content: space-between; gap: 10px; padding: 6px 0; border-top: 1px solid var(--border-color, #eee); font-size: 12px; }
.device-row:first-of-type { border-top: 0; }
.device-meta { display: block; color: var(--text-secondary, #8a8a8a); font-size: 11px; margin-top: 2px; }
.sync-notice { margin: 0; color: #1d7a46; font-size: 12px; }
.sync-error { margin: 0; color: #c0392b; font-size: 12px; }
</style>
