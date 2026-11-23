<script setup lang="ts">
import { computed, onBeforeUnmount, onMounted, ref } from 'vue'
import { useRouter } from 'vue-router'
import { showConfirmDialog, showToast } from 'vant'
import { useSyncStore } from '../../src/stores/sync'
import vaultSyncApi, {
  type PairingCodeView,
  type PairingStatusView,
  type VaultDeviceView,
  type VaultSyncConfigView,
} from '../../src/services/vaultSyncApi'

const router = useRouter()
const syncStore = useSyncStore()

const config = ref<VaultSyncConfigView | null>(null)
const endpoint = ref('')
const deviceName = ref('')
const inviteCode = ref('')
const joinCode = ref('')
const joinStatus = ref('')
const joinVerification = ref('')
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

async function loadConfig() {
  config.value = await vaultSyncApi.getConfig()
  if (!deviceName.value) deviceName.value = config.value.deviceName || '手机'
  if (!endpoint.value) endpoint.value = config.value.endpoint || ''
}

async function loadDevices() {
  if (!config.value?.active) return
  try {
    devices.value = await vaultSyncApi.listDevices()
  } catch (error: any) {
    showToast(error?.message || '读取设备失败')
  }
}

async function testConnection() {
  if (busy.value) return
  busy.value = true
  try {
    showToast(await vaultSyncApi.test(endpoint.value))
  } catch (error: any) {
    showToast(error?.message || '连接失败')
  } finally {
    busy.value = false
  }
}

async function createVault() {
  if (busy.value) return
  busy.value = true
  try {
    config.value = await vaultSyncApi.createVault(endpoint.value, inviteCode.value, deviceName.value)
    inviteCode.value = ''
    showToast('同步空间已创建')
    await syncStore.refreshStatus()
    void syncStore.triggerSync()
    await loadDevices()
  } catch (error: any) {
    showToast(error?.message || '创建失败')
  } finally {
    busy.value = false
  }
}

async function startPairing() {
  if (busy.value) return
  busy.value = true
  try {
    const code = await vaultSyncApi.startPairing()
    pairing.value = { ...code, status: 'open', claimedDevice: null, verificationCode: null }
    stopPairingTimer()
    pairingTimer = setInterval(pollPairing, 2000)
  } catch (error: any) {
    showToast(error?.message || '生成配对码失败')
  } finally {
    busy.value = false
  }
}

async function pollPairing() {
  if (!pairing.value) return
  try {
    const status = await vaultSyncApi.pairingStatus(pairing.value.pairingId)
    pairing.value = { ...pairing.value, ...status }
    if (['expired', 'canceled', 'confirmed'].includes(status.status)) stopPairingTimer()
  } catch (error: any) {
    showToast(error?.message || '查询配对状态失败')
    stopPairingTimer()
  }
}

async function confirmPairing() {
  if (!pairing.value || busy.value) return
  busy.value = true
  try {
    await vaultSyncApi.confirmPairing(pairing.value.pairingId)
    pairing.value = { ...pairing.value, status: 'confirmed' }
    stopPairingTimer()
    showToast('新设备已加入')
    await loadDevices()
  } catch (error: any) {
    showToast(error?.message || '确认失败')
  } finally {
    busy.value = false
  }
}

async function joinVault() {
  if (busy.value) return
  busy.value = true
  joinStatus.value = '正在认领配对码…'
  try {
    const start = await vaultSyncApi.beginJoin(endpoint.value, joinCode.value, deviceName.value)
    joinPairingId = start.pairingId
    joinVerification.value = start.verificationCode
    joinStatus.value = '等待已有设备确认…'
    stopJoinTimer()
    joinTimer = setInterval(pollJoin, 2000)
  } catch (error: any) {
    joinStatus.value = ''
    busy.value = false
    showToast(error?.message || '加入失败')
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
      joinStatus.value = '已加入，正在拉取远端数据…'
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
  } catch (error: any) {
    stopJoinTimer()
    joinPairingId = ''
    busy.value = false
    showToast(error?.message || '加入失败')
  }
}

async function syncNow() {
  const ok = await syncStore.triggerSync()
  showToast(ok ? '同步完成' : (syncStore.errorMsg || '同步失败'))
  await loadDevices()
}

async function seedLocal() {
  if (busy.value) return
  try {
    await showConfirmDialog({
      title: '上传本机独有文稿',
      message: '只上传远端空间中不存在的本机文稿，不会覆盖已有内容。',
    })
  } catch {
    return
  }
  busy.value = true
  try {
    const count = await vaultSyncApi.seedLocal()
    showToast(count > 0 ? `已加入 ${count} 项，正在上传` : '没有需要上传的内容')
    if (count > 0) void syncStore.triggerSync()
  } catch (error: any) {
    showToast(error?.message || '登记失败')
  } finally {
    busy.value = false
  }
}

async function revoke(device: VaultDeviceView) {
  try {
    await showConfirmDialog({
      title: '撤销设备',
      message: `撤销「${device.name}」后，它需要重新配对才能继续同步。`,
    })
  } catch {
    return
  }
  busy.value = true
  try {
    await vaultSyncApi.revokeDevice(device.id)
    showToast('已撤销')
    await loadDevices()
  } catch (error: any) {
    showToast(error?.message || '撤销失败')
  } finally {
    busy.value = false
  }
}

