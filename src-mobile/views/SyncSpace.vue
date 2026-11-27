<script setup lang="ts">
import { computed, nextTick, onBeforeUnmount, onMounted, ref, watch } from 'vue'
import { useRouter } from 'vue-router'
import { showConfirmDialog, showToast } from 'vant'
import { useSyncStore } from '../../src/stores/sync'
import { useWorkspaceStore } from '../../src/stores/workspace'
import webdavSpaceApi, {
  type JoinLocalStateView,
  type OfferPreviewView,
  type PairingStatusView,
  type WebDavDeviceView,
  type WebdavSpaceConfigView,
} from '../../src/services/webdavSpaceApi'
import {
  WEBDAV_PROVIDERS,
  findProvider,
  matchProvider,
} from '../../src/config/webdavProviders'

interface PendingJoin {
  mode: 'offer' | 'join'
  code: string
  preview: OfferPreviewView | null
  state: JoinLocalStateView
  draftCount: number
}

// 用户先点六位配对码入口时，如果本机还没连接 WebDAV，先把动作挂起，
// 保存账号后自动继续，避免把账号表单当成“新建空间”的第一步。
type PendingAfterAccount =
  | { type: 'joinOffer'; code: string }
  | { type: 'startJoin' }
  | { type: 'create' }

const router = useRouter()
const syncStore = useSyncStore()
const workspaceStore = useWorkspaceStore()

const config = ref<WebdavSpaceConfigView | null>(null)
const providerId = ref('jianguoyun')
const showProviderSheet = ref(false)
const currentProvider = computed(() => findProvider(providerId.value))
const providerActions = WEBDAV_PROVIDERS.map(provider => ({ name: provider.label, id: provider.id }))
const endpoint = ref('')
const username = ref('')
const password = ref('')
const deviceName = ref('')
const joinCode = ref('')
const joinOtherCode = ref('')
const hostJoinCode = ref('')
const showHostJoinInput = ref(false)
const showJoinOtherInput = ref(false)
const busy = ref(false)
const devices = ref<WebDavDeviceView[]>([])
const deviceError = ref('')
const hostPairing = ref<(PairingStatusView & { shortCode?: string }) | null>(null)
const guestPairing = ref<(PairingStatusView & { shortCode?: string }) | null>(null)
const joinPrompt = ref<PendingJoin | null>(null)
const joinPromptVisible = ref(false)
const guestAdoptPeer = ref(false)
const accountExpanded = ref(false)
const pendingAfterAccount = ref<PendingAfterAccount | null>(null)
const accountReady = computed(() => !!config.value?.account.active)
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

