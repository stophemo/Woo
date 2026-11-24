<template>
  <div class="sync-space-section">
    <h3>跨设备</h3>
    <p class="sync-help">
      每台设备先配置同一个 WebDAV 账号，之后只用六位配对码加入。正文和图片端到端加密，
      WebDAV 只保存密文。WebDAV 没有服务端推送，同步不是实时的。
      同一个 WebDAV 账号的访问者能看到配对临时文件和六位定位码；批准前请核对设备名、平台、核对码和空间指纹。
    </p>

    <div class="sync-box">
      <h4>WebDAV 账号</h4>
      <label class="sync-field">
        <span>WebDAV 地址</span>
        <input
          v-model="endpoint"
          type="url"
          placeholder="https://dav.example.com/dav/"
          autocomplete="off"
          spellcheck="false"
        />
      </label>
      <label class="sync-field">
        <span>用户名</span>
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
      <p class="sync-help">
        示例：坚果云 <code>https://dav.jianguoyun.com/dav/</code>（使用应用密码）；
        Nextcloud <code>https://cloud.example.com/remote.php/dav/files/用户名/</code>；
        群晖 <code>https://nas.example.com:5006/woo</code>；也可填写任意 HTTPS WebDAV。
        应用密码只加密保存在本机。部分供应商（如坚果云免费版）有请求频率限制；同步失败不影响本地写作。
      </p>
      <div class="sync-actions">
        <button class="sync-btn primary" :disabled="busy" @click="saveAccount">
          {{ busy ? '正在测试…' : '测试连接并保存' }}
        </button>
      </div>
      <p v-if="config?.account.configured" class="sync-help">
        已保存：{{ config.account.username }} @ {{ config.account.endpoint }}
      </p>
    </div>

    <p v-if="config?.legacyNotice" class="sync-notice">{{ config.legacyNotice }}</p>
    <p v-if="config?.notice" class="sync-notice">{{ config.notice }}</p>

    <template v-if="config?.account.active && !config?.spaceConfigured && !guestPairing && !hostPairing">
      <div class="sync-box">
        <h4>建立同步空间</h4>
        <p class="sync-help">第一台设备建立空间后，其他设备用六位配对码加入。请确认本机数据是当前最新版本。</p>
        <label class="sync-field">
          <span>本机名称</span>
          <input v-model="deviceName" type="text" maxlength="80" placeholder="例如 MacBook Air" autocomplete="off" />
        </label>
        <div class="sync-actions">
          <button class="sync-btn primary" :disabled="busy" @click="createSpace">建立同步空间</button>
        </div>
      </div>

      <div class="sync-box">
        <h4>加入已有空间：输入配对码</h4>
        <p class="sync-help">在已有设备上点击「显示配对码」，把六位数字填到这里。已有设备批准后，本机会自动完成加入。</p>
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
          <button class="sync-btn primary" :disabled="busy || joinCode.length !== 6" @click="requestJoinOffer(joinCode)">
            查找并认领
          </button>
        </div>
      </div>

      <div class="sync-box">
        <h4>我是新设备：显示我的配对码</h4>
        <p class="sync-help">生成六位码后，在已有设备上输入并批准；批准后本机会自动完成加入，不需要再点确认。</p>
        <label class="sync-field">
          <span>本机名称</span>
          <input v-model="deviceName" type="text" maxlength="80" placeholder="例如新手机" autocomplete="off" />
        </label>
        <div class="sync-actions">
          <button class="sync-btn" :disabled="busy" @click="requestStartJoin">显示我的配对码</button>
        </div>
      </div>
    </template>

    <template v-if="config?.active">
      <div class="sync-box">
        <div class="sync-row"><span>WebDAV</span><code>{{ config.account.endpoint }}</code></div>
        <div class="sync-row"><span>账号</span><code>{{ config.account.username }}</code></div>
        <div class="sync-row"><span>空间指纹</span><code>{{ config.identity.spaceFingerprint }}</code></div>
        <div class="sync-row"><span>本机</span><code>{{ config.identity.deviceName }} · {{ platformLabel(config.identity.platform) }}</code></div>
        <div class="sync-row">
          <span>状态</span>
          <code>{{ syncStore.isSyncing ? '同步中…' : syncStore.lastSyncLabel }}</code>
        </div>
        <p v-if="syncStore.pendingChanges > 0" class="sync-help">待同步 {{ syncStore.pendingChanges }} 项</p>
        <p class="sync-help">
          同步会在启动、本地修改、回到前台和每 15 分钟兜底时进行，因此另一台设备的修改不会立刻出现。
          设备列表来自加密心跳，只读；WebDAV 没有服务端，不能远程吊销。要阻止旧设备，请修改 WebDAV 应用密码。
        </p>
        <div class="sync-actions">
          <button class="sync-btn primary" :disabled="syncStore.isSyncing" @click="syncNow">
            {{ syncStore.isSyncing ? '同步中…' : '立即同步' }}
          </button>
          <button class="sync-btn" :disabled="busy" @click="startOffer">显示配对码</button>
          <button class="sync-btn" :disabled="busy" @click="showHostJoinInput = !showHostJoinInput">
            输入新设备配对码
          </button>
          <button class="sync-btn" :disabled="busy" @click="showJoinOtherInput = !showJoinOtherInput">
            加入其他空间
          </button>
          <button class="sync-btn danger" :disabled="busy" @click="leave">退出空间</button>
        </div>
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
            </span>
          </div>
        </div>
        <p v-if="devices.length === 0" class="sync-help">完成一次同步后会显示设备心跳。</p>
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
      <p class="sync-help">{{ hostPairing.message || (hostPairing.status === 'waiting' ? '等待新设备输入配对码…' : '') }}</p>
      <p v-if="hostPairing.status === 'claimed'" class="sync-warning">
        请确认新设备屏幕显示相同的六位核对码和空间指纹。如果不是你正在操作的设备，请不要批准。
      </p>
      <div v-if="hostPairing.status === 'claimed'" class="sync-actions">
        <button class="sync-btn primary" :disabled="busy" @click="approveHost">核对一致，批准加入</button>
      </div>
      <p v-if="hostPairing.status === 'approved'" class="sync-notice">已批准，新设备会自动完成加入。</p>
      <p v-if="hostPairing.status === 'expired'" class="sync-error">配对码已过期，请重新生成。</p>
      <p v-if="hostPairing.expiresAt" class="sync-help">
        有效期至 {{ formatTime(hostPairing.expiresAt) }}
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
        有效期至 {{ formatTime(guestPairing.expiresAt) }}
      </p>
    </div>

    <div v-if="joinPrompt" class="join-prompt-overlay" @click.self="cancelJoinPrompt">
      <div class="join-prompt-dialog">
        <h4>{{ joinPrompt.state.hasIdentity ? '选择以哪台设备的数据为准' : '本机文稿将被清空' }}</h4>
        <p class="join-prompt-message">{{ joinPromptMessage }}</p>
        <p class="sync-warning">
          清空只影响本机，不删除 WebDAV 上的旧空间；WebDAV 账号与界面偏好会保留。
          清空发生在对方批准之前；如果随后配对失败，本机数据不会自动恢复。
        </p>
        <div class="sync-actions">
          <button class="sync-btn primary" :disabled="busy" @click="clearAndContinue">
            {{ joinPrompt.state.hasIdentity ? '以对方为准（清空本机后加入）' : '清空本机文稿并加入' }}
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