async function leave() {
  try {
    await showConfirmDialog({
      title: '离开同步空间',
      message: '本机笔记仍会保留，但不再与其他设备同步。确定离开吗？',
    })
  } catch {
    return
  }
  busy.value = true
  try {
    await vaultSyncApi.leave()
    config.value = await vaultSyncApi.getConfig()
    devices.value = []
    pairing.value = null
    stopPairingTimer()
    stopJoinTimer()
    showToast('已离开同步空间')
    await syncStore.refreshStatus()
  } catch (error: any) {
    showToast(error?.message || '离开失败')
  } finally {
    busy.value = false
  }
}

function platformLabel(platform: string) {
  return ({ macos: 'macOS', windows: 'Windows', linux: 'Linux', android: 'Android', ios: 'iOS' } as Record<string, string>)[platform] || platform
}

function stopPairingTimer() {
  if (pairingTimer) clearInterval(pairingTimer)
  pairingTimer = null
}

function stopJoinTimer() {
  if (joinTimer) clearInterval(joinTimer)
  joinTimer = null
}

onMounted(async () => {
  try {
    await loadConfig()
    await syncStore.refreshStatus()
    await loadDevices()
  } catch (error: any) {
    showToast(error?.message || '读取同步配置失败')
  }
})

onBeforeUnmount(() => {
  stopPairingTimer()
  stopJoinTimer()
})
</script>

<template>
  <div class="sync-page">
    <van-nav-bar title="同步空间" left-arrow fixed placeholder @click-left="router.back()" />

    <template v-if="config?.active">
      <van-cell-group inset title="当前空间">
        <van-cell title="服务地址" :label="config.endpoint" />
        <van-cell title="空间 ID" :value="shortVaultId" />
        <van-cell title="本机名称" :value="config.deviceName" />
        <van-cell title="同步状态" :value="syncStore.isSyncing ? '同步中…' : syncStore.lastSyncLabel" />
      </van-cell-group>

      <van-cell-group v-if="pairing" inset title="新设备配对">
        <van-cell title="六位配对码">
          <template #value>
            <span class="pairing-code">{{ pairing.shortCode }}</span>
          </template>
        </van-cell>
        <van-cell v-if="pairing.claimedDevice" title="新设备" :value="pairing.claimedDevice.name" />
        <van-cell v-if="pairing.verificationCode" title="核对码">
          <template #value>
            <span class="pairing-code">{{ pairing.verificationCode }}</span>
          </template>
        </van-cell>
        <van-cell
          v-if="pairing.claimedDevice"
          title="确认新设备加入"
          is-link
          :label="'请确认新设备屏幕显示相同核对码'"
          @click="confirmPairing"
        />
        <van-cell v-else title="等待新设备输入配对码…" />
      </van-cell-group>

      <van-cell-group inset title="设备">
        <van-cell
          v-for="device in devices"
          :key="device.id"
          :title="device.name"
          :label="platformLabel(device.platform) + (device.isCurrent ? ' · 当前设备' : '')"
        >
          <template v-if="!device.isCurrent" #right-icon>
            <van-button size="mini" type="danger" plain :disabled="busy" @click="revoke(device)">撤销</van-button>
          </template>
        </van-cell>
      </van-cell-group>

      <p class="hint">
        请保留至少一台已配对设备并备份应用数据目录。当前版本没有恢复码，密钥丢失后无法解密。
      </p>

      <div class="actions">
        <van-button block type="primary" :loading="syncStore.isSyncing" @click="syncNow">立即同步</van-button>
        <van-button block plain :disabled="busy" @click="startPairing">生成六位配对码</van-button>
        <van-button block plain :disabled="busy" @click="seedLocal">上传本机独有文稿</van-button>
        <van-button block plain type="danger" :disabled="busy" @click="leave">离开同步空间</van-button>
      </div>
    </template>

    <template v-else>
      <van-cell-group inset title="连接服务">
        <van-field v-model="endpoint" label="服务地址" placeholder="https://woo-sync.example.workers.dev" clearable />
        <van-field v-model="deviceName" label="本机名称" maxlength="80" clearable />
        <van-cell title="测试连接" is-link @click="testConnection" />
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
        <van-cell title="加入空间" is-link :label="joinStatus" @click="joinCode.length === 6 && joinVault()" />
        <van-cell v-if="joinVerification" title="核对码">
          <template #value><span class="pairing-code">{{ joinVerification }}</span></template>
        </van-cell>
      </van-cell-group>

      <van-cell-group inset title="创建新空间">
        <van-field v-model="inviteCode" label="邀请码" placeholder="部署同步服务时设置" type="password" clearable />
        <van-cell title="创建空间" is-link @click="createVault" />
      </van-cell-group>

      <p class="hint">
        如果这台设备之前使用 WebDAV，建议先完成一次同步再创建空间。
        六位配对码只用于加入空间，不会上传到第三方服务。
      </p>
    </template>
  </div>
</template>

<style scoped>
.sync-page { padding: 12px 0 32px; display: flex; flex-direction: column; gap: 12px; }
.pairing-code { font-size: 22px; letter-spacing: 5px; font-weight: 600; color: var(--van-primary-color); }
.actions { display: flex; flex-direction: column; gap: 10px; padding: 16px; }
.hint { margin: 0 20px; color: var(--van-text-color-2); font-size: 12px; line-height: 1.7; }
</style>
