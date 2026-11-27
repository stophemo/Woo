<template>
  <div class="sync-space-section">
    <h3>跨设备</h3>
    <p class="sync-help">
      每台设备只需要填写同一个 WebDAV 账号，保存后自动开始同步；
      云端已有空间时会直接加入，不需要再输入配对码。
      WebDAV 没有服务端推送，同步不是实时的。
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
      </div>
      <p v-if="config?.account.configured" class="sync-help">
        已保存：{{ config.account.username }} @ {{ config.account.endpoint }}
      </p>
    </div>

    <p v-if="config?.legacyNotice" class="sync-notice">{{ config.legacyNotice }}</p>
    <p v-if="config?.notice" class="sync-notice">{{ config.notice }}</p>

    <template v-if="config?.account.active && !config?.spaceConfigured && !guestPairing && !hostPairing">
      <p class="sync-help">
        配对只需要两边各点一次：<strong>已有笔记的一方</strong>生成六位码，<strong>新设备</strong>输入它。
        下面两种方式选一种即可，不要两边都点「生成配对码」。
      </p>
      <div class="sync-choice-grid">
      <div class="sync-box sync-choice">
        <h4>方式一：输入另一台设备的六位码</h4>
        <p class="sync-help">如果笔记在那台设备上：先在那台点「显示配对码」，把六位数字填到这里。对方批准后本机自动完成加入。</p>
        <label class="sync-field">
          <span>另一台设备显示的六位码</span>
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
          <button class="sync-btn primary" :disabled="busy || joinCode.length !== 6" @click="requestJoinOffer(joinCode)">
            输入并加入
          </button>
        </div>
      </div>

      <div class="sync-box sync-choice">
        <h4>方式二：让另一台设备输入本机码</h4>
        <p class="sync-help">如果笔记在本机上：生成本机六位码，再去那台已有空间的设备输入并批准。</p>
        <label class="sync-field">
          <span>本机名称</span>
          <input v-model="deviceName" type="text" maxlength="80" placeholder="例如新手机" autocomplete="off" />
        </label>
        <div class="sync-actions">
          <button class="sync-btn" :disabled="busy" @click="requestStartJoin">显示我的配对码</button>
        </div>
      </div>

      <details class="sync-box sync-choice span-2 sync-advanced">
        <summary>两台设备都没有同步空间：建立新空间</summary>
        <p class="sync-help">第一台设备建立空间后，其他设备用上面的六位配对码加入。请确认本机数据是当前最新版本。</p>
        <p class="sync-warning">如果已经在其他设备上建过空间，不要点这里；关掉这一段，走上面的「方式一」。</p>
        <label class="sync-field">
          <span>本机名称</span>
          <input v-model="deviceName" type="text" maxlength="80" placeholder="例如 MacBook Air" autocomplete="off" />
        </label>
        <div class="sync-actions">
          <button class="sync-btn" :disabled="busy" @click="createSpace">建立同步空间</button>
        </div>
      </details>
      </div>
    </template>

    <template v-if="config?.active">
      <div class="space-hero">
        <div class="space-hero-top">
          <div class="space-hero-copy">
            <span class="space-kicker">WebDAV 同步空间</span>
            <strong class="space-fingerprint">{{ config.identity.spaceFingerprint }}</strong>
            <button class="space-id-copy" type="button" @click="copySpaceId">
              <span>空间 ID</span>
              <code>{{ config.identity.vaultId }}</code>
              <span class="copy-label">复制</span>
            </button>
          </div>
          <span class="space-status" :class="{ syncing: syncStore.isSyncing }">
            <span class="status-dot"></span>
            {{ syncStore.isSyncing ? '同步中' : syncStore.lastSyncLabel }}
          </span>
        </div>
        <div class="space-meta-grid">
          <div><span>WebDAV</span><code>{{ config.account.endpoint }}</code></div>
          <div><span>账号</span><code>{{ config.account.username }}</code></div>
          <div><span>本机</span><code>{{ config.identity.deviceName }} · {{ platformLabel(config.identity.platform) }}</code></div>
          <div><span>待同步</span><code>{{ syncStore.pendingChanges || 0 }} 项</code></div>
        </div>
        <div class="space-actions">
          <button class="sync-btn primary" :disabled="syncStore.isSyncing" @click="syncNow">
            {{ syncStore.isSyncing ? '同步中…' : '立即同步' }}
          </button>
          <button class="sync-btn" :disabled="busy" @click="startOffer">① 让新设备加入（显示配对码）</button>
          <button class="sync-btn" :disabled="busy" @click="showHostJoinInput = !showHostJoinInput">
            ② 输入新设备码并批准
          </button>
          <button class="sync-btn" :disabled="busy" @click="showJoinOtherInput = !showJoinOtherInput">
            加入其他空间
          </button>
          <button class="sync-btn danger" :disabled="busy" @click="leave">退出空间</button>
        </div>
        <p class="space-note">不是实时同步；设备列表来自加密心跳，只读，不能远程吊销。</p>
      </div>

      <div v-if="showHostJoinInput" class="sync-box">
        <h4>输入新设备显示的六位码</h4>
        <label class="sync-field">
          <span>六位配对码</span>
          <input
            v-model="hostJoinCode"
            type="text"
            inputmode="numeric"
            maxlength="6"
            placeholder="000000"
            class="pairing-input"
            autocomplete="one-time-code"
          />
        </label>
        <div class="sync-actions">
          <button class="sync-btn primary" :disabled="busy || hostJoinCode.length !== 6" @click="findJoin">
            查找并核对
          </button>
          <button class="sync-btn" :disabled="busy" @click="cancelHostJoin">取消</button>
        </div>
      </div>

      <div v-if="showJoinOtherInput" class="sync-box">
        <h4>加入其他空间</h4>
        <p class="sync-help">会先检查本机数据；如果本机已有空间，你需要选择以哪台设备的数据为准。</p>
        <label class="sync-field">
          <span>对方设备显示的六位码</span>
          <input
            v-model="joinOtherCode"
            type="text"
            inputmode="numeric"
            maxlength="6"
            placeholder="000000"
            class="pairing-input"
            autocomplete="one-time-code"
          />
        </label>
        <div class="sync-actions">
          <button class="sync-btn primary" :disabled="busy || joinOtherCode.length !== 6" @click="requestJoinOffer(joinOtherCode)">
            查找并加入
          </button>
          <button class="sync-btn" :disabled="busy" @click="requestStartJoin">
            让已有设备输入本机配对码
          </button>
          <button class="sync-btn" :disabled="busy" @click="closeJoinOther">取消</button>
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
        <p v-if="config?.active && devices.filter(device => !device.isCurrent).length === 0" class="sync-help">只有当前设备。</p>
        <p v-if="deviceError" class="sync-error">{{ deviceError }}</p>
        <p class="sync-help">只读列表，不能移除设备；旧设备仍持有 vaultKey，只能通过修改 WebDAV 应用密码阻止它继续访问。</p>
      </div>
    </template>

    <div v-if="hostPairing" class="sync-box pairing-box">
      <div class="sync-section-head">
        <h4>已有设备：批准新设备</h4>
        <button class="sync-btn small" :disabled="busy" @click="cancelPairing">取消配对</button>
      </div>
      <div v-if="hostPairing.shortCode" class="sync-row">
        <span>六位配对码</span><strong class="pairing-code">{{ hostPairing.shortCode }}</strong>
      </div>
      <div v-if="hostPairing.peer" class="sync-row">
        <span>新设备</span><code>{{ hostPairing.peer.name }} · {{ platformLabel(hostPairing.peer.platform) }}</code>
      </div>
      <div v-if="hostPairing.verificationCode" class="sync-row">
        <span>核对码</span><strong class="pairing-code">{{ hostPairing.verificationCode }}</strong>
      </div>
      <div v-if="hostPairing.spaceFingerprint" class="sync-row">
        <span>空间指纹</span><code>{{ hostPairing.spaceFingerprint }}</code>
      </div>
      <p class="sync-help">
        {{ hostPairing.message || (hostPairing.status === 'waiting' ? `请在另一台设备输入 ${hostPairing.shortCode}…` : '') }}
      </p>
      <p v-if="hostPairing.status === 'claimed'" class="sync-warning">
        请确认新设备屏幕显示相同的六位核对码和空间指纹。如果不是你正在操作的设备，请不要批准。
      </p>
      <div v-if="hostPairing.status === 'claimed'" class="sync-actions">
        <button class="sync-btn primary" :disabled="busy" @click="approveHost">核对一致，批准加入</button>
      </div>
      <p v-if="hostPairing.status === 'approved'" class="sync-notice">已批准，新设备会自动完成加入。</p>
      <p v-if="hostPairing.status === 'expired'" class="sync-error">配对码已过期，请重新生成。</p>
      <p v-if="hostPairing.expiresAt" class="sync-help">
        有效期至 {{ formatTime(hostPairing.expiresAt) }}（{{ remainingLabel(hostPairing.expiresAt) }}）
      </p>
    </div>

    <div v-if="guestPairing" class="sync-box pairing-box">
      <div class="sync-section-head">
        <h4>新设备：等待加入</h4>
        <button class="sync-btn small" :disabled="busy" @click="cancelPairing">取消配对</button>
      </div>
      <div v-if="guestPairing.shortCode" class="sync-row">
        <span>六位配对码</span><strong class="pairing-code">{{ guestPairing.shortCode }}</strong>
      </div>
      <div v-if="guestPairing.peer" class="sync-row">
        <span>已有设备</span><code>{{ guestPairing.peer.name }} · {{ platformLabel(guestPairing.peer.platform) }}</code>
      </div>
      <div v-if="guestPairing.verificationCode" class="sync-row">
        <span>核对码</span><strong class="pairing-code">{{ guestPairing.verificationCode }}</strong>
      </div>
      <div v-if="guestPairing.spaceFingerprint" class="sync-row">
        <span>空间指纹</span><code>{{ guestPairing.spaceFingerprint }}</code>
      </div>
      <p class="sync-help">{{ guestPairing.message || '等待已有设备批准；批准后本机会自动完成加入。' }}</p>
      <p v-if="guestPairing.status === 'ready'" class="sync-warning">
        已有设备已批准，正在自动完成加入…
      </p>
      <p v-else class="sync-help">人工确认只在已有设备上完成；本机不需要再点确认。</p>
      <p v-if="guestPairing.status === 'expired'" class="sync-error">配对码已过期，请重新生成。</p>
      <p v-if="guestPairing.expiresAt" class="sync-help">
        有效期至 {{ formatTime(guestPairing.expiresAt) }}（{{ remainingLabel(guestPairing.expiresAt) }}）
      </p>
    </div>

    <div v-if="joinPrompt" class="join-prompt-overlay" @click.self="cancelJoinPrompt">
      <div class="join-prompt-dialog">
        <h4>{{ joinPrompt.state.hasIdentity ? '选择以哪台设备的数据为准' : '加入后以对方数据为准' }}</h4>
        <p class="join-prompt-message">{{ joinPromptMessage }}</p>
        <p class="sync-warning">
          选择以对方为准后只会开始配对；只有对方批准成功后才会清空本机文稿。
          取消、过期或未批准都不会清空。清空只影响本机，不删除 WebDAV 上的旧空间。
        </p>
        <div class="sync-actions">
          <button class="sync-btn primary" :disabled="busy" @click="clearAndContinue">
            以对方为准（批准后清空并加入）
          </button>
          <button v-if="joinPrompt.state.hasIdentity" class="sync-btn" :disabled="busy" @click="joinUsingLocal">
            以本机为准（请在对方设备输入本机配对码）
          </button>
          <button class="sync-btn" :disabled="busy" @click="cancelJoinPrompt">取消</button>
        </div>
      </div>
    </div>

    <p v-if="notice" class="sync-notice">{{ notice }}</p>
    <p v-if="error" class="sync-error">{{ error }}</p>
  </div>