let hostTimer: ReturnType<typeof setInterval> | null = null
let guestTimer: ReturnType<typeof setInterval> | null = null
let pollingHost = false
let pollingGuest = false
let hostMode: 'offer' | 'join' = 'offer'

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

async function loadConfig() {
  try {
    config.value = await webdavSpaceApi.getConfig()
    endpoint.value = config.value.account.endpoint || endpoint.value
    username.value = config.value.account.username || username.value
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
    notice.value = 'WebDAV 连接成功，账号已加密保存。'
    await loadConfig()
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
      await beginFindOffer(code, preview.vaultId)
      return
    }
    joinPrompt.value = { mode: 'offer', code, preview, state, draftCount: drafts }
  } catch (err: any) {
    error.value = err?.message || '读取对方配对码失败'
  } finally {
    busy.value = false
  }
}

async function beginFindOffer(code: string, expectedVaultId?: string) {
  showJoinOtherInput.value = false
  joinOtherCode.value = ''
  await resetPairing()
  const status = await webdavSpaceApi.findOffer(code, deviceName.value)
  if (expectedVaultId && status.vaultId !== expectedVaultId) {
    await webdavSpaceApi.cancelPairing().catch(() => {})
    throw new Error('配对码对应的空间已变化，请重新输入')
  }
  guestPairing.value = { ...status, shortCode: code }
  if (!status.configured && status.status !== 'expired') {
    guestTimer = setInterval(pollGuest, 2000)
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
      await beginStartJoin()
      return
    }
    joinPrompt.value = { mode: 'join', code: '', preview: null, state, draftCount: drafts }
  } catch (err: any) {
    error.value = err?.message || '检查本机数据失败'
  } finally {
    busy.value = false
  }
}