/** 配对中途切页/切后台时，前端状态会丢；把最小状态存本地，回来接着轮询。 */
function saveGuestResume() {
  if (!guestPairing.value) return
  try {
    const payload: GuestResume = { ...guestPairing.value, shortCode: guestPairing.value.shortCode || '', adoptPeer: guestAdoptPeer.value }
    localStorage.setItem(GUEST_RESUME_KEY, JSON.stringify(payload))
  } catch { /* ignore */ }
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

const joinPromptMessage = computed(() => {
  const prompt = joinPrompt.value
  if (!prompt) return ''
  const parts: string[] = []
  if (prompt.state.hasIdentity) {
    const vault = prompt.state.vaultId
      ? `${prompt.state.vaultId.slice(0, 8)}…${prompt.state.vaultId.slice(-8)}`
      : '未知'
    const lastSync = prompt.state.lastSyncTime
      ? new Date(prompt.state.lastSyncTime).toLocaleString('zh-CN')
      : '尚未成功同步'
    parts.push(`本机已有同步空间 ${vault}（最近同步：${lastSync}）。`)
  }
  if (prompt.state.hasLocalData) {
    parts.push(
      `本机有 ${prompt.state.documentCount} 篇文稿、${prompt.state.folderCount} 个目录、` +
      `${prompt.state.versionCount} 条版本记录（其中回收站 ${prompt.state.trashCount} 篇）。`
    )
  }
  if (prompt.draftCount > 0) parts.push(`另有 ${prompt.draftCount} 篇本地草稿。`)
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

/** 这些错误重试也没用，应当立即停下并让用户重新配对。 */
function isFatalPairingError(message: string) {
  return /配对会话不存在|必须选择以对方为准|已拒绝|配对码已过期/.test(message)
}

/** 配对码剩余时间；过期后返回“已过期”。 */
function remainingLabel(expiresAt?: number | null) {
  if (!expiresAt) return ''
  const left = expiresAt - nowTick.value
  if (left <= 0) return '已过期'
  const total = Math.floor(left / 1000)
  return `${Math.floor(total / 60)}:${String(total % 60).padStart(2, '0')} 后过期`
}

async function copySpaceId() {
  const id = config.value?.identity.vaultId
  if (!id) return
  try {
    await navigator.clipboard.writeText(id)
    showToast('空间 ID 已复制')
  } catch {
    showToast('复制失败，请手动选择空间 ID')
  }
}

async function loadConfig() {
  config.value = await webdavSpaceApi.getConfig()
  endpoint.value = config.value.account.endpoint || endpoint.value
  username.value = config.value.account.username || username.value
  providerId.value = matchProvider(endpoint.value).id
  if (!endpoint.value) endpoint.value = currentProvider.value.endpoint
  if (!deviceName.value) deviceName.value = config.value.identity.deviceName || '手机'
  if (!config.value.account.active) accountExpanded.value = true
  if (config.value.active) await loadDevices()
  else devices.value = []
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

async function loadDevices() {
  if (!config.value?.active) {
    devices.value = []
    return
  }
  deviceError.value = ''
  try {
    devices.value = await webdavSpaceApi.listDevices()
  } catch (error: any) {
    deviceError.value = error?.message || '读取设备列表失败'
  }
}

async function saveAccount() {
  if (busy.value) return
  busy.value = true
  let saved = false
  try {
    await webdavSpaceApi.connectAccount(endpoint.value, username.value, password.value || undefined)
    password.value = ''
    saved = true
  } catch (error: any) {
    showToast(error?.message || 'WebDAV 连接失败')
  } finally {
    busy.value = false
  }
  if (!saved) return

  const pending = pendingAfterAccount.value
  pendingAfterAccount.value = null
  accountExpanded.value = false
  try {
    await loadConfig()
  } catch (error: any) {
    showToast(error?.message || '读取 WebDAV 配置失败')
    return
  }

  if (config.value?.active) {
    // 已经自动建立或加入空间，不再需要配对码。
    await syncStore.refreshStatus()
    void syncStore.triggerSync()
    showToast(config.value?.notice || '已连接，正在开始同步…')
    return
  }

  showToast(config.value?.notice || (pending ? 'WebDAV 已连接，继续配对' : 'WebDAV 连接成功，账号已保存'))
  if (!pending) return
  if (pending.type === 'joinOffer') await requestJoinOffer(pending.code)
  else if (pending.type === 'startJoin') await requestStartJoin()
  else await createSpace()
}

function promptForAccount(next: PendingAfterAccount) {
  pendingAfterAccount.value = next
  accountExpanded.value = true
  showToast('首次连接需要填写同一个 WebDAV 账号，只需一次')
  void nextTick(() => {
    document.getElementById('webdav-account')?.scrollIntoView({ behavior: 'smooth', block: 'center' })
  })
}

async function createSpace() {
  if (busy.value) return
  if (!accountReady.value) {
    promptForAccount({ type: 'create' })
    return
  }
  busy.value = true
  try {
    await webdavSpaceApi.createSpace(deviceName.value)
    showToast('同步空间已建立')
    await loadConfig()
    await syncStore.refreshStatus()
    void syncStore.triggerSync()
  } catch (error: any) {
    showToast(error?.message || '建立失败')
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
    showToast('请输入六位配对码')
    return
  }
  if (!accountReady.value) {
    promptForAccount({ type: 'joinOffer', code })
    return
  }
  busy.value = true
  try {
    const preview = await webdavSpaceApi.previewOffer(code)
    const state = await webdavSpaceApi.joinState()
    const drafts = localDraftCount()
    if (!state.hasIdentity && !state.hasLocalData && drafts === 0) {
      await beginFindOffer(code, preview.vaultId)
      return
    }
    joinPrompt.value = { mode: 'offer', code, preview, state, draftCount: drafts }
    joinPromptVisible.value = true
  } catch (error: any) {
    showToast(error?.message || '读取对方配对码失败')
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
  if (!accountReady.value) {
    promptForAccount({ type: 'startJoin' })
    return
  }
  busy.value = true
  try {
    const state = await webdavSpaceApi.joinState()
    const drafts = localDraftCount()
    if (!state.hasIdentity && !state.hasLocalData && drafts === 0) {
      await beginStartJoin(false)
      return
    }
    joinPrompt.value = { mode: 'join', code: '', preview: null, state, draftCount: drafts }
    joinPromptVisible.value = true
  } catch (error: any) {
    showToast(error?.message || '检查本机数据失败')
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
  try {
    if (prompt.mode === 'offer' && prompt.preview) {
      const latest = await webdavSpaceApi.previewOffer(prompt.code)
      if (latest.vaultId !== prompt.preview.vaultId) {
        throw new Error('配对码对应的空间已变化，请重新输入')
      }
    }
    joinPrompt.value = null
    joinPromptVisible.value = false
    showJoinOtherInput.value = false
    joinOtherCode.value = ''
    if (prompt.mode === 'offer') await beginFindOffer(prompt.code, prompt.preview?.vaultId, true)
    else await beginStartJoin(true)
    showToast('等待对方批准；批准成功后才清空本机文稿')
  } catch (error: any) {
    showToast(error?.message || '开始加入失败')
  } finally {
    busy.value = false
  }
}

function joinUsingLocal() {
  joinPrompt.value = null
  joinPromptVisible.value = false
  guestAdoptPeer.value = false
  showToast('已取消加入。请在本机点「显示配对码」，再到对方设备输入本机配对码')
}

function cancelJoinPrompt() {
  const prompt = joinPrompt.value
  joinPrompt.value = null
  joinPromptVisible.value = false
  guestAdoptPeer.value = false
  showToast(
    prompt && !prompt.state.hasIdentity
      ? '已取消。要以本机数据为准，请先建立同步空间'
      : '已取消加入，本机数据未改变'
  )
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
      showToast(synced ? '已加入同步空间' : '已加入，首次拉取稍后重试')
      await router.replace('/')
      return
    }
    guestPairing.value = { ...guestPairing.value, ...status }
    if (status.status === 'expired') {
      stopGuestTimer()
      clearGuestResume()
    }
  } catch (error: any) {
    // 坚果云等供应商经常 429/5xx：一次失败不代表配对失效，继续轮询。
    guestPollFailures += 1
    const message = error?.message || '查询配对状态失败'
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
    showToast(message)
  } finally {
    pollingGuest = false
  }
}

async function startOffer() {
  if (busy.value) return
  busy.value = true
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
      spaceFingerprint: config.value?.identity.spaceFingerprint || null,
      message: '等待新设备输入配对码…'
    }
    showHostJoinInput.value = false
    hostTimer = setInterval(pollHost, 2000)
    saveHostResume()
  } catch (error: any) {
    showToast(error?.message || '生成配对码失败')
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
  } catch (error: any) {
    // 与新设备侧一致：网络抖动不能把配对卡死在“等待”上。
    hostPollFailures += 1
    const message = error?.message || '查询配对状态失败'
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
    showToast(message)
  } finally {
    pollingHost = false
  }
}

async function findJoin() {
  if (busy.value) return
  busy.value = true
  try {
    await resetPairing()
    hostMode = 'join'
    const status = await webdavSpaceApi.findJoin(hostJoinCode.value)
    hostPairing.value = { ...status, shortCode: hostJoinCode.value }
    showHostJoinInput.value = false
    saveHostResume()
  } catch (error: any) {
    showToast(error?.message || '查找配对码失败')
  } finally {
    busy.value = false
  }
}

async function approveHost() {
  if (!hostPairing.value || busy.value) return
  busy.value = true
  try {
    const status = hostMode === 'offer'
      ? await webdavSpaceApi.approveOffer(hostPairing.value.pairingId)
      : await webdavSpaceApi.approveJoin(hostPairing.value.pairingId)
    hostPairing.value = { ...hostPairing.value, ...status }
    stopHostTimer()
    clearHostResume()
    showToast('已批准，新设备会自动完成加入')
    void loadDevices()
  } catch (error: any) {
    showToast(error?.message || '批准失败')
  } finally {
    busy.value = false
  }
}

async function cancelPairing() {
  stopHostTimer()
  stopGuestTimer()
  hostPairing.value = null
  guestPairing.value = null
  guestAdoptPeer.value = false
  joinCode.value = ''
  hostJoinCode.value = ''
  showHostJoinInput.value = false
  clearHostResume()
  clearGuestResume()
  await webdavSpaceApi.cancelPairing().catch(() => {})
}

function closeJoinOther() {
  showJoinOtherInput.value = false
  joinOtherCode.value = ''
}

async function syncNow() {
  const ok = await syncStore.triggerSync()
  showToast(ok ? '同步完成' : (syncStore.errorMsg || '同步失败'))
  await loadDevices()
}

async function leave() {
  try {
    await showConfirmDialog({
      title: '退出同步空间',
      message: '只清除本机身份，本机笔记仍保留。要阻止旧设备继续同步，请修改 WebDAV 应用密码。如果这是唯一设备，退出后旧空间将无法解密。',
    })
  } catch {
    return
  }
  busy.value = true
  try {
    await webdavSpaceApi.leaveSpace()
    await cancelPairing()
    devices.value = []
    await loadConfig()
    await syncStore.refreshStatus()
    showToast('已退出同步空间')
  } catch (error: any) {
    showToast(error?.message || '退出失败')
  } finally {
    busy.value = false
  }
}

onMounted(async () => {
  nowTick.value = Date.now()
  tickTimer = setInterval(() => { nowTick.value = Date.now() }, 1000)
  try {
    await loadConfig()
    restorePairing()
    await syncStore.refreshStatus()
  } catch (error: any) {
    showToast(error?.message || '读取跨设备配置失败')
  }
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

<template>
  <div class="sync-page">
    <van-nav-bar title="跨设备" left-arrow fixed placeholder @click-left="router.back()" />

    <p class="hint">
      输入对方显示的六位配对码即可加入已有空间，或显示本机六位码让对方输入。
      每台设备第一次连接时还需要填写同一个 WebDAV 账号（只需一次，不会新建空间；之后只用六位码）。
      WebDAV 没有服务端推送，同步不是实时的。
    </p>
    <p class="hint">
      同一个 WebDAV 账号的访问者能看到配对临时文件和六位定位码；批准前请核对设备名、平台、核对码和空间指纹。
    </p>

    <p v-if="config?.legacyNotice" class="hint">{{ config.legacyNotice }}</p>
    <p v-if="config?.notice" class="hint">{{ config.notice }}</p>

    <template v-if="accountReady && !config?.active && !guestPairing && !hostPairing">
      <van-cell-group inset title="加入已有空间">
        <van-field
          v-model="joinCode"
          label="配对码"
          placeholder="输入 Mac 显示的六位数字"
          maxlength="6"
          type="digit"
          clearable
        />
        <van-cell title="输入并加入" is-link @click="requestJoinOffer(joinCode)" />
        <div class="hint-block">
          Mac 已经建好空间时，直接输入它的六位码。第一次连接会请你补一次 WebDAV 账号，之后只用六位码；
          已有设备批准后本机会自动完成加入。
        </div>
      </van-cell-group>

      <van-cell-group inset title="或者：让 Mac 输入本机配对码">
        <van-cell title="显示我的配对码" is-link label="在 Mac 上输入这六位码并批准" @click="requestStartJoin" />
      </van-cell-group>

      <details class="advanced-group">
        <summary>两台设备都没有同步空间：建立新空间</summary>
        <van-cell-group inset title="建立新空间（仅第一台设备）">
          <van-field v-model="deviceName" label="本机名称" maxlength="80" clearable />
          <van-cell title="建立同步空间" is-link @click="createSpace" />
          <div class="hint-block">
            如果已经在 Mac 上建过空间，不要点这里；直接在上面输入 Mac 的六位配对码。
          </div>
        </van-cell-group>
      </details>
    </template>

    <template v-if="config?.active">
      <section class="space-hero-mobile">
        <div class="space-hero-mobile-top">
          <div>
            <span class="space-kicker">WebDAV 同步空间</span>
            <strong class="space-fingerprint">{{ config.identity.spaceFingerprint }}</strong>
          </div>
          <span class="space-status" :class="{ syncing: syncStore.isSyncing }">
            <i class="status-dot"></i>
            {{ syncStore.isSyncing ? '同步中' : syncStore.lastSyncLabel }}
          </span>
        </div>
        <button class="space-id-mobile" type="button" @click="copySpaceId">
          <span>空间 ID</span>
          <code>{{ config.identity.vaultId }}</code>
          <span class="copy-label">复制</span>
        </button>
        <div class="space-meta-mobile">
          <div><span>WebDAV</span><b>{{ config.account.endpoint }}</b></div>
          <div><span>账号</span><b>{{ config.account.username }}</b></div>
          <div><span>本机</span><b>{{ config.identity.deviceName }} · {{ platformLabel(config.identity.platform) }}</b></div>
          <div><span>待同步</span><b>{{ syncStore.pendingChanges || 0 }} 项</b></div>
        </div>
      </section>

      <div class="hint">
        启动、本地修改、回到前台和每 15 分钟兜底时同步。设备列表来自加密心跳，只读，不能远程吊销；
        要阻止旧设备，请修改 WebDAV 应用密码。
      </div>

      <div class="actions">
        <van-button block type="primary" :loading="syncStore.isSyncing" @click="syncNow">立即同步</van-button>
        <van-button block plain :disabled="busy" @click="startOffer">显示配对码</van-button>
        <van-button block plain :disabled="busy" @click="showHostJoinInput = !showHostJoinInput">
          输入新设备配对码
        </van-button>
        <van-button block plain :disabled="busy" @click="showJoinOtherInput = !showJoinOtherInput">
          加入其他空间
        </van-button>
        <van-button block plain type="danger" :disabled="busy" @click="leave">退出空间</van-button>
      </div>

      <van-cell-group v-if="showHostJoinInput" inset title="输入新设备配对码">
        <van-field
          v-model="hostJoinCode"
          label="配对码"
          placeholder="六位数字"
          maxlength="6"
          type="digit"
          clearable
        />
        <van-cell title="查找并核对" is-link @click="hostJoinCode.length === 6 && findJoin()" />
        <van-cell title="取消" is-link @click="cancelPairing" />
      </van-cell-group>

      <van-cell-group v-if="showJoinOtherInput" inset title="加入其他空间">
        <van-field
          v-model="joinOtherCode"
          label="配对码"
          placeholder="对方设备显示的六位码"
          maxlength="6"
          type="digit"
          clearable
        />
        <van-cell title="查找并加入" is-link @click="requestJoinOffer(joinOtherCode)" />
        <van-cell title="让已有设备输入本机配对码" is-link @click="requestStartJoin" />
        <van-cell title="取消" is-link @click="closeJoinOther" />
      </van-cell-group>

      <van-cell-group inset title="已加入设备">
        <van-cell
          v-for="device in devices"
          :key="device.deviceId"
          :title="device.name"
          :label="`${platformLabel(device.platform)} · ${formatLastSeen(device.lastSeenAt)} · v${device.appVersion}${isStale(device.lastSeenAt) ? ' · 很久未同步' : ''}`"
        >
          <template #value>
            <van-tag v-if="device.isCurrent" type="primary">当前设备</van-tag>
          </template>
        </van-cell>
        <van-cell
          v-if="config?.active && !devices.some(device => device.isCurrent)"
          :title="config.identity.deviceName"
          :label="`${platformLabel(config.identity.platform)} · 尚未同步`"
        >
          <template #value><van-tag type="primary">当前设备</van-tag></template>
        </van-cell>
        <van-cell
          v-if="config?.active && devices.filter(device => !device.isCurrent).length === 0"
          title="只有当前设备"
        />
        <van-cell v-if="deviceError" :title="deviceError" />
        <van-cell title="刷新" is-link @click="loadDevices" />
      </van-cell-group>
      <div class="hint">
        只读列表，不能移除设备；旧设备仍持有 vaultKey，只能通过修改 WebDAV 应用密码阻止它继续访问。
      </div>
    </template>

    <van-cell-group inset title="WebDAV 账号">
      <van-cell
        :title="accountReady ? '已连接' : '首次连接需要填写'"
        :label="accountReady
          ? `${config?.account.username} · ${config?.account.endpoint}`
          : '填写一次，保存后自动建立或加入同步空间'"
        is-link
        @click="accountExpanded = !accountExpanded"
      />
    </van-cell-group>

    <div v-if="accountExpanded" id="webdav-account">
      <van-cell-group inset title="WebDAV 账号设置">
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
        <div class="hint-block">{{ currentProvider.passwordHint }}</div>
      </van-cell-group>
    </div>

    <van-action-sheet
      v-model:show="showProviderSheet"
      :actions="providerActions"
      cancel-text="取消"
      @select="onProviderSelect"
    />

    <van-cell-group v-if="hostPairing" inset title="已有设备：批准新设备">
      <van-cell v-if="hostPairing.shortCode" title="六位配对码">
        <template #value><span class="pairing-code">{{ hostPairing.shortCode }}</span></template>
      </van-cell>
      <van-cell v-if="hostPairing.peer" title="新设备" :label="platformLabel(hostPairing.peer.platform)" :value="hostPairing.peer.name" />
      <van-cell v-if="hostPairing.verificationCode" title="核对码">
        <template #value><span class="pairing-code">{{ hostPairing.verificationCode }}</span></template>
      </van-cell>
      <van-cell v-if="hostPairing.spaceFingerprint" title="空间指纹" :value="hostPairing.spaceFingerprint" />
      <van-cell :title="hostPairing.message || '等待新设备…'" />
      <van-cell
        v-if="hostPairing.expiresAt"
        title="剩余时间"
        :value="remainingLabel(hostPairing.expiresAt)"
      />
      <div v-if="hostPairing.status === 'claimed'" class="hint">
        请确认新设备屏幕显示相同的六位码和空间指纹。如果不是你正在操作的设备，请不要批准。
      </div>
      <van-cell
        v-if="hostPairing.status === 'claimed'"
        title="核对一致，批准加入"
        is-link
        @click="approveHost"
      />
      <van-cell v-if="hostPairing.status === 'approved'" title="已批准，新设备会自动完成加入" />
      <van-cell v-if="hostPairing.status === 'expired'" title="配对码已过期，请重新生成" />
      <van-cell title="取消配对" is-link @click="cancelPairing" />
    </van-cell-group>

    <van-cell-group v-if="guestPairing" inset title="新设备：等待加入">
      <van-cell v-if="guestPairing.shortCode" title="六位配对码">
        <template #value><span class="pairing-code">{{ guestPairing.shortCode }}</span></template>
      </van-cell>
      <van-cell v-if="guestPairing.peer" title="已有设备" :label="platformLabel(guestPairing.peer.platform)" :value="guestPairing.peer.name" />
      <van-cell v-if="guestPairing.verificationCode" title="核对码">
        <template #value><span class="pairing-code">{{ guestPairing.verificationCode }}</span></template>
      </van-cell>
      <van-cell v-if="guestPairing.spaceFingerprint" title="空间指纹" :value="guestPairing.spaceFingerprint" />
      <van-cell :title="guestPairing.message || '等待已有设备批准；批准后本机会自动完成加入。'" />
      <van-cell
        v-if="guestPairing.expiresAt"
        title="剩余时间"
        :value="remainingLabel(guestPairing.expiresAt)"
      />
      <div v-if="guestPairing.status === 'ready'" class="hint">已有设备已批准，正在自动完成加入…</div>
      <div v-else class="hint">人工确认只在已有设备上完成；本机不需要再点确认。</div>
      <van-cell v-if="guestPairing.status === 'expired'" title="配对码已过期，请重新生成" />
      <van-cell title="取消配对" is-link @click="cancelPairing" />
    </van-cell-group>

    <van-popup
      v-model:show="joinPromptVisible"
      position="bottom"
      round
      :style="{ maxHeight: '82vh' }"
      @closed="joinPrompt = null"
    >
      <div v-if="joinPrompt" class="join-prompt">
        <h3>{{ joinPrompt.state.hasIdentity ? '选择以哪台设备的数据为准' : '加入后以对方数据为准' }}</h3>
        <p class="join-prompt-message">{{ joinPromptMessage }}</p>
        <p class="hint">
          选择以对方为准后只会开始配对；只有对方批准成功后才会清空本机文稿。
          取消、过期或未批准都不会清空。清空只影响本机，不删除 WebDAV 上的旧空间。
        </p>
        <van-button block type="primary" :disabled="busy" @click="clearAndContinue">
          以对方为准（批准后清空并加入）
        </van-button>
        <van-button
          v-if="joinPrompt.state.hasIdentity"
          block
          plain
          :disabled="busy"
          @click="joinUsingLocal"
        >以本机为准（请在对方设备输入本机配对码）</van-button>
        <van-button block plain :disabled="busy" @click="cancelJoinPrompt">取消</van-button>
      </div>
    </van-popup>
  </div>
</template>

<style scoped>
.sync-page { padding: 12px 0 32px; display: flex; flex-direction: column; gap: 12px; }
.advanced-group { margin: 4px 16px 0; }
.advanced-group summary { padding: 6px 2px; color: var(--van-text-color-2); font-size: 12px; cursor: pointer; }
.advanced-group[open] summary { margin-bottom: 6px; }
.space-hero-mobile { display: flex; flex-direction: column; gap: 12px; margin: 0 16px; padding: 16px; border: 1px solid color-mix(in srgb, var(--van-primary-color) 14%, var(--van-border-color)); border-radius: 16px; background: linear-gradient(150deg, color-mix(in srgb, var(--van-primary-color) 8%, var(--van-background-2)), var(--van-background-2) 62%); }
.space-hero-mobile-top { display: flex; align-items: flex-start; justify-content: space-between; gap: 12px; }
.space-kicker { display: block; margin-bottom: 5px; color: var(--van-text-color-2); font-size: 10px; font-weight: 600; letter-spacing: .1em; }
.space-fingerprint { color: var(--van-text-color); font-family: ui-monospace, SFMono-Regular, Menlo, monospace; font-size: 25px; letter-spacing: .14em; }
.space-status { display: inline-flex; flex: 0 0 auto; align-items: center; gap: 5px; padding: 4px 8px; border: 1px solid color-mix(in srgb, var(--van-primary-color) 24%, transparent); border-radius: 999px; background: color-mix(in srgb, var(--van-primary-color) 8%, transparent); color: var(--van-text-color-2); font-size: 10px; }
.space-status .status-dot { width: 6px; height: 6px; border-radius: 50%; background: var(--van-primary-color); }
.space-status.syncing .status-dot { animation: pulse-dot 1.2s ease-in-out infinite; }
.space-id-mobile { display: flex; align-items: center; gap: 7px; width: 100%; padding: 0; border: 0; background: transparent; color: var(--van-text-color-2); font: inherit; font-size: 11px; text-align: left; }
.space-id-mobile code { overflow: hidden; color: var(--van-text-color); font-family: ui-monospace, SFMono-Regular, Menlo, monospace; text-overflow: ellipsis; white-space: nowrap; }
.space-id-mobile .copy-label { flex: 0 0 auto; color: var(--van-primary-color); }
.space-meta-mobile { display: grid; grid-template-columns: repeat(2, minmax(0, 1fr)); gap: 8px 14px; }
.space-meta-mobile > div { display: flex; min-width: 0; flex-direction: column; gap: 2px; }
.space-meta-mobile span { color: var(--van-text-color-2); font-size: 10px; }
.space-meta-mobile b { overflow: hidden; color: var(--van-text-color); font-size: 11px; font-weight: 500; text-overflow: ellipsis; white-space: nowrap; }
@keyframes pulse-dot { 0%, 100% { opacity: 1; } 50% { opacity: .3; } }
.pairing-code { display: inline-block; padding: 2px 8px; border-radius: 6px; background: var(--van-background-2); color: var(--van-text-color); font-family: ui-monospace, SFMono-Regular, Menlo, monospace; font-size: 22px; letter-spacing: 5px; font-weight: 700; }
.actions { display: flex; flex-direction: column; gap: 10px; padding: 16px; }
.hint { margin: 0 20px; color: var(--van-text-color-2); font-size: 12px; line-height: 1.7; }
.hint-block { padding: 8px 16px 12px; color: var(--van-text-color-2); font-size: 12px; line-height: 1.7; overflow-wrap: anywhere; }
.join-prompt { display: flex; flex-direction: column; gap: 12px; padding: 20px 16px calc(20px + env(safe-area-inset-bottom)); }
.join-prompt h3 { margin: 0; color: var(--van-text-color); font-size: 16px; }
.join-prompt-message { margin: 0; white-space: pre-line; color: var(--van-text-color); font-size: 13px; line-height: 1.7; }
</style>