</template>

<script setup lang="ts">
import { computed, onBeforeUnmount, onMounted, ref, watch } from 'vue'
import { useSyncStore } from '../../stores/sync'
import { useWorkspaceStore } from '../../stores/workspace'
import webdavSpaceApi, {
  type JoinLocalStateView,
  type OfferPreviewView,
  type PairingStatusView,
  type WebDavDeviceView,
  type WebdavSpaceConfigView,
} from '../../services/webdavSpaceApi'
import {
  WEBDAV_PROVIDERS,
  findProvider,
  matchProvider,
} from '../../config/webdavProviders'

interface PendingJoin {
  mode: 'offer' | 'join'
  code: string
  preview: OfferPreviewView | null
  state: JoinLocalStateView
  draftCount: number
}

const syncStore = useSyncStore()
const workspaceStore = useWorkspaceStore()
const config = ref<WebdavSpaceConfigView | null>(null)
const providerId = ref('jianguoyun')
const currentProvider = computed(() => findProvider(providerId.value))
const endpoint = ref('')
const username = ref('')
const password = ref('')
const deviceName = ref('')
const joinCode = ref('')
const joinOtherCode = ref('')
const hostJoinCode = ref('')
const showHostJoinInput = ref(false)
const showJoinOtherInput = ref(false)
const notice = ref('')
const error = ref('')
const deviceError = ref('')
const busy = ref(false)
const devices = ref<WebDavDeviceView[]>([])
const hostPairing = ref<(PairingStatusView & { shortCode?: string }) | null>(null)
const guestPairing = ref<(PairingStatusView & { shortCode?: string }) | null>(null)
const joinPrompt = ref<PendingJoin | null>(null)
const guestAdoptPeer = ref(false)

