<script setup lang="ts">
import { onBeforeUnmount, onMounted, ref } from 'vue'
import { useRouter } from 'vue-router'
import { showConfirmDialog, showToast } from 'vant'
import { useSyncStore } from '../../src/stores/sync'
import webdavSpaceApi, {
  type PairingStatusView,
  type WebdavSpaceConfigView,
} from '../../src/services/webdavSpaceApi'

const router = useRouter()
const syncStore = useSyncStore()

const config = ref<WebdavSpaceConfigView | null>(null)
const endpoint = ref('')
const username = ref('')
const password = ref('')
const deviceName = ref('')
const joinCode = ref('')
const hostJoinCode = ref('')
const showHostJoinInput = ref(false)
const busy = ref(false)
const hostPairing = ref<(PairingStatusView & { shortCode?: string }) | null>(null)
const guestPairing = ref<(PairingStatusView & { shortCode?: string }) | null>(null)
let hostTimer: ReturnType<typeof setInterval> | null = null
let guestTimer: ReturnType<typeof setInterval> | null = null
let pollingHost = false
let pollingGuest = false
let hostMode: 'offer' | 'join' = 'offer'

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

function formatTime(ms: number) {
  return new Date(ms).toLocaleTimeString('zh-CN', { hour: '2-digit', minute: '2-digit' })
}

async function loadConfig() {
  config.value = await webdavSpaceApi.getConfig()
  endpoint.value = config.value.account.endpoint || endpoint.value
  username.value = config.value.account.username || username.value
  if (!deviceName.value) deviceName.value = config.value.identity.deviceName || '手机'
  if (config.value.notice) showToast(config.value.notice)
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

async function findOffer() {
  if (busy.value) return
  busy.value = true
  try {
    await resetPairing()
    const status = await webdavSpaceApi.findOffer(joinCode.value, deviceName.value)
    guestPairing.value = { ...status, shortCode: joinCode.value }
    if (status.status !== 'ready' && status.status !== 'expired') {
      guestTimer = setInterval(pollGuest, 2000)
    }
  } catch (error: any) {
    showToast(error?.message || '配对失败')
  } finally {
    busy.value = false
  }
}

async function startJoin() {
  if (busy.value) return
  busy.value = true
  try {
    await resetPairing()
    const start = await webdavSpaceApi.startJoin(deviceName.value)
    guestPairing.value = {
      pairingId: start.pairingId,
      shortCode: start.shortCode,
      status: 'waiting',
      expiresAt: start.expiresAt,
      verificationCode: start.verificationCode,
      peer: null,
      vaultId: null,
      spaceFingerprint: null,
      message: '等待已有设备输入六位码并批准…'
    }
    guestTimer = setInterval(pollGuest, 2000)
  } catch (error: any) {
    showToast(error?.message || '生成配对码失败')
  } finally {
    busy.value = false
  }
}

async function pollGuest() {
  if (!guestPairing.value || pollingGuest) return
  pollingGuest = true
  try {
    const status = await webdavSpaceApi.joinStatus(guestPairing.value.pairingId)
    guestPairing.value = { ...guestPairing.value, ...status }
    if (status.status === 'ready' || status.status === 'expired') stopGuestTimer()
  } catch (error: any) {
    stopGuestTimer()
    showToast(error?.message || '查询配对状态失败')
  } finally {
    pollingGuest = false
  }
}

async function confirmGuest() {
  if (!guestPairing.value || busy.value) return
  busy.value = true
  try {
    await webdavSpaceApi.confirmJoin(guestPairing.value.pairingId)
    stopGuestTimer()
    guestPairing.value = null
    showToast('已加入同步空间')
    await loadConfig()
    await syncStore.refreshStatus()
    void syncStore.triggerSync()
  } catch (error: any) {
    showToast(error?.message || '完成加入失败')
  } finally {
    busy.value = false
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
    showToast('已批准新设备，请在新设备上确认')
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
  joinCode.value = ''
  hostJoinCode.value = ''
  showHostJoinInput.value = false
  await webdavSpaceApi.cancelPairing().catch(() => {})
}

async function syncNow() {
  const ok = await syncStore.triggerSync()
  showToast(ok ? '同步完成' : (syncStore.errorMsg || '同步失败'))
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
        https://nas.example.com:5006/woo。应用密码只加密保存在本机。
      </div>
    </van-cell-group>

    <p v-if="config?.legacyNotice" class="hint">{{ config.legacyNotice }}</p>
    <p v-if="config?.notice" class="hint">{{ config.notice }}</p>

    <template v-if="config?.account.active && !config?.spaceConfigured">
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
        <van-cell title="查找并认领" is-link @click="joinCode.length === 6 && findOffer()" />
      </van-cell-group>

      <van-cell-group inset title="我是新设备">
        <van-cell title="显示我的配对码" is-link label="在已有设备上输入并批准" @click="startJoin" />
      </van-cell-group>
    </template>

    <template v-if="config?.active">
      <van-cell-group inset title="当前空间">
        <van-cell title="WebDAV" :label="config.account.endpoint" />
        <van-cell title="账号" :value="config.account.username" />
        <van-cell title="空间指纹" :value="config.identity.spaceFingerprint" />
        <van-cell title="本机" :label="platformLabel(config.identity.platform)" :value="config.identity.deviceName" />
        <van-cell title="同步状态" :value="syncStore.isSyncing ? '同步中…' : syncStore.lastSyncLabel" />
      </van-cell-group>

      <div class="hint">
        启动、本地修改、回到前台和每 15 分钟兜底时同步。没有服务端设备列表，不能远程吊销；
        如需阻止旧设备，请修改 WebDAV 应用密码。
      </div>

      <div class="actions">
        <van-button block type="primary" :loading="syncStore.isSyncing" @click="syncNow">立即同步</van-button>
        <van-button block plain :disabled="busy" @click="startOffer">显示配对码</van-button>
        <van-button block plain :disabled="busy" @click="showHostJoinInput = !showHostJoinInput">
          输入新设备配对码
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
        请确认新设备屏幕显示相同的六位码和空间指纹，再批准。
      </div>
      <van-cell
        v-if="hostPairing.status === 'claimed'"
        title="核对一致，批准新设备"
        is-link
        @click="approveHost"
      />
      <van-cell v-if="hostPairing.status === 'approved'" title="已批准，等待新设备确认" />
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
      <van-cell :title="guestPairing.message || '等待已有设备确认…'" />
      <div v-if="guestPairing.status === 'ready'" class="hint">
        请与已有设备核对六位码和空间指纹一致后，再完成加入。
      </div>
      <van-cell
        v-if="guestPairing.status === 'ready'"
        title="核对一致，完成加入"
        is-link
        @click="confirmGuest"
      />
      <van-cell v-if="guestPairing.status === 'expired'" title="配对码已过期，请重新生成" />
      <van-cell title="取消配对" is-link @click="cancelPairing" />
    </van-cell-group>
  </div>
</template>

<style scoped>
.sync-page { padding: 12px 0 32px; display: flex; flex-direction: column; gap: 12px; }
.pairing-code { font-size: 22px; letter-spacing: 5px; font-weight: 600; color: var(--van-primary-color); }
.actions { display: flex; flex-direction: column; gap: 10px; padding: 16px; }
.hint { margin: 0 20px; color: var(--van-text-color-2); font-size: 12px; line-height: 1.7; }
.hint-block { padding: 8px 16px 12px; color: var(--van-text-color-2); font-size: 12px; line-height: 1.7; overflow-wrap: anywhere; }
</style>