async function beginStartJoin() {
  showJoinOtherInput.value = false
  joinOtherCode.value = ''
  await resetPairing()
  const start = await webdavSpaceApi.startJoin(deviceName.value)
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
    await webdavSpaceApi.clearLocalForJoin()
    workspaceStore.clearLocalDataForJoin()
    await workspaceStore.bootstrap()
    await workspaceStore.restoreLastView()
    joinPrompt.value = null
    showJoinOtherInput.value = false
    joinOtherCode.value = ''
    await loadConfig()
    await syncStore.refreshStatus()
    if (prompt.mode === 'offer') {
      await beginFindOffer(prompt.code, prompt.preview?.vaultId)
    } else {
      await beginStartJoin()
    }
    notice.value = '已清空本机文稿，正在加入对方空间…'
  } catch (err: any) {
    error.value = err?.message || '清空本机数据失败'
  } finally {
    busy.value = false
  }
}

function joinUsingLocal() {
  joinPrompt.value = null
  notice.value = '已取消本次加入，本机数据未改变。请在本机点「显示配对码」，再到对方设备输入本机配对码。'
}

function cancelJoinPrompt() {
  const prompt = joinPrompt.value
  joinPrompt.value = null
  notice.value = prompt && !prompt.state.hasIdentity
    ? '已取消加入，本机数据未改变。如果要以本机数据为准，请先建立同步空间，再让对方输入本机配对码。'
    : '已取消加入，本机数据未改变。'
}

async function pollGuest() {
  if (!guestPairing.value || pollingGuest) return
  pollingGuest = true
  try {
    const status = await webdavSpaceApi.joinStatus(guestPairing.value.pairingId)
    if (status.configured) {
      stopGuestTimer()
      guestPairing.value = null
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
    }
  } catch (err: any) {
    stopGuestTimer()
    error.value = err?.message || '查询配对状态失败'
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
    hostPairing.value = { ...hostPairing.value, ...status }
    if (status.status === 'claimed' || status.status === 'approved' || status.status === 'expired') {
      stopHostTimer()
    }
  } catch (err: any) {
    stopHostTimer()
    error.value = err?.message || '查询配对状态失败'
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
    const status = await webdavSpaceApi.findJoin(hostJoinCode.value)
    hostPairing.value = { ...status, shortCode: hostJoinCode.value }
    showHostJoinInput.value = false
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
  joinCode.value = ''
  hostJoinCode.value = ''
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
  await loadConfig()
  await syncStore.refreshStatus()
})

watch(() => syncStore.lastSyncTime, () => {
  if (config.value?.active) void loadDevices()
})

onBeforeUnmount(() => {
  stopHostTimer()
  stopGuestTimer()
})
</script>

<style scoped>
.sync-space-section { display: flex; flex-direction: column; gap: 12px; }
.sync-space-section h3 { margin: 0; font-size: 15px; }
.sync-space-section h4 { margin: 0 0 8px; font-size: 13px; }
.sync-help { margin: 0; color: var(--text-secondary, #8a8a8a); font-size: 12px; line-height: 1.6; }
.sync-box { border: 1px solid var(--border-color, #e5e5e5); border-radius: 10px; padding: 12px; display: flex; flex-direction: column; gap: 10px; }
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
.sync-field input.pairing-input { font-size: 20px; letter-spacing: 8px; text-align: center; }
.pairing-box { border-color: var(--accent-color, #2f6fed); }
.pairing-code { color: var(--text-primary, inherit); font-family: ui-monospace, SFMono-Regular, Menlo, monospace; font-size: 24px; letter-spacing: 6px; font-weight: 600; }
.sync-section-head { display: flex; align-items: center; justify-content: space-between; }
.sync-notice { margin: 0; color: #1d7a46; font-size: 12px; }
.sync-warning { margin: 0; color: #a15c00; font-size: 12px; line-height: 1.6; }
.sync-error { margin: 0; color: #c0392b; font-size: 12px; }
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