let hostTimer: ReturnType<typeof setInterval> | null = null
let guestTimer: ReturnType<typeof setInterval> | null = null
let tickTimer: ReturnType<typeof setInterval> | null = null
let pollingHost = false
let pollingGuest = false
let hostMode: 'offer' | 'join' = 'offer'
let hostPollFailures = 0
let guestPollFailures = 0
const MAX_POLL_FAILURES = 8
const nowTick = ref(Date.now())
const GUEST_RESUME_KEY = 'woo.webdav.pairing.guest.v1'
const HOST_RESUME_KEY = 'woo.webdav.pairing.host.v1'

type GuestResume = PairingStatusView & { shortCode: string; adoptPeer: boolean }
type HostResume = PairingStatusView & { shortCode: string; mode: 'offer' | 'join' }

/** 配对中途切走设置页/切后台时，前端状态会丢；把最小状态存本地，回来接着轮询。 */
function saveGuestResume() {
  if (!guestPairing.value) return
  try {
    const payload: GuestResume = { ...guestPairing.value, shortCode: guestPairing.value.shortCode || '', adoptPeer: guestAdoptPeer.value }
    localStorage.setItem(GUEST_RESUME_KEY, JSON.stringify(payload))
  } catch { /* localStorage 不可用时忽略 */ }
}

