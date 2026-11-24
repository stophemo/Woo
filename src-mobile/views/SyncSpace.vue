<script setup lang="ts">
import { computed, onBeforeUnmount, onMounted, ref, watch } from 'vue'
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

interface PendingJoin {
  mode: 'offer' | 'join'
  code: string
  preview: OfferPreviewView | null
  state: JoinLocalStateView
  draftCount: number
}

const router = useRouter()
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
const busy = ref(false)
const devices = ref<WebDavDeviceView[]>([])
const deviceError = ref('')
const hostPairing = ref<(PairingStatusView & { shortCode?: string }) | null>(null)
const guestPairing = ref<(PairingStatusView & { shortCode?: string }) | null>(null)
const joinPrompt = ref<PendingJoin | null>(null)
const joinPromptVisible = ref(false)
const guestAdoptPeer = ref(false)
let hostTimer: ReturnType<typeof setInterval> | null = null
let guestTimer: ReturnType<typeof setInterval> | null = null
let pollingHost = false
let pollingGuest = false
let hostMode: 'offer' | 'join' = 'offer'

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
  if (!deviceName.value) deviceName.value = config.value.identity.deviceName || '手机'
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
  } catch (error: any) {
    deviceError.value = error?.message || '读取设备列表失败'
  }
}

async function saveAccount() {
  if (busy.value) return
  busy.value = true
  try {
    await webdavSpaceApi.connectAccount(endpoint.value, username.value, password.value || undefined)
    password.value = ''
    showToast('WebDAV 连接成功，账号已保存')
    await loadConfig()
  } catch (error: any) {
    showToast(error?.message || 'WebDAV 连接失败')
  } finally {
    busy.value = false
  }
}

async function createSpace() {
  if (busy.value) return
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
  }
}

async function requestStartJoin() {
  if (busy.value) return
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
    if (status.configured) {
      stopGuestTimer()
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
    if (status.status === 'expired') stopGuestTimer()
  } catch (error: any) {
    stopGuestTimer()
    showToast(error?.message || '查询配对状态失败')
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
    hostPairing.value = { ...hostPairing.value, ...status }
    if (status.status === 'claimed' || status.status === 'approved' || status.status === 'expired') {
      stopHostTimer()
    }
  } catch (error: any) {
    stopHostTimer()
    showToast(error?.message || '查询配对状态失败')
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
  try {
    await loadConfig()
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
})
</script>

<template>
  <div class="sync-page">
    <van-nav-bar title="跨设备" left-arrow fixed placeholder @click-left="router.back()" />

    <p class="hint">
      每台设备先配置同一个 WebDAV 账号，之后只用六位配对码加入。正文和图片端到端加密。
      WebDAV 没有服务端推送，同步不是实时的。
      同一个 WebDAV 账号的访问者能看到配对临时文件和六位定位码；批准前请核对设备名、平台、核对码和空间指纹。
    </p>

    <van-cell-group inset title="WebDAV 账号">
      <van-field v-model="endpoint" label="地址" placeholder="https://dav.example.com/dav/" clearable />
      <van-field v-model="username" label="用户名" placeholder="WebDAV 用户名" clearable />
      <van-field
        v-model="password"
        label="应用密码"
        type="password"
        :placeholder="config?.account.hasPassword ? '已保存，留空不改' : 'WebDAV 应用密码'"
        clearable
      />
      <van-cell title="测试连接并保存" is-link :label="busy ? '正在测试…' : ''" @click="saveAccount" />
      <div class="hint-block">
        坚果云 https://dav.jianguoyun.com/dav/（使用应用密码）；Nextcloud
        https://cloud.example.com/remote.php/dav/files/用户名/；群晖
        https://nas.example.com:5006/woo。应用密码只加密保存在本机。部分供应商有请求频率限制，同步失败不影响本地写作。
      </div>
    </van-cell-group>

    <p v-if="config?.legacyNotice" class="hint">{{ config.legacyNotice }}</p>
    <p v-if="config?.notice" class="hint">{{ config.notice }}</p>

    <template v-if="config?.account.active && !config?.spaceConfigured && !guestPairing && !hostPairing">
      <van-cell-group inset title="建立同步空间">
        <van-field v-model="deviceName" label="本机名称" maxlength="80" clearable />
        <van-cell title="建立同步空间" is-link @click="createSpace" />
      </van-cell-group>

      <van-cell-group inset title="加入已有空间">
        <van-field
          v-model="joinCode"
          label="配对码"
          placeholder="六位数字"
          maxlength="6"
          type="digit"
          clearable
        />
        <van-cell title="查找并认领" is-link @click="requestJoinOffer(joinCode)" />
        <div class="hint-block">已有设备批准后，本机会自动完成加入，不需要再点确认。</div>
      </van-cell-group>

      <van-cell-group inset title="我是新设备">
        <van-cell title="显示我的配对码" is-link label="已有设备批准后自动完成加入" @click="requestStartJoin" />
      </van-cell-group>
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