function clearGuestResume() {
  try { localStorage.removeItem(GUEST_RESUME_KEY) } catch { /* ignore */ }
}

function saveHostResume() {
  if (!hostPairing.value) return
  try {
    const payload: HostResume = { ...hostPairing.value, shortCode: hostPairing.value.shortCode || '', mode: hostMode }
    localStorage.setItem(HOST_RESUME_KEY, JSON.stringify(payload))
  } catch { /* ignore */ }
}

function clearHostResume() {
  try { localStorage.removeItem(HOST_RESUME_KEY) } catch { /* ignore */ }
}

function restorePairing() {
  try {
    const rawGuest = localStorage.getItem(GUEST_RESUME_KEY)
    if (rawGuest && !guestPairing.value) {
      const saved = JSON.parse(rawGuest) as GuestResume
      if (saved?.pairingId && saved.expiresAt > Date.now()) {
        guestAdoptPeer.value = !!saved.adoptPeer
        guestPairing.value = { ...saved, message: '正在恢复上一次配对…' }
        stopGuestTimer()
        guestTimer = setInterval(pollGuest, 2000)
        void pollGuest()
      } else {
        clearGuestResume()
      }
    }
    const rawHost = localStorage.getItem(HOST_RESUME_KEY)
    if (rawHost && !hostPairing.value) {
      const saved = JSON.parse(rawHost) as HostResume
      if (saved?.pairingId && saved.expiresAt > Date.now()) {
        hostMode = saved.mode === 'join' ? 'join' : 'offer'
        hostPairing.value = { ...saved, message: '正在恢复上一次配对…' }
        stopHostTimer()
        if (hostMode === 'offer') {
          hostTimer = setInterval(pollHost, 2000)
          void pollHost()
        }
      } else {
        clearHostResume()
      }
    }
  } catch {
    clearGuestResume()
    clearHostResume()
  }
}

const spaceCaption = computed(() => config.value?.identity.spaceFingerprint || '')
const joinPromptMessage = computed(() => {
  const prompt = joinPrompt.value
  if (!prompt) return ''
  const parts: string[] = []
  if (prompt.state.hasIdentity) {
    const vault = prompt.state.vaultId ? `${prompt.state.vaultId.slice(0, 8)}…${prompt.state.vaultId.slice(-8)}` : '未知'
    const lastSync = prompt.state.lastSyncTime
      ? new Date(prompt.state.lastSyncTime).toLocaleString('zh-CN')
      : '尚未成功同步'
    parts.push(`本机已有同步空间 ${vault}（最近同步：${lastSync}）。`)
  }
  if (prompt.state.hasLocalData) {
    parts.push(`本机有 ${prompt.state.documentCount} 篇文稿、${prompt.state.folderCount} 个目录、${prompt.state.versionCount} 条版本记录（其中回收站 ${prompt.state.trashCount} 篇）。`)
  }
  if (prompt.draftCount > 0) {
    parts.push(`另有 ${prompt.draftCount} 篇本地草稿。`)
  }
  if (prompt.preview) {
    parts.push(`对方设备：${prompt.preview.peer.name} · ${platformLabel(prompt.preview.peer.platform)}。`)
    parts.push(`对方空间指纹：${prompt.preview.spaceFingerprint}。`)
  } else if (prompt.mode === 'join') {
    parts.push('对方设备信息将在已有设备批准后显示；本机现在还不能读取对方设备名。')
  }
  return parts.join('\n')
})

function stopHostTimer() {
  if (hostTimer) clearInterval(hostTimer)
  hostTimer = null
}

function stopGuestTimer() {
  if (guestTimer) clearInterval(guestTimer)
  guestTimer = null
}

function formatTime(ms: number) {
  return new Date(ms).toLocaleTimeString('zh-CN', { hour: '2-digit', minute: '2-digit' })
}

/** 配对码剩余时间；过期后返回“已过期”。 */
function remainingLabel(expiresAt?: number | null) {
  if (!expiresAt) return ''
  const left = expiresAt - nowTick.value
  if (left <= 0) return '已过期'
  const total = Math.floor(left / 1000)
  return `${Math.floor(total / 60)}:${String(total % 60).padStart(2, '0')} 后过期`
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

function platformLabel(platform: string) {
  return ({ macos: 'macOS', windows: 'Windows', linux: 'Linux', android: 'Android', ios: 'iOS' } as Record<string, string>)[platform] || platform
}

function isStale(lastSeenAt: number) {
  return Date.now() - lastSeenAt > 30 * 24 * 60 * 60 * 1000
}

/** 这些错误重试也没用，应当立即停下并让用户重新配对。 */
function isFatalPairingError(message: string) {
  return /配对会话不存在|必须选择以对方为准|已拒绝|配对码已过期/.test(message)
}

async function copySpaceId() {
  const id = config.value?.identity.vaultId
  if (!id) return
  try {
    await navigator.clipboard.writeText(id)
    notice.value = '空间 ID 已复制。'
  } catch {
    notice.value = '复制失败，请手动选择空间 ID。'
  }
}

async function loadConfig() {
  try {
    config.value = await webdavSpaceApi.getConfig()
    endpoint.value = config.value.account.endpoint || endpoint.value
    username.value = config.value.account.username || username.value
    providerId.value = matchProvider(endpoint.value).id
    if (!endpoint.value) endpoint.value = currentProvider.value.endpoint
    if (!deviceName.value) {
      deviceName.value = config.value.identity.deviceName || 'Woo 设备'
    }
    if (config.value.notice) notice.value = config.value.notice
    if (config.value.active) await loadDevices()
    else devices.value = []
  } catch (err: any) {
    error.value = err?.message || '读取跨设备配置失败'
  }
}

/** 切换服务商时填入对应预设；没有预设的服务商清空，避免沿用上一个供应商的地址。 */
function applyProvider() {
  endpoint.value = currentProvider.value.endpoint
  error.value = ''
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

async function createSpace() {
  if (busy.value) return
  busy.value = true
  error.value = ''
  notice.value = ''
  try {
    await webdavSpaceApi.createSpace(deviceName.value)
    notice.value = '同步空间已建立，正在上传本机数据…'
    await loadConfig()
    await syncStore.refreshStatus()
    void syncStore.triggerSync()
  } catch (err: any) {
    error.value = err?.message || '建立同步空间失败'
  } finally {
    busy.value = false
  }
}

async function resetPairing() {
  stopHostTimer()
  stopGuestTimer()
  hostPairing.value = null
  guestPairing.value = null
  clearHostResume()
  clearGuestResume()
  await webdavSpaceApi.cancelPairing().catch(() => {})
}

function localDraftCount() {
  return workspaceStore.drafts.length + workspaceStore.trashDrafts.length
}

async function requestJoinOffer(code: string) {
  if (busy.value) return
  if (!/^\d{6}$/.test(code)) {
    error.value = '请输入六位配对码'
    return
  }
  busy.value = true
  error.value = ''
  notice.value = ''
  try {
    const preview = await webdavSpaceApi.previewOffer(code)
    const state = await webdavSpaceApi.joinState()
    const drafts = localDraftCount()
    if (!state.hasIdentity && !state.hasLocalData && drafts === 0) {
      await beginFindOffer(code, preview.vaultId, false)
      return
    }
    joinPrompt.value = { mode: 'offer', code, preview, state, draftCount: drafts }
  } catch (err: any) {
    error.value = err?.message || '读取对方配对码失败'
  } finally {
    busy.value = false
  }
}

async function beginFindOffer(code: string, expectedVaultId?: string, adoptPeer = false) {
  showJoinOtherInput.value = false
  joinOtherCode.value = ''
  await resetPairing()
  guestAdoptPeer.value = adoptPeer
  const status = await webdavSpaceApi.findOffer(code, deviceName.value, adoptPeer)
  if (expectedVaultId && status.vaultId !== expectedVaultId) {
    await webdavSpaceApi.cancelPairing().catch(() => {})
    throw new Error('配对码对应的空间已变化，请重新输入')
  }
  guestPairing.value = { ...status, shortCode: code }
  if (!status.configured && status.status !== 'expired') {
    guestTimer = setInterval(pollGuest, 2000)
    saveGuestResume()
  }
}

async function requestStartJoin() {
  if (busy.value) return
  busy.value = true
  error.value = ''
  notice.value = ''
  try {
    const state = await webdavSpaceApi.joinState()
    const drafts = localDraftCount()
    if (!state.hasIdentity && !state.hasLocalData && drafts === 0) {
      await beginStartJoin(false)
      return
    }
    joinPrompt.value = { mode: 'join', code: '', preview: null, state, draftCount: drafts }
  } catch (err: any) {
    error.value = err?.message || '检查本机数据失败'
  } finally {
    busy.value = false
  }
}

async function beginStartJoin(adoptPeer = false) {
  showJoinOtherInput.value = false
  joinOtherCode.value = ''
  await resetPairing()
  guestAdoptPeer.value = adoptPeer
  const start = await webdavSpaceApi.startJoin(deviceName.value, adoptPeer)
  guestPairing.value = {
    pairingId: start.pairingId,
    shortCode: start.shortCode,
    status: 'waiting',
    configured: false,
    expiresAt: start.expiresAt,
    verificationCode: start.verificationCode,
    peer: null,
    vaultId: null,
    spaceFingerprint: null,
    message: '等待已有设备输入六位码并批准；批准后本机会自动完成加入。'
  }
  guestTimer = setInterval(pollGuest, 2000)
  saveGuestResume()
}

async function clearAndContinue() {
  const prompt = joinPrompt.value
  if (!prompt || busy.value) return
  busy.value = true
  error.value = ''
  notice.value = ''
  try {
    if (prompt.mode === 'offer' && prompt.preview) {
      const latest = await webdavSpaceApi.previewOffer(prompt.code)
      if (latest.vaultId !== prompt.preview.vaultId) {
        throw new Error('配对码对应的空间已变化，请重新输入')
      }
    }
    joinPrompt.value = null
    showJoinOtherInput.value = false
    joinOtherCode.value = ''
    if (prompt.mode === 'offer') {
      await beginFindOffer(prompt.code, prompt.preview?.vaultId, true)
    } else {
      await beginStartJoin(true)
    }
    notice.value = '等待对方批准；批准成功后才清空本机文稿并加入。'
  } catch (err: any) {
    error.value = err?.message || '开始加入失败'
  } finally {
    busy.value = false
  }
}

function joinUsingLocal() {
  joinPrompt.value = null
  guestAdoptPeer.value = false
  notice.value = '已取消本次加入，本机数据未改变。请在本机点「显示配对码」，再到对方设备输入本机配对码。'
}

function cancelJoinPrompt() {
  const prompt = joinPrompt.value
  joinPrompt.value = null
  guestAdoptPeer.value = false
  notice.value = prompt && !prompt.state.hasIdentity
    ? '已取消加入，本机数据未改变。如果要以本机数据为准，请先建立同步空间，再让对方输入本机配对码。'
    : '已取消加入，本机数据未改变。'
}

async function pollGuest() {
  if (!guestPairing.value || pollingGuest) return
  pollingGuest = true
  try {
    const status = await webdavSpaceApi.joinStatus(guestPairing.value.pairingId)
    guestPollFailures = 0
    if (status.configured) {
      stopGuestTimer()
      clearGuestResume()
      guestPairing.value = null
      if (guestAdoptPeer.value) {
        workspaceStore.clearLocalDataForJoin()
        guestAdoptPeer.value = false
      }
      await loadConfig()
      await syncStore.refreshStatus()
      const synced = await syncStore.triggerSync()
      await workspaceStore.bootstrap()
      if (synced) {
        await workspaceStore.restoreLastView()
        notice.value = '已加入同步空间，远端数据已拉取。'
      } else {
        notice.value = `已加入同步空间，但首次拉取未完成：${syncStore.errorMsg || '请稍后点「立即同步」'}`
      }
      return
    }
    guestPairing.value = { ...guestPairing.value, ...status }
    if (status.status === 'expired') {
      stopGuestTimer()
      clearGuestResume()
    }
  } catch (err: any) {
    // 坚果云等供应商经常 429/5xx：一次失败不代表配对失效，继续轮询。
    guestPollFailures += 1
    const message = err?.message || '查询配对状态失败'
    const expired = !!guestPairing.value?.expiresAt && Date.now() >= (guestPairing.value?.expiresAt || 0)
    if (!isFatalPairingError(message) && guestPollFailures <= MAX_POLL_FAILURES && !expired) {
      if (guestPairing.value) {
        guestPairing.value = {
          ...guestPairing.value,
          message: `网络不稳定，正在自动重试（第 ${guestPollFailures} 次）…`,
        }
      }
      return
    }
    stopGuestTimer()
    clearGuestResume()
    error.value = message
  } finally {
    pollingGuest = false
  }
}

async function startOffer() {
  if (busy.value) return
  busy.value = true
  error.value = ''
  notice.value = ''
  try {
    await resetPairing()
    hostMode = 'offer'
    hostPollFailures = 0
    guestPollFailures = 0
    const start = await webdavSpaceApi.startOffer()
    hostPairing.value = {
      pairingId: start.pairingId,
      shortCode: start.shortCode,
      status: 'waiting',
      configured: false,
      expiresAt: start.expiresAt,
      verificationCode: start.verificationCode,
      peer: null,
      vaultId: null,
      spaceFingerprint: spaceCaption.value,
      message: '等待新设备输入配对码…'
    }
    showHostJoinInput.value = false
    hostTimer = setInterval(pollHost, 2000)
    saveHostResume()
  } catch (err: any) {
    error.value = err?.message || '生成配对码失败'
  } finally {
    busy.value = false
  }
}

async function pollHost() {
  if (!hostPairing.value || hostMode !== 'offer' || pollingHost) return
  pollingHost = true
  try {
    const status = await webdavSpaceApi.offerStatus(hostPairing.value.pairingId)
    hostPollFailures = 0
    hostPairing.value = { ...hostPairing.value, ...status }
    if (status.status === 'claimed' || status.status === 'approved' || status.status === 'expired') {
      stopHostTimer()
      if (status.status === 'approved' || status.status === 'expired') clearHostResume()
      else saveHostResume()
    }
  } catch (err: any) {
    // 与新设备侧一致：网络抖动不能把配对卡死在“等待”上。
    hostPollFailures += 1
    const message = err?.message || '查询配对状态失败'
    const expired = !!hostPairing.value?.expiresAt && Date.now() >= (hostPairing.value?.expiresAt || 0)
    if (!isFatalPairingError(message) && hostPollFailures <= MAX_POLL_FAILURES && !expired) {
      if (hostPairing.value) {
        hostPairing.value = {
          ...hostPairing.value,
          message: `网络不稳定，正在自动重试（第 ${hostPollFailures} 次）…`,
        }
      }
      return
    }
    stopHostTimer()
    clearHostResume()
    error.value = message
  } finally {
    pollingHost = false
  }
}

async function findJoin() {
  if (busy.value) return
  busy.value = true
  error.value = ''
  notice.value = ''
  try {
    await resetPairing()
    hostMode = 'join'
    hostPollFailures = 0
    const status = await webdavSpaceApi.findJoin(hostJoinCode.value)
    hostPairing.value = { ...status, shortCode: hostJoinCode.value }
    showHostJoinInput.value = false
    saveHostResume()
  } catch (err: any) {
    error.value = err?.message || '查找配对码失败'
  } finally {
    busy.value = false
  }
}

async function approveHost() {
  if (!hostPairing.value || busy.value) return
  busy.value = true
  error.value = ''
  try {
    const status = hostMode === 'offer'
      ? await webdavSpaceApi.approveOffer(hostPairing.value.pairingId)
      : await webdavSpaceApi.approveJoin(hostPairing.value.pairingId)
    hostPairing.value = { ...hostPairing.value, ...status }
    stopHostTimer()
    clearHostResume()
    notice.value = '已批准，新设备会自动完成加入。'
    void loadDevices()
  } catch (err: any) {
    error.value = err?.message || '批准新设备失败'
  } finally {
    busy.value = false
  }
}

async function cancelHostJoin() {
  showHostJoinInput.value = false
  hostJoinCode.value = ''
  await cancelPairing()
}

function closeJoinOther() {
  showJoinOtherInput.value = false
  joinOtherCode.value = ''
}

async function cancelPairing() {
  stopHostTimer()
  stopGuestTimer()
  hostPairing.value = null
  guestPairing.value = null
  guestAdoptPeer.value = false
  joinCode.value = ''
  hostJoinCode.value = ''
  clearHostResume()
  clearGuestResume()
  await webdavSpaceApi.cancelPairing().catch(() => {})
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
  if (!window.confirm('退出空间只清除本机身份，本机笔记仍保留。要阻止旧设备继续同步，请修改 WebDAV 应用密码。如果这是唯一设备，退出后旧空间将无法解密。确定退出吗？')) return
  busy.value = true
  error.value = ''
  try {
    await webdavSpaceApi.leaveSpace()
    await cancelPairing()
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
  nowTick.value = Date.now()
  tickTimer = setInterval(() => { nowTick.value = Date.now() }, 1000)
  await loadConfig()
  restorePairing()
  await syncStore.refreshStatus()
})

watch(() => syncStore.lastSyncTime, () => {
  if (config.value?.active) void loadDevices()
})

onBeforeUnmount(() => {
  stopHostTimer()
  stopGuestTimer()
  if (tickTimer) clearInterval(tickTimer)
  tickTimer = null
})
</script>

<style scoped>
.sync-space-section { display: flex; flex-direction: column; gap: 12px; }
.sync-space-section h3 { margin: 0; font-size: 15px; }
.sync-space-section h4 { margin: 0 0 8px; font-size: 13px; }
.sync-help { margin: 0; color: var(--text-secondary, #8a8a8a); font-size: 12px; line-height: 1.6; }
.sync-box { border: 1px solid var(--border-color, #e5e5e5); border-radius: 12px; padding: 14px; display: flex; flex-direction: column; gap: 10px; background: color-mix(in srgb, var(--bg-secondary, #fff) 72%, transparent); }
.sync-choice-grid { display: grid; grid-template-columns: repeat(2, minmax(0, 1fr)); gap: 10px; }
.sync-choice.span-2 { grid-column: 1 / -1; }
.space-hero { display: flex; flex-direction: column; gap: 14px; padding: 16px; border: 1px solid var(--border-color, #e5e5e5); border-radius: 14px; background: linear-gradient(145deg, color-mix(in srgb, var(--accent-color, #2f6fed) 7%, var(--bg-elevated, #fff)), var(--bg-elevated, #fff) 58%); box-shadow: 0 8px 24px color-mix(in srgb, #000 6%, transparent); }
.space-hero-top { display: flex; align-items: flex-start; justify-content: space-between; gap: 14px; }
.space-hero-copy { display: flex; min-width: 0; flex-direction: column; gap: 6px; }
.space-kicker { color: var(--text-secondary, #8a8a8a); font-size: 11px; font-weight: 600; letter-spacing: .08em; text-transform: uppercase; }
.space-fingerprint { color: var(--text-primary); font-family: ui-monospace, SFMono-Regular, Menlo, monospace; font-size: 26px; letter-spacing: .16em; line-height: 1.1; }
.space-id-copy { display: flex; align-items: center; gap: 8px; max-width: 100%; padding: 0; border: 0; background: transparent; color: var(--text-secondary, #8a8a8a); font: inherit; font-size: 11px; cursor: pointer; text-align: left; }
.space-id-copy code { overflow: hidden; color: var(--text-primary); font-family: ui-monospace, SFMono-Regular, Menlo, monospace; font-size: 11px; text-overflow: ellipsis; white-space: nowrap; }
.space-id-copy .copy-label { flex: 0 0 auto; color: var(--accent-color, #2f6fed); }
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
.sync-row { display: flex; gap: 10px; align-items: baseline; font-size: 12px; }
.sync-row > span { color: var(--text-secondary, #8a8a8a); min-width: 72px; }
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
.sync-field select { width: 100%; box-sizing: border-box; padding: 8px 10px; border: 1px solid var(--border-color, #d9d9d9); border-radius: 8px; background: var(--bg-primary, #fff); color: inherit; font: inherit; }
.sync-field input.pairing-input { font-size: 20px; letter-spacing: 8px; text-align: center; }
.pairing-box { border-color: var(--accent-color, #2f6fed); }
.pairing-code { color: var(--text-primary, inherit); font-family: ui-monospace, SFMono-Regular, Menlo, monospace; font-size: 24px; letter-spacing: 6px; font-weight: 600; }
.sync-section-head { display: flex; align-items: center; justify-content: space-between; }
.sync-notice { margin: 0; color: #1d7a46; font-size: 12px; }
.sync-warning { margin: 0; color: #a15c00; font-size: 12px; line-height: 1.6; }
.sync-error { margin: 0; color: #c0392b; font-size: 12px; }
.sync-advanced summary { cursor: pointer; font-size: 12px; font-weight: 600; color: var(--text-secondary, #8a8a8a); }
.sync-advanced[open] summary { margin-bottom: 8px; }
.sync-advanced .sync-field { margin-top: 8px; }
.sync-advanced .sync-actions { margin-top: 8px; }
@media (max-width: 640px) {
  .sync-choice-grid, .space-meta-grid { grid-template-columns: minmax(0, 1fr); }
  .sync-choice.span-2 { grid-column: auto; }
}
.device-row { padding: 7px 0; border-top: 1px solid var(--border-color, #eee); font-size: 12px; }
.device-row:first-of-type { border-top: 0; }
.device-row strong { font-weight: 600; }
.device-current { margin-left: 7px; color: var(--accent-color, #2f6fed); font-size: 11px; }
.device-meta { display: block; margin-top: 3px; color: var(--text-secondary, #8a8a8a); font-size: 11px; }
.join-prompt-overlay { position: fixed; inset: 0; z-index: 10001; display: flex; align-items: center; justify-content: center; padding: 20px; background: rgba(0, 0, 0, .4); }
.join-prompt-dialog { width: min(460px, 100%); max-height: 80vh; overflow: auto; display: flex; flex-direction: column; gap: 12px; padding: 16px; border: 1px solid var(--border-primary, #ddd); border-radius: 12px; background: var(--bg-elevated, #fff); color: var(--text-primary); box-shadow: var(--shadow-dropdown, 0 8px 28px rgba(0,0,0,.18)); }
.join-prompt-dialog h4 { margin: 0; font-size: 14px; }
.join-prompt-message { margin: 0; white-space: pre-line; color: var(--text-primary); font-size: 12px; line-height: 1.7; }
</style>
